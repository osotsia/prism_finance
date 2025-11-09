//! C-style callback functions that bridge Rust logic to IPOPT.
//!
//! This module contains the `extern "C"` functions that are passed as function
//! pointers to the IPOPT C API. These functions are thin wrappers that handle
//! the unsafe C-to-Rust transition and then call the safe Rust logic.

use crate::computation::ledger::{ComputationError, Ledger, SolverIteration};
use crate::graph::NodeId;
use crate::solver::ipopt_ffi::Bool;
use crate::solver::problem::PrismProblem;
use libc::{c_int, c_void};
use std::panic::{catch_unwind, UnwindSafe};
use std::slice;
use std::sync::Arc;

type Number = f64;
type Index = c_int;

/// A wrapper function to execute a closure within a `catch_unwind` block.
/// If a panic occurs, it prints an error and returns `false`, which signals
/// an error to IPOPT.
fn ipopt_callback_wrapper<F>(closure: F) -> Bool
where
    F: FnOnce() -> Result<bool, String> + UnwindSafe,
{
    match catch_unwind(closure) {
        Ok(Ok(success)) => {
            if success { 1 } else { 0 }
        }
        Ok(Err(msg)) => {
            eprintln!("\n[PRISM DEBUG] --- ERROR in IPOPT Callback ---");
            eprintln!("[PRISM DEBUG] The following error prevented the constraints from being evaluated:");
            eprintln!("[PRISM DEBUG] Details: {}", msg);
            eprintln!("[PRISM DEBUG] ----------------------------------\n");
            0 // Return 0 (false) to IPOPT on error
        }
        Err(_) => {
            eprintln!("FATAL: Panic occurred within an IPOPT callback.");
            0 // Return 0 (false) to IPOPT on panic
        }
    }
}

/// Helper to get a mutable reference to the `PrismProblem` from `user_data`.
unsafe fn get_problem<'a>(user_data: *mut c_void) -> &'a mut PrismProblem<'a> {
    &mut *(user_data as *mut PrismProblem)
}

/// A "Scenario Engine": The core helper that takes a potential solution `x_guess` from the
/// solver and runs the computation graph to see what the results would be. This is the
/// heart of the callbacks, used to evaluate both the constraints and their derivatives.
fn evaluate_graph_at_point<'a>(
    problem: &'a PrismProblem<'a>,
    x_guess: &[f64],
    targets: &[NodeId],
) -> Result<Ledger, ComputationError> {
    let mut ledger = problem.base_ledger.clone();
    let model_len = problem.model_len;

    // "The Unpacker": Translate the solver's flat list of numbers `x_guess` into
    // the structured, time-series values needed by the computation engine.
    for (i, var_id) in problem.variables.iter().enumerate() {
        let start_idx = i * model_len;
        let end_idx = start_idx + model_len;
        let var_values = x_guess[start_idx..end_idx].to_vec();
        ledger.insert(*var_id, Ok(Arc::new(var_values)));
    }

    // "The Calculator": Run the engine to compute the values of the target nodes.
    problem.sync_engine.compute(targets, &mut ledger)?;
    Ok(ledger)
}

/// Computes the objective function. For Prism, this is always 0, as we
/// are only solving for constraint satisfaction.
pub extern "C" fn eval_f(
    _n: Index,
    _x: *mut Number,
    _new_x: Bool,
    obj_value: *mut Number,
    _user_data: *mut c_void,
) -> Bool {
    unsafe {
        *obj_value = 0.0;
    }
    1 // Return 1 (true) for success
}

/// Computes the gradient of the objective function. Since our objective is
/// constant (0), the gradient is always a vector of zeros.
pub extern "C" fn eval_grad_f(
    n: Index,
    _x: *mut Number,
    _new_x: Bool,
    grad_f: *mut Number,
    _user_data: *mut c_void,
) -> Bool {
    let grad_f_slice = unsafe { slice::from_raw_parts_mut(grad_f, n as usize) };
    grad_f_slice.fill(0.0);
    1 // Return 1 (true) for success
}

/// Computes the values of the constraint functions (the residuals).
pub extern "C" fn eval_g(
    n: Index,
    x: *mut Number,
    _new_x: Bool,
    m: Index,
    g: *mut Number,
    user_data: *mut c_void,
) -> Bool {
    ipopt_callback_wrapper(|| {
        let problem = unsafe { get_problem(user_data) };
        let x_slice = unsafe { slice::from_raw_parts(x, n as usize) };
        let g_slice = unsafe { slice::from_raw_parts_mut(g, m as usize) };

        // Evaluate the graph with the current guess `x` to get the constraint values.
        let result_ledger = evaluate_graph_at_point(problem, x_slice, &problem.residuals)
            .map_err(|e| format!("Computation engine failed: {}", e.to_string()))?;

        // "The Packer": Flatten the structured results back into the simple list `g` that IPOPT expects.
        for (i, residual_id) in problem.residuals.iter().enumerate() {
            match result_ledger.get(*residual_id) {
                Some(Ok(val_arc)) => {
                    let start_idx = i * problem.model_len;
                    for t in 0..problem.model_len {
                        let val = *val_arc.get(t).unwrap_or_else(|| val_arc.last().unwrap_or(&0.0));
                        g_slice[start_idx + t] = val;
                    }
                }
                Some(Err(e)) => return Err(format!("Upstream error for residual {}: {}", residual_id.index(), e)),
                None => return Err(format!("Failed to compute residual {}", residual_id.index())),
            }
        }
        Ok(true)
    })
}

/// Computes the Jacobian of the constraint functions.
#[allow(non_snake_case)]
pub extern "C" fn eval_jac_g(
    n: Index,
    x: *mut Number,
    _new_x: Bool,
    m: Index,
    nele_jac: Index,
    iRow: *mut Index,
    jCol: *mut Index,
    values: *mut Number,
    user_data: *mut c_void,
) -> Bool {
    // If `values` is null, IPOPT is asking for the sparsity structure.
    if values.is_null() {
        let n_usize = n as usize;
        let m_usize = m as usize;
        let iRow_slice = unsafe { slice::from_raw_parts_mut(iRow, nele_jac as usize) };
        let jCol_slice = unsafe { slice::from_raw_parts_mut(jCol, nele_jac as usize) };
        let mut idx = 0;
        // We assume a dense Jacobian for simplicity.
        for r in 0..m_usize {
            for c in 0..n_usize {
                iRow_slice[idx] = r as Index;
                jCol_slice[idx] = c as Index;
                idx += 1;
            }
        }
        return 1; // Return 1 (true) for success
    }

    // Otherwise, IPOPT is asking for the Jacobian values, which we approximate.
    ipopt_callback_wrapper(|| {
        let n_usize = n as usize;
        let values_slice = unsafe { slice::from_raw_parts_mut(values, nele_jac as usize) };
        let x_slice = unsafe { slice::from_raw_parts(x, n_usize) };
        let mut x_mut = x_slice.to_vec();

        let h = 1e-8; // Step size for finite difference
        let mut jac_idx = 0;

        // Loop over IPOPT constraints `g_i` (rows of Jacobian)
        for i in 0..(m as usize) {
            // Loop over IPOPT variables `x_j` (columns of Jacobian)
            for j in 0..n_usize {
                let original_xj = x_mut[j];

                x_mut[j] = original_xj + h;
                let g_plus = get_single_constraint_value(i, &x_mut, user_data)?;

                x_mut[j] = original_xj - h;
                let g_minus = get_single_constraint_value(i, &x_mut, user_data)?;
                
                x_mut[j] = original_xj; // Restore original value

                // Central difference formula for d(g_i)/d(x_j)
                values_slice[jac_idx] = (g_plus - g_minus) / (2.0 * h);
                jac_idx += 1;
            }
        }
        Ok(true)
    })
}

/// A dummy callback for the Hessian of the Lagrangian.
/// This is required by the IPOPT C interface, even when using a quasi-Newton
/// approximation for the Hessian. It will not be called if the approximation
/// is enabled, but the function pointer cannot be null.
#[allow(non_snake_case)]
pub extern "C" fn eval_h(
    _n: Index,
    _x: *mut Number,
    _new_x: Bool,
    _obj_factor: Number,
    _m: Index,
    _lambda: *mut Number,
    _new_lambda: Bool,
    _nele_hess: Index,
    _iRow: *mut Index,
    _jCol: *mut Index,
    _values: *mut Number,
    _user_data: *mut c_void,
) -> Bool {
    1 // Return 1 (true) for success
}

/// The callback executed by IPOPT at the end of each iteration.
#[allow(non_snake_case)]
pub extern "C" fn intermediate_callback(
    _alg_mod: Index,
    iter_count: Index,
    obj_value: Number,
    inf_pr: Number,
    inf_du: Number,
    _mu: Number,
    _d_norm: Number,
    _regularization_size: Number,
    _alpha_du: Number,
    _alpha_pr: Number,
    _ls_trials: Index,
    user_data: *mut c_void,
) -> Bool {
    ipopt_callback_wrapper(|| {
        let problem = unsafe { get_problem(user_data) };
        let iteration_data = SolverIteration { iter_count, obj_value, inf_pr, inf_du };
        match problem.iteration_history.lock() {
            Ok(mut history) => {
                history.push(iteration_data);
                Ok(true)
            }
            Err(e) => Err(format!("Failed to lock iteration history mutex: {}", e)),
        }
    })
}

/// Helper to evaluate a single constraint `g_i` at a point `x`, for finite differencing.
fn get_single_constraint_value(ipopt_con_idx: usize, x: &[f64], user_data: *mut c_void) -> Result<f64, String> {
    let problem = unsafe { get_problem(user_data) };
    let model_len = problem.model_len;

    // Map from flattened IPOPT constraint index to (residual_node, time_step)
    let residual_list_idx = ipopt_con_idx / model_len;
    let time_step = ipopt_con_idx % model_len;
    let residual_node_id = problem.residuals[residual_list_idx];

    let result_ledger = evaluate_graph_at_point(problem, x, &[residual_node_id])
        .map_err(|e| e.to_string())?;

    match result_ledger.get(residual_node_id) {
        Some(Ok(val_arc)) => {
            // Get the value at the specific time step, handling broadcasting.
            let val = *val_arc
                .get(time_step)
                .unwrap_or_else(|| val_arc.last().unwrap_or(&0.0));
            Ok(val)
        }
        _ => Err(format!(
            "Failed to compute residual for node {}",
            residual_node_id.index()
        )),
    }
}