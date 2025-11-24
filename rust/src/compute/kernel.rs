use std::cmp::max;
use std::sync::Arc;
use crate::store::Operation;
use super::ledger::{Value, ComputationError};

/// Pure function: (Operation, Inputs) -> Result<Output>
#[inline]
pub fn execute(
    op: &Operation,
    inputs: &[&Value],
    node_name_debug: &str, // Passed only for error messaging
) -> Result<Value, ComputationError> {
    
    match op {
        Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide => {
            if inputs.len() != 2 {
                return Err(ComputationError::Mismatch { msg: format!("{} requires 2 inputs", node_name_debug) });
            }
            let (lhs, rhs) = (inputs[0], inputs[1]);

            // 1. Scalar Optimization (Fast Path)
            if let (Value::Scalar(l), Value::Scalar(r)) = (lhs, rhs) {
                 return match op {
                    Operation::Add => Ok(Value::Scalar(l + r)), 
                    Operation::Subtract => Ok(Value::Scalar(l - r)),
                    Operation::Multiply => Ok(Value::Scalar(l * r)),
                    Operation::Divide => {
                        if *r == 0.0 { Err(ComputationError::MathError("Division by zero".into())) } 
                        else { Ok(Value::Scalar(l / r)) }
                    },
                    _ => unreachable!(),
                 };
            }

            // 2. Vector Broadcasting (Slow Path)
            let (l_len, l_is_scalar) = lhs.shape();
            let (r_len, r_is_scalar) = rhs.shape();
            let len = max(l_len, r_len);
            
            let mut result = Vec::with_capacity(len);
            let l_val_s = if l_is_scalar { lhs.as_scalar_unchecked() } else { 0.0 };
            let r_val_s = if r_is_scalar { rhs.as_scalar_unchecked() } else { 0.0 };
            
            for i in 0..len {
                let l = if l_is_scalar { l_val_s } else { lhs.get_at(i) };
                let r = if r_is_scalar { r_val_s } else { rhs.get_at(i) };
                
                match op {
                    Operation::Add => result.push(l + r),
                    Operation::Subtract => result.push(l - r),
                    Operation::Multiply => result.push(l * r),
                    Operation::Divide => {
                        if r == 0.0 { return Err(ComputationError::MathError("Division by zero".into())); } 
                        else { result.push(l / r); }
                    },
                    _ => unreachable!(),
                }
            }
            Ok(Value::Series(Arc::new(result)))
        }

        Operation::PreviousValue { lag, .. } => {
            if inputs.len() != 2 { return Err(ComputationError::Mismatch { msg: "Prev requires 2 inputs".into() }); }
            let (main, def) = (inputs[0], inputs[1]);
            
            let len = max(main.len(), def.len()); 
            let mut result = Vec::with_capacity(len);
            let lag_u = *lag as usize;
            
            for i in 0..len {
                if i < lag_u {
                    result.push(def.get_at(i));
                } else {
                    result.push(main.get_at(i - lag_u));
                }
            }
            Ok(Value::Series(Arc::new(result)))
        }
    }
}