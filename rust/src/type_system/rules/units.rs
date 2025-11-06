//! Rule for dimensional analysis and unit inference.
use crate::graph::{NodeMetadata, Operation, Unit};
use crate::type_system::error::{ValidationError, ValidationErrorType};
use std::collections::HashSet;

// ... (ParsedUnit struct and helper functions remain the same) ...

/// Infers the unit of a formula or returns a validation error.
/// Signature updated from `&[&NodeMetadata]` to `&[NodeMetadata]`.
pub(crate) fn infer_and_validate(
    op: &Operation,
    parents: &[NodeMetadata],
) -> Result<Option<Unit>, ValidationError> {
    let parent_units: Vec<_> = parents.iter().filter_map(|m| m.unit.as_ref()).collect();
    if parent_units.is_empty() { return Ok(None); }

    match op {
        Operation::Add | Operation::Subtract => {
            let first_unit = &parent_units[0].0;
            if parent_units.iter().all(|u| &u.0 == first_unit) {
                Ok(Some(Unit(first_unit.clone())))
            } else {
                Err(ValidationError {
                    node_id: Default::default(),
                    error_type: ValidationErrorType::UnitMismatch,
                    message: format!("Unit Mismatch: Addition/subtraction requires all units to be identical, but found mixed units."),
                })
            }
        }
        Operation::Multiply => {
            let mut res = ParsedUnit::default();
            for unit in parent_units {
                let p = ParsedUnit::from_str(&unit.0);
                res.num.extend(&p.num);
                res.den.extend(&p.den);
            }
            cancel_units(&mut res.num, &mut res.den);
            Ok(Some(Unit(res.to_string())))
        }
        Operation::Divide => {
            if parents.len() != 2 { return Ok(None); } // Only support binary division for now
            let p1 = ParsedUnit::from_str(&parents[0].unit.as_ref().unwrap().0);
            let p2 = ParsedUnit::from_str(&parents[1].unit.as_ref().unwrap().0);
            
            let mut res = p1;
            res.num.extend(&p2.den);
            res.den.extend(&p2.num);
            cancel_units(&mut res.num, &mut res.den);
            Ok(Some(Unit(res.to_string())))
        }
        _ => Ok(None),
    }
}

// ... (ParsedUnit and cancel_units implementation is the same, but a small correction in Divide logic)

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedUnit<'a> {
    num: HashSet<&'a str>,
    den: HashSet<&'a str>,
}

impl<'a> ParsedUnit<'a> {
    fn from_str(s: &'a str) -> Self {
        let (num_str, den_str) = s.split_once('/').unwrap_or((s, ""));
        let num = num_str.split('*').filter(|p| !p.is_empty()).collect();
        let den = den_str.split('*').filter(|p| !p.is_empty()).collect();
        Self { num, den }
    }

    fn to_string(&self) -> String {
        if self.num.is_empty() && self.den.is_empty() { return "".into(); }
        let mut num_vec: Vec<_> = self.num.iter().copied().collect();
        num_vec.sort();
        let num_str = if num_vec.is_empty() { "1".to_string() } else { num_vec.join("*") };

        if self.den.is_empty() { return num_str; }
        let mut den_vec: Vec<_> = self.den.iter().copied().collect();
        den_vec.sort();
        format!("{}/{}", num_str, den_vec.join("*"))
    }
}


fn cancel_units<'a>(num: &mut HashSet<&'a str>, den: &mut HashSet<&'a str>) {
    let common: HashSet<_> = num.intersection(den).cloned().collect();
    for term in common {
        num.remove(term);
        den.remove(term);
    }
}