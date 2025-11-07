//! Rule for inferring and validating temporal consistency (Stock vs. Flow).
use crate::graph::{NodeMetadata, Operation, TemporalType};
use crate::type_system::error::{ValidationError, ValidationErrorType};

/// Infers the temporal type of a formula or returns a validation error.
/// This rule is now more permissive, allowing Stock + Stock operations.
pub(crate) fn infer_and_validate(
    op: &Operation,
    parents: &[NodeMetadata],
) -> Result<Option<TemporalType>, ValidationError> {
    match op {
        Operation::Add | Operation::Subtract => {
            let has_stock = parents
                .iter()
                .any(|m| m.temporal_type == Some(TemporalType::Stock));
            let has_flow = parents
                .iter()
                .any(|m| m.temporal_type == Some(TemporalType::Flow));

            if has_stock {
                // Any operation involving a Stock results in a Stock (e.g., Balance + P&L Item -> New Balance).
                Ok(Some(TemporalType::Stock))
            } else if has_flow {
                // If there are no Stocks but there are Flows, the result is a Flow.
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