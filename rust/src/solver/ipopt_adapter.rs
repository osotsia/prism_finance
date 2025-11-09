//! C-style callback functions that bridge Rust logic to IPOPT.
//!
//! This module contains the `extern "C"` functions that are passed as function
//! pointers to the IPOPT C API. These functions are thin wrappers that handle
//! the unsafe C-to-Rust transition and then call the safe Rust logic.

use crate::computation::ledger::SolverIteration;
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
        let model_len = problem.model_len;

        let mut ledger = problem.base_ledger.clone();

        // Set the current guess for solver variables in the ledger by un-flattening x.
        for (i, var_id) in problem.variables.iter().enumerate() {
            let start_idx = i * model_len;
            let end_idx = start_idx + model_len;
            let var_values = x_slice[start_idx..end_idx].to_vec();
            ledger.insert(*var_id, Ok(Arc::new(var_values)));
        }

        // Compute the values of the residual nodes based on the current guess.
        let compute_result = problem.sync_engine.compute(&problem.residuals, &mut ledger);
        if let Err(e) = compute_result {
            // Immediately return an error if the computation engine fails.
            return Err(format!("Computation engine failed: {}", e.to_string()));
        }

        // Populate the output slice `g` by flattening the computed residual values.
        for (i, residual_id) in problem.residuals.iter().enumerate() {
            match ledger.get(*residual_id) {
                Some(Ok(val_arc)) => {
                    let start_idx = i * model_len;
                    for t in 0..model_len {
                        // Handle broadcasting of results if residual computed as scalar.
                        let val = *val_arc.get(t).unwrap_or_else(|| val_arc.last().unwrap_or(&0.0));
                        g_slice[start_idx + t] = val;
                    }
                }
                Some(Err(e)) => {
                    return Err(format!("Upstream error computing residual {}: {}", residual_id.index(), e));
                }
                None => {
                    let msg = format!("Failed to compute residual for node {}", residual_id.index());
                    return Err(msg);
                }
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
        for r in 0..m_usize {
            for c in 0..n_usize {
                iRow_slice[idx] = r as Index;
                jCol_slice[idx] = c as Index;
                idx += 1;
            }
        }
        return 1; // Return 1 (true) for success
    }

    // Otherwise, IPOPT is asking for the Jacobian values.
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

                // Compute g_i(x + h*e_j)
                x_mut[j] = original_xj + h;
                let g_plus = get_g_i(i, &x_mut, user_data)?;

                // Compute g_i(x - h*e_j)
                x_mut[j] = original_xj - h;
                let g_minus = get_g_i(i, &x_mut, user_data)?;

                // Restore original value
                x_mut[j] = original_xj;

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
/// It captures the state of the solver and stores it for later display.
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

        let iteration_data = SolverIteration {
            iter_count,
            obj_value,
            inf_pr,
            inf_du,
        };

        // Lock the mutex and push the new iteration data. The lock is released
        // automatically when `history` goes out of scope.
        match problem.iteration_history.lock() {
            Ok(mut history) => {
                history.push(iteration_data);
                Ok(true)
            }
            Err(e) => Err(format!("Failed to lock iteration history mutex: {}", e)),
        }
    })
}

/// Helper function to evaluate a single flattened constraint `g_i` at a given point `x`.
/// This is used by the finite differencing logic in `eval_jac_g`.
fn get_g_i(ipopt_con_idx: usize, x: &[f64], user_data: *mut c_void) -> Result<f64, String> {
    let problem = unsafe { get_problem(user_data) };
    let model_len = problem.model_len;

    // Map from flattened IPOPT constraint index to (residual_node, time_step)
    let residual_list_idx = ipopt_con_idx / model_len;
    let time_step = ipopt_con_idx % model_len;
    let residual_node_id = problem.residuals[residual_list_idx];

    let mut ledger = problem.base_ledger.clone();

    // Un-flatten the IPOPT variable vector `x` into the ledger
    for (k, var_id) in problem.variables.iter().enumerate() {
        let start_idx = k * model_len;
        let end_idx = start_idx + model_len;
        let var_values = x[start_idx..end_idx].to_vec();
        ledger.insert(*var_id, Ok(Arc::new(var_values)));
    }

    // We only need this one residual, but the engine will compute its dependencies.
    problem
        .sync_engine
        .compute(&[residual_node_id], &mut ledger)
        .map_err(|e| e.to_string())?;

    match ledger.get(residual_node_id) {
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