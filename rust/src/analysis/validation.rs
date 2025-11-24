use crate::store::{Registry, NodeId, NodeKind, Operation, TemporalType, Unit};
use super::units::ParsedUnit;
use super::topology;

#[derive(Debug)]
pub struct ValidationError {
    pub node_name: String,
    pub message: String,
}

pub fn validate(registry: &Registry) -> Result<(), Vec<ValidationError>> {
    let order = topology::sort(registry).map_err(|e| vec![ValidationError { node_name: "Graph".into(), message: e }])?;
    let mut errors = Vec::new();
    
    // Cache inferred types: NodeId -> (Temporal, Unit)
    let mut inference_cache = vec![None; registry.count()];

    for node in order {
        let idx = node.index();
        let meta = &registry.meta[idx];
        let kind = &registry.kinds[idx];
        
        // 1. Infer
        let (inf_temp, inf_unit) = match kind {
            NodeKind::Scalar(_) | NodeKind::TimeSeries(_) | NodeKind::SolverVariable => {
                (meta.temporal_type.clone(), meta.unit.as_ref().map(|u| u.0.clone()))
            },
            NodeKind::Formula(op) => {
                let parents = registry.get_parents(node);
                // Retrieve parents' inferred metadata
                let p_metas: Vec<_> = parents.iter().map(|p| inference_cache[p.index()].clone().unwrap_or_default()).collect();
                
                let t = infer_temporal(op, &p_metas, &meta.name, &mut errors);
                // UPDATED: Pass errors vector and name to infer_unit
                let u = infer_unit(op, &p_metas, &meta.name, &mut errors); 
                (t, u)
            }
        };

        // 2. Validate against declaration
        if let Some(decl) = &meta.temporal_type {
            if let Some(inf) = &inf_temp {
                if decl != inf {
                    errors.push(ValidationError { node_name: meta.name.clone(), message: format!("Declared {:?} != Inferred {:?}", decl, inf) });
                }
            }
        }
        if let Some(decl) = &meta.unit {
            if let Some(inf) = &inf_unit {
                if &decl.0 != inf {
                     errors.push(ValidationError { node_name: meta.name.clone(), message: format!("Declared unit {} != Inferred unit {}", decl.0, inf) });
                }
            }
        }

        inference_cache[idx] = Some((inf_temp, inf_unit));
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn infer_temporal(op: &Operation, parents: &[(Option<TemporalType>, Option<String>)], name: &str, errs: &mut Vec<ValidationError>) -> Option<TemporalType> {
    use TemporalType::*;
    match op {
        Operation::Add | Operation::Subtract => {
            let stocks = parents.iter().filter(|(t, _)| matches!(t, Some(Stock))).count();
            let flows = parents.iter().filter(|(t, _)| matches!(t, Some(Flow))).count();
            if stocks > 1 { 
                errs.push(ValidationError { node_name: name.into(), message: "Ambiguous: Stock +/- Stock".into() });
                None
            } else if stocks == 1 { Some(Stock) }
            else if flows > 0 { Some(Flow) }
            else { None }
        }
        Operation::Multiply | Operation::Divide => {
            if parents.iter().any(|(t, _)| matches!(t, Some(Stock))) {
                errs.push(ValidationError { node_name: name.into(), message: "Invalid: Stock * or /".into() });
                None
            } else { Some(Flow) }
        }
        Operation::PreviousValue { .. } => parents.first()?.0.clone(),
    }
}

fn infer_unit(
    op: &Operation, 
    parents: &[(Option<TemporalType>, Option<String>)],
    name: &str,
    errs: &mut Vec<ValidationError>
) -> Option<String> {
    match op {
        Operation::Add | Operation::Subtract => {
            // UPDATED: Validate homogeneity
            let units: Vec<&String> = parents.iter().filter_map(|p| p.1.as_ref()).collect();
            if units.is_empty() { return None; }
            
            let first = units[0];
            for u in &units[1..] {
                if *u != first {
                    errs.push(ValidationError { 
                        node_name: name.into(), 
                        message: format!("Unit Mismatch: Cannot add/sub '{}' and '{}'", first, u) 
                    });
                    return None;
                }
            }
            Some(first.clone())
        },
        Operation::Multiply => {
            let mut acc = ParsedUnit::default();
            for (_, u) in parents { 
                if let Some(s) = u { if let Ok(p) = ParsedUnit::from_str(s) { acc.multiply(&p); } } 
            }
            Some(acc.to_string())
        }
        Operation::Divide => {
            if parents.len() != 2 { return None; }
            let mut n = ParsedUnit::from_str(parents[0].1.as_deref()?).ok()?;
            let d = ParsedUnit::from_str(parents[1].1.as_deref()?).ok()?;
            n.divide(&d);
            Some(n.to_string())
        }
        Operation::PreviousValue { .. } => parents.first()?.1.clone(),
    }
}