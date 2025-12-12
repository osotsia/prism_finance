use super::problem::PrismProblem;
use crate::compute::{engine::Engine, ledger::{Ledger, ComputationError, SolverIteration}};
use libc::{c_int, c_void};
use std::slice;

type Number = f64;
type Index = c_int;
type Bool = c_int;

unsafe fn get_prob<'a>(user_data: *mut c_void) -> &'a mut PrismProblem<'a> {
    &mut *(user_data as *mut PrismProblem)
}

// Writes directly into the flat buffer
fn eval_graph(prob: &PrismProblem, x: &[f64]) -> Result<Ledger, ComputationError> {
    let mut ledger = prob.base_ledger.clone();
    let len = prob.model_len;

    for (i, &var_id) in prob.variables.iter().enumerate() {
        let start = i * len;
        ledger.set_input(var_id, &x[start..start + len])?;
    }

    Engine::run(prob.program, &mut ledger)?;
    Ok(ledger)
}

pub extern "C" fn eval_f(_n: Index, _x: *mut Number, _new_x: Bool, obj: *mut Number, _u: *mut c_void) -> Bool {
    unsafe { *obj = 0.0; } 1
}

pub extern "C" fn eval_grad_f(n: Index, _x: *mut Number, _new_x: Bool, grad: *mut Number, _u: *mut c_void) -> Bool {
    let sl = unsafe { slice::from_raw_parts_mut(grad, n as usize) };
    sl.fill(0.0); 1
}

pub extern "C" fn eval_g(n: Index, x: *mut Number, _new_x: Bool, m: Index, g: *mut Number, user_data: *mut c_void) -> Bool {
    let prob = unsafe { get_prob(user_data) };
    let x_sl = unsafe { slice::from_raw_parts(x, n as usize) };
    let g_sl = unsafe { slice::from_raw_parts_mut(g, m as usize) };

    match eval_graph(prob, x_sl) {
        Ok(led) => {
            for (i, &rid) in prob.residuals.iter().enumerate() {
                if let Some(val) = led.get(rid) {
                    let start = i * prob.model_len;
                    g_sl[start..start + prob.model_len].copy_from_slice(val);
                } else {
                    return 0;
                }
            }
            1
        },
        Err(_) => 0
    }
}

pub extern "C" fn eval_jac_g(
    n: Index, x: *mut Number, _new: Bool, m: Index, nele: Index, 
    i_row: *mut Index, j_col: *mut Index, values: *mut Number, user_data: *mut c_void
) -> Bool {
    if values.is_null() {
        // Fill sparsity (dense)
        let i_sl = unsafe { slice::from_raw_parts_mut(i_row, nele as usize) };
        let j_sl = unsafe { slice::from_raw_parts_mut(j_col, nele as usize) };
        let mut idx = 0;
        for r in 0..m { for c in 0..n { i_sl[idx] = r; j_sl[idx] = c; idx += 1; } }
        return 1;
    }
    
    // Finite difference Jacobian
    let n_usize = n as usize;
    let m_usize = m as usize;
    let val_sl = unsafe { slice::from_raw_parts_mut(values, nele as usize) };
    let x_base = unsafe { slice::from_raw_parts(x, n_usize) };
    let mut x_mut = x_base.to_vec();
    let prob = unsafe { get_prob(user_data) };
    
    let h = 1e-8;
    let mut jac_idx = 0;

    for i in 0..m_usize {
        for j in 0..n_usize {
            let orig = x_mut[j];
            
            x_mut[j] = orig + h;
            let res_p = eval_single_residual(prob, &x_mut, i);
            
            x_mut[j] = orig - h;
            let res_m = eval_single_residual(prob, &x_mut, i);
            
            x_mut[j] = orig;
            
            if let (Ok(p), Ok(m)) = (res_p, res_m) {
                val_sl[jac_idx] = (p - m) / (2.0 * h);
            } else { return 0; }
            jac_idx += 1;
        }
    }
    1
}

fn eval_single_residual(prob: &PrismProblem, x: &[f64], constraint_idx: usize) -> Result<f64, ()> {
    // Flattened index mapping: constraint_idx corresponds to (ResidualNode, TimeStep)
    let residual_node_idx = constraint_idx / prob.model_len; 
    let step_idx = constraint_idx % prob.model_len;

    let residual_node = prob.residuals[residual_node_idx];
    let led = eval_graph(prob, x).map_err(|_| ())?;
    
    let val = led.get(residual_node)
        .ok_or(())? 
        .get(step_idx)
        .ok_or(())?;
    
    Ok(*val)
}

pub extern "C" fn eval_h(_n: Index, _x: *mut Number, _new: Bool, _obj: Number, _m: Index, _lam: *mut Number, _new_l: Bool, _nele: Index, _i: *mut Index, _j: *mut Index, _v: *mut Number, _u: *mut c_void) -> Bool { 
    1 
}

pub extern "C" fn intermediate_callback(
    _alg: Index, iter: Index, obj: Number, inf_pr: Number, inf_du: Number, 
    _mu: Number, _dn: Number, _rs: Number, _adu: Number, _apr: Number, _ls: Index, 
    user_data: *mut c_void
) -> Bool {
    let prob = unsafe { get_prob(user_data) };
    if let Ok(mut hist) = prob.iteration_history.lock() {
        hist.push(SolverIteration { iter_count: iter, obj_value: obj, inf_pr, inf_du });
    }
    1
}