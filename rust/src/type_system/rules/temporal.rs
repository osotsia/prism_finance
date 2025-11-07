//! Rule for inferring and validating temporal consistency (Stock vs. Flow).
use crate::graph::{NodeMetadata, Operation, TemporalType};
use crate::type_system::error::{ValidationError, ValidationErrorType};

/// Infers the temporal type of a formula or returns a validation error.
pub(crate) fn infer_and_validate(
    op: &Operation,
    parents: &[NodeMetadata],
) -> Result<Option<TemporalType>, ValidationError> {
    match op {
        Operation::Add | Operation::Subtract => {
            let stock_count = parents
                .iter()
                .filter(|m| m.temporal_type == Some(TemporalType::Stock))
                .count();

            let flow_count = parents
                .iter()
                .filter(|m| m.temporal_type == Some(TemporalType::Flow))
                .count();

            if stock_count > 1 {
                // Disallow Stock + Stock, as it is ambiguous.
                Err(ValidationError {
                    node_id: Default::default(), // Orchestrator will set this.
                    node_name: String::new(),
                    error_type: ValidationErrorType::TemporalMismatch,
                    message: "Temporal Error: Addition or subtraction of more than one 'Stock' type is ambiguous and not permitted.".into(),
                })
            } else if stock_count == 1 {
                // Stock + Flow -> Stock (e.g., Opening Balance + Net Income -> Closing Balance).
                Ok(Some(TemporalType::Stock))
            } else if flow_count > 0 {
                // Flow + Flow -> Flow.
                Ok(Some(TemporalType::Flow))
            } else {
                // No typed parents, so no inferred type.
                Ok(None)
            }
        }
        Operation::Multiply | Operation::Divide => {
            // Multiplication/division is generally only well-defined for flows (e.g., Revenue * Margin).
            // A Stock in this context is conceptually ambiguous and not supported.
            if parents
                .iter()
                .any(|p| p.temporal_type == Some(TemporalType::Stock))
            {
                Err(ValidationError {
                    node_id: Default::default(), // Orchestrator will set this.
                    node_name: String::new(),
                    error_type: ValidationErrorType::TemporalMismatch,
                    message: "Temporal Error: Multiplication or division involving 'Stock' types is not supported.".into(),
                })
            } else if parents
                .iter()
                .any(|p| p.temporal_type == Some(TemporalType::Flow))
            {
                Ok(Some(TemporalType::Flow))
            } else {
                Ok(None)
            }
        }
        Operation::PreviousValue { .. } => {
            // .prev() inherits the temporal type of its main parent.
            Ok(parents[0].temporal_type.clone())
        }
    }
}