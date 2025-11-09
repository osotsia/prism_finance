//! Integrates with the IPOPT NLP solver to find solutions to constrained problems.

use crate::computation::{ComputationError, Ledger};
use crate::solver::ipopt_adapter;
use crate::solver::ipopt_ffi;
use crate::solver::problem::PrismProblem;
use libc::c_int;
use std::ffi::c_void;
use std::sync::Arc;

/// The main entry point for the solver.
pub fn solve(problem: PrismProblem) -> Result<Ledger, ComputationError> {
    let model_len = problem.model_len;
    let num_vars = problem.variables.len() * model_len;
    let num_constraints = problem.residuals.len() * model_len;

    if num_vars == 0 {
        // Nothing to solve, return the pre-computed ledger.
        return Ok(problem.base_ledger);
    }
    if num_constraints == 0 {
        return Err(ComputationError::SolverConfiguration(
            "Solver variables exist but no constraints were defined.".to_string(),
        ));
    }

    // --- 1. Define bounds ---
    let mut x_l: Vec<ipopt_ffi::Number> = vec![ipopt_ffi::IPOPT_NEGINF; num_vars];
    let mut x_u: Vec<ipopt_ffi::Number> = vec![ipopt_ffi::IPOPT_POSINF; num_vars];
    // All constraints are equality constraints, so their lower and upper bounds are 0.
    let mut g_l: Vec<ipopt_ffi::Number> = vec![0.0; num_constraints];
    let mut g_u: Vec<ipopt_ffi::Number> = vec![0.0; num_constraints];

    // --- 2. Initial guess ---
    let mut x_init: Vec<ipopt_ffi::Number> = vec![0.0; num_vars];

    // --- 3. Box user data to pass to callbacks ---
    // The `PrismProblem` contains all the context our callbacks need.
    // We put it on the heap and pass a raw pointer to IPOPT.
    let user_data = Box::into_raw(Box::new(problem));

    // --- 4. Create the IPOPT problem ---
    let ipopt_problem = unsafe {
        ipopt_ffi::CreateIpoptProblem(
            num_vars as c_int,
            x_l.as_mut_ptr(),
            x_u.as_mut_ptr(),
            num_constraints as c_int,
            g_l.as_mut_ptr(),
            g_u.as_mut_ptr(),
            (num_vars * num_constraints) as c_int, // Non-zeros in Jacobian (dense)
            0,                                    // Non-zeros in Hessian (not used)
            ipopt_ffi::FR_C_STYLE,
            Some(ipopt_adapter::eval_f),
            Some(ipopt_adapter::eval_g),
            Some(ipopt_adapter::eval_grad_f),
            Some(ipopt_adapter::eval_jac_g),
            None, // No Hessian evaluation needed
            user_data as *mut c_void,
        )
    };

    if ipopt_problem.is_null() {
        // Must reclaim the Box if problem creation fails.
        let _ = unsafe { Box::from_raw(user_data) };
        return Err(ComputationError::SolverConfiguration(
            "IPOPT failed to create problem.".to_string(),
        ));
    }

    // --- Set solver options (optional) ---
    unsafe {
        // Suppress verbose IPOPT banner and output.
        ipopt_ffi::AddIpoptIntOption(ipopt_problem, "print_level\0".as_ptr() as *const i8, 0);
        ipopt_ffi::AddIpoptNumOption(ipopt_problem, "tol\0".as_ptr() as *const i8, 1e-9);
    };

    // --- 5. Solve the problem ---
    let mut solve_status: ipopt_ffi::ApplicationReturnStatus;
    let mut final_obj_val: ipopt_ffi::Number = 0.0;
    
    // IPOPT writes its solution back into the initial guess vector `x_init`.
    // We will later copy this into `final_x`.
    
    unsafe {
        solve_status = ipopt_ffi::IpoptSolve(
            ipopt_problem,
            x_init.as_mut_ptr(),
            g_l.as_mut_ptr(), // IPOPT can write final constraint values here, but we don't need them.
            &mut final_obj_val,
            std::ptr::null_mut(), // Multiplier info not needed
            std::ptr::null_mut(), // Multiplier info not needed
            std::ptr::null_mut(), // Multiplier info not needed
            user_data as *mut c_void,
        );
    }

    // The solution is now in `x_init`.
    let final_x = x_init;

    // --- 6. Clean up ---
    unsafe {
        ipopt_ffi::FreeIpoptProblem(ipopt_problem);
    };
    // IMPORTANT: Reclaim the Box to deallocate the PrismProblem.
    let solved_problem = unsafe { Box::from_raw(user_data) };

    // --- 7. Process results ---
    if solve_status == ipopt_ffi::ApplicationReturnStatus::Solve_Succeeded {
        let mut final_ledger = solved_problem.base_ledger.clone();
        for (i, var_id) in solved_problem.variables.iter().enumerate() {
            let start_idx = i * model_len;
            let end_idx = start_idx + model_len;
            let var_values = final_x[start_idx..end_idx].to_vec();
            final_ledger.insert(*var_id, Ok(Arc::new(var_values)));
        }
        Ok(final_ledger)
    } else {
        Err(ComputationError::SolverDidNotConverge(format!(
            "IPOPT failed with status code: {:?}",
            solve_status
        )))
    }
}