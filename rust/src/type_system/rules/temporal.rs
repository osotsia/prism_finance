//! Rule for inferring and validating temporal consistency (Stock vs. Flow).
use crate::graph::{NodeMetadata, Operation, TemporalType};
use crate::type_system::error::{ValidationError, ValidationErrorType};

/// Infers the temporal type of a formula or returns a validation error.
/// Signature updated from `&[&NodeMetadata]` to `&[NodeMetadata]`.
pub(crate) fn infer_and_validate(
    op: &Operation,
    parents: &[NodeMetadata],
) -> Result<Option<TemporalType>, ValidationError> {
    match op {
        Operation::Add | Operation::Subtract => {
            let stock_parents: Vec<_> = parents
                .iter()
                .filter(|m| m.temporal_type == Some(TemporalType::Stock))
                .collect();
            let flow_parents: Vec<_> = parents
                .iter()
                .filter(|m| m.temporal_type == Some(TemporalType::Flow))
                .collect();

            if stock_parents.len() > 1 {
                Err(ValidationError {
                    node_id: Default::default(), // Orchestrator will set this.
                    error_type: ValidationErrorType::TemporalMismatch,
                    message: "Temporal Error: Cannot add or subtract two 'Stock' variables.".into(),
                })
            } else if stock_parents.len() == 1 && !flow_parents.is_empty() {
                // Stock + Flow -> Stock
                Ok(Some(TemporalType::Stock))
            } else if !flow_parents.is_empty() {
                // Flow + Flow -> Flow
                Ok(Some(TemporalType::Flow))
            } else {
                // No typed parents, so no inferred type.
                Ok(None)
            }
        }
        Operation::Multiply | Operation::Divide => {
            if parents.iter().all(|p| p.temporal_type == Some(TemporalType::Flow)) {
                Ok(Some(TemporalType::Flow))
            } else if parents.iter().any(|p| p.temporal_type.is_some()) {
                Err(ValidationError {
                    node_id: Default::default(),
                    error_type: ValidationErrorType::TemporalMismatch,
                    message: "Temporal Error: Multiplication or division involving 'Stock' types is not supported.".into(),
                })
            } else {
                Ok(None)
            }
        }
        Operation::PreviousValue { .. } => {
            Ok(parents[0].temporal_type.clone())
        }
    }
}