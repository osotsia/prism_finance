//! Rule for dimensional analysis and unit inference.
use crate::graph::{NodeMetadata, Operation, Unit};
use crate::type_system::error::{ValidationError, ValidationErrorType};
use std::collections::{BTreeMap, HashMap};

/// A parsed representation of a unit, mapping each base unit to its exponent.
/// Example: "kg*m/s^2" -> { "kg": 1, "m": 1, "s": -2 }
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedUnit {
    terms: HashMap<String, i32>,
}

impl ParsedUnit {
    /// Parses a string slice into a `ParsedUnit` using a manual parser.
    fn from_str(s: &str) -> Result<Self, ()> {
        let mut terms = HashMap::new();
        let mut parts = s.split('/');
        
        // Numerator
        if let Some(num_str) = parts.next() {
            Self::parse_product(num_str, 1, &mut terms)?;
        }

        // Denominator
        if let Some(den_str) = parts.next() {
            Self::parse_product(den_str, -1, &mut terms)?;
        }
        
        // Ensure no more than one '/' was present
        if parts.next().is_some() {
            return Err(());
        }

        Ok(Self { terms })
    }

    /// Helper to parse a product of factors (e.g., "kg*m^2").
    fn parse_product(product_str: &str, sign: i32, terms: &mut HashMap<String, i32>) -> Result<(), ()> {
        if product_str.trim().is_empty() || product_str == "1" {
            return Ok(());
        }

        for factor_str in product_str.split('*') {
            let mut factor_parts = factor_str.split('^');
            let base = factor_parts.next().ok_or(())?.trim();
            if base.is_empty() { return Err(()); }

            let exponent = match factor_parts.next() {
                Some(exp_str) => exp_str.parse::<i32>().map_err(|_| ())?,
                None => 1,
            };
            
            *terms.entry(base.to_string()).or_insert(0) += exponent * sign;
        }
        Ok(())
    }

    /// Merges another `ParsedUnit` into this one, effectively multiplying them.
    fn multiply_by(&mut self, other: &Self) {
        for (base, exponent) in &other.terms {
            *self.terms.entry(base.clone()).or_insert(0) += exponent;
        }
    }

    /// Merges another `ParsedUnit` into this one, effectively dividing by it.
    fn divide_by(&mut self, other: &Self) {
        for (base, exponent) in &other.terms {
            *self.terms.entry(base.clone()).or_insert(0) -= exponent;
        }
    }

    /// Formats the `ParsedUnit` back into a canonical string representation.
    fn to_string(&self) -> String {
        // Partition terms into numerator (exp > 0) and denominator (exp < 0).
        let (num_terms, den_terms): (Vec<_>, Vec<_>) = self
            .terms
            .iter()
            .filter(|&(_, &exp)| exp != 0)
            .partition(|&(_, &exp)| exp > 0);

        // Helper to format a single term like "m" or "s^2".
        let format_term = |(base, &exp): (&String, &i32)| {
            if exp.abs() == 1 { base.clone() } else { format!("{}^{}", base, exp.abs()) }
        };

        // Helper to format and join a list of terms.
        let format_product = |terms: Vec<(&String, &i32)>| -> String {
            if terms.is_empty() { return "1".to_string(); }
            let mut sorted_terms = terms;
            sorted_terms.sort_by_key(|&(base, _)| base);
            sorted_terms.into_iter().map(format_term).collect::<Vec<_>>().join("*")
        };

        let num_str = format_product(num_terms);
        let den_str = format_product(den_terms);
        
        if den_str == "1" {
            if num_str == "1" { "".to_string() } else { num_str } // Dimensionless
        } else {
            format!("{}/{}", num_str, den_str)
        }
    }
}


/// Infers the unit of a formula or returns a validation error.
pub(crate) fn infer_and_validate(
    op: &Operation,
    parents: &[NodeMetadata],
) -> Result<Option<Unit>, ValidationError> {
    let parent_units: Vec<_> = parents.iter().filter_map(|m| m.unit.as_ref()).collect();
    if parent_units.is_empty() {
        return Ok(None);
    }

    match op {
        Operation::Add | Operation::Subtract => {
            let first_unit = &parent_units[0];
            if parent_units.iter().all(|u| u == first_unit) {
                Ok(Some((*first_unit).clone()))
            } else {
                Err(ValidationError {
                    node_id: Default::default(),
                    node_name: String::new(),
                    error_type: ValidationErrorType::UnitMismatch,
                    message: "Unit Mismatch: Addition/subtraction requires all units to be identical.".into(),
                })
            }
        }
        Operation::Multiply => {
            // Use try_fold to accumulate units, short-circuiting if any unit is unparsable.
            let result = parent_units.iter().try_fold(ParsedUnit::default(), |mut acc, unit| {
                let parsed = ParsedUnit::from_str(&unit.0)?;
                acc.multiply_by(&parsed);
                Ok::<_, ()>(acc)
            });
            
            match result {
                Ok(final_unit) => Ok(Some(Unit(final_unit.to_string()))),
                Err(_) => Ok(None), // Fail gracefully
            }
        }
        Operation::Divide => {
            if parents.len() != 2 { return Ok(None); } // Only support binary division
            let num_unit_str = parents[0].unit.as_ref().map_or("", |u| u.0.as_str());
            let den_unit_str = parents[1].unit.as_ref().map_or("", |u| u.0.as_str());
            
            match (ParsedUnit::from_str(num_unit_str), ParsedUnit::from_str(den_unit_str)) {
                (Ok(mut num_unit), Ok(den_unit)) => {
                    num_unit.divide_by(&den_unit);
                    Ok(Some(Unit(num_unit.to_string())))
                }
                _ => Ok(None), // Fail gracefully if units are unparsable
            }
        }
        Operation::PreviousValue { .. } => {
            Ok(parents[0].unit.clone())
        }
    }
}

// --- Unit Parser Test Suite ---
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("USD", "USD")]
    #[case("m*s", "m*s")] // Alphabetical order is canonical
    #[case("s*m", "m*s")] // Test canonical reordering
    #[case("m/s", "m/s")]
    #[case("m/s^2", "m/s^2")]
    #[case("kg*m/s^2", "kg*m/s^2")]
    #[case("m*m", "m^2")] // Test aggregation
    #[case("m^2/m", "m")] // Test cancellation
    #[case("m/m", "")] // Test full cancellation to dimensionless
    #[case("USD*h/h", "USD")]
    #[case("1/s", "1/s")]
    #[case("", "")]
    #[case("m^1", "m")]
    fn test_unit_parsing_and_canonicalization(#[case] input: &str, #[case] expected: &str) {
        let parsed = ParsedUnit::from_str(input).unwrap();
        assert_eq!(parsed.to_string(), expected);
    }
    
    #[test]
    fn test_unit_multiplication() {
        let mut u1 = ParsedUnit::from_str("kg*m").unwrap();
        let u2 = ParsedUnit::from_str("m/s^2").unwrap();
        u1.multiply_by(&u2);
        assert_eq!(u1.to_string(), "kg*m^2/s^2");
    }

    #[test]
    fn test_unit_division() {
        let mut u1 = ParsedUnit::from_str("m/s").unwrap();
        let u2 = ParsedUnit::from_str("s").unwrap();
        u1.divide_by(&u2);
        assert_eq!(u1.to_string(), "m/s^2");
    }
}