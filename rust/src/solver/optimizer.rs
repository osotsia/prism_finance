use crate::store::{Registry, NodeId};
use crate::compute::{engine::Engine, ledger::{Ledger, ComputationError}, bytecode::Program};
use super::problem::PrismProblem;
use super::ipopt_adapter;
use super::ipopt_ffi;
use std::sync::{Arc, Mutex};
use std::ffi::c_void;
use libc::c_int;

pub fn solve(
    registry: &Registry, 
    program: &Program,
    solver_vars: Vec<NodeId>, 
    residuals: Vec<NodeId>,
    base_ledger: Ledger,
    model_len: usize,
) -> Result<Ledger, ComputationError> {
    
    let problem = PrismProblem {
        registry,
        program,
        variables: solver_vars.clone(),
        residuals: residuals.clone(),
        model_len,
        base_ledger,
        iteration_history: Mutex::new(Vec::new()),
    };
    
    let n_vars = (problem.variables.len() * model_len) as c_int;
    let n_cons = (problem.residuals.len() * model_len) as c_int;
    
    // Initial guess (all zeros)
    let mut x_init = vec![0.0; n_vars as usize];

    let user_data = Box::into_raw(Box::new(problem));

    let ipopt_prob = unsafe {
        ipopt_ffi::CreateIpoptProblem(
            n_vars,
            vec![ipopt_ffi::IPOPT_NEGINF; n_vars as usize].as_mut_ptr(),
            vec![ipopt_ffi::IPOPT_POSINF; n_vars as usize].as_mut_ptr(),
            n_cons,
            vec![0.0; n_cons as usize].as_mut_ptr(),
            vec![0.0; n_cons as usize].as_mut_ptr(),
            n_vars * n_cons, 
            0, 
            ipopt_ffi::FR_C_STYLE,
            Some(ipopt_adapter::eval_f),
            Some(ipopt_adapter::eval_g),
            Some(ipopt_adapter::eval_grad_f),
            Some(ipopt_adapter::eval_jac_g),
            Some(ipopt_adapter::eval_h),
            user_data as *mut c_void,
        )
    };

    if ipopt_prob.is_null() {
        let _ = unsafe { Box::from_raw(user_data) };
        return Err(ComputationError::MathError("Failed to create IPOPT problem".into()));
    }

    unsafe {
        // [Options setup omitted for brevity, same as original]
        ipopt_ffi::AddIpoptIntOption(ipopt_prob, "print_level\0".as_ptr() as *const i8, 0);
        ipopt_ffi::AddIpoptNumOption(ipopt_prob, "tol\0".as_ptr() as *const i8, 1e-9);
        ipopt_ffi::SetIntermediateCallback(ipopt_prob, Some(ipopt_adapter::intermediate_callback));
        
        ipopt_ffi::IpoptSolve(
            ipopt_prob,
            x_init.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            user_data as *mut c_void,
        );
        ipopt_ffi::FreeIpoptProblem(ipopt_prob);
    }
    
    let solved_problem = unsafe { Box::from_raw(user_data) };
    let final_x = x_init;
    
    // Reconstruct final ledger
    let mut final_ledger = solved_problem.base_ledger.clone();
    for (i, &vid) in solved_problem.variables.iter().enumerate() {
        let start = i * model_len;
        let val = &final_x[start..start + model_len];
        final_ledger.set_input(vid, val)?;
    }
    
    if let Ok(hist) = solved_problem.iteration_history.into_inner() {
        final_ledger.solver_trace = Some(hist);
    }
    
    // Final Compute pass to ensure downstream consistency
    Engine::run(program, &mut final_ledger)?;

    Ok(final_ledger)
}