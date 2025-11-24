use crate::store::{Registry, NodeId, NodeKind, NodeMetadata, Operation, TemporalType, Unit};
use crate::compute::{engine::Engine, ledger::Ledger};
use crate::analysis::{topology, validation};
use crate::display::trace;
use crate::solver::optimizer;
use pyo3::prelude::*;
use pyo3::exceptions::{PyValueError, PyRuntimeError};

#[pyclass(name = "_Ledger")]
#[derive(Debug, Clone, Default)]
pub struct PyLedger {
    pub inner: Ledger,
}

#[pymethods]
impl PyLedger {
    #[new]
    pub fn new() -> Self { Self::default() }
    
    pub fn get_value(&self, node_id: usize) -> Option<Vec<f64>> {
        self.inner.get(NodeId::new(node_id)).and_then(|r| r.as_ref().ok()).map(|v| v.to_vec())
    }
}

#[pyclass(name = "_ComputationGraph")]
#[derive(Debug, Clone, Default)]
pub struct PyComputationGraph {
    registry: Registry,
    constraints: Vec<(NodeId, String)>,
}

#[pymethods]
impl PyComputationGraph {
    #[new]
    pub fn new() -> Self { Self::default() }

    pub fn add_constant_node(&mut self, value: Vec<f64>, name: String, unit: Option<String>, temporal_type: Option<String>) -> PyResult<usize> {
        let meta = NodeMetadata {
            name,
            unit: unit.map(Unit),
            temporal_type: temporal_type.map(|t| if t == "Stock" { TemporalType::Stock } else { TemporalType::Flow }),
        };
        let kind = if value.len() == 1 { NodeKind::Scalar(value[0]) } else { 
            let idx = self.registry.constants_data.len() as u32;
            self.registry.constants_data.push(value);
            NodeKind::TimeSeries(idx)
        };
        Ok(self.registry.add_node(kind, &[], meta).index())
    }

    pub fn add_binary_formula(&mut self, op_name: &str, parents: Vec<usize>, name: String) -> PyResult<usize> {
        let op = match op_name {
            "add" => Operation::Add, "subtract" => Operation::Subtract,
            "multiply" => Operation::Multiply, "divide" => Operation::Divide,
            _ => return Err(PyValueError::new_err("Invalid Op")),
        };
        let p_ids: Vec<NodeId> = parents.into_iter().map(NodeId::new).collect();
        let meta = NodeMetadata { name, ..Default::default() };
        Ok(self.registry.add_node(NodeKind::Formula(op), &p_ids, meta).index())
    }
    
    pub fn add_formula_previous_value(&mut self, main: usize, def: usize, lag: u32, name: String) -> usize {
        let op = Operation::PreviousValue { lag, default_node: NodeId::new(def) };
        let p = vec![NodeId::new(main), NodeId::new(def)];
        self.registry.add_node(NodeKind::Formula(op), &p, NodeMetadata { name, ..Default::default() }).index()
    }
    
    pub fn add_solver_variable(&mut self, name: String) -> usize {
        self.registry.add_node(NodeKind::SolverVariable, &[], NodeMetadata { name, ..Default::default() }).index()
    }

    pub fn must_equal(&mut self, lhs: usize, rhs: usize, name: String) {
        let p = vec![NodeId::new(lhs), NodeId::new(rhs)];
        let resid = self.registry.add_node(
            NodeKind::Formula(Operation::Subtract), 
            &p, 
            NodeMetadata { name: format!("Residual: {}", name), ..Default::default() }
        );
        self.constraints.push((resid, name));
    }

    pub fn set_node_name(&mut self, id: usize, name: String) -> PyResult<()> {
        if id < self.registry.count() { self.registry.meta[id].name = name; Ok(()) } 
        else { Err(PyValueError::new_err("Invalid Node ID")) }
    }
    
    pub fn update_constant_node(&mut self, id: usize, val: Vec<f64>) -> PyResult<()> {
        if id >= self.registry.count() { return Err(PyValueError::new_err("Invalid Node ID")); }
        match &mut self.registry.kinds[id] {
            NodeKind::Scalar(s) => if val.len() == 1 { *s = val[0]; Ok(()) } else { Err(PyValueError::new_err("Cannot change scalar to vector")) },
            NodeKind::TimeSeries(idx) => { self.registry.constants_data[*idx as usize] = val; Ok(()) },
            _ => Err(PyValueError::new_err("Not a constant"))
        }
    }
    
    pub fn set_node_metadata(&mut self, id: usize, unit: Option<String>, temporal_type: Option<String>) -> PyResult<(Option<String>, Option<String>)> {
        if id >= self.registry.count() { return Err(PyValueError::new_err("Invalid Node ID")); }
        let meta = &mut self.registry.meta[id];
        let old_u = meta.unit.as_ref().map(|u| u.0.clone());
        let old_t = meta.temporal_type.as_ref().map(|t| format!("{:?}", t));
        if let Some(u) = unit { meta.unit = Some(Unit(u)); }
        if let Some(t) = temporal_type { meta.temporal_type = Some(if t == "Stock" { TemporalType::Stock } else { TemporalType::Flow }); }
        Ok((old_u, old_t))
    }

    pub fn compute(&self, targets: Vec<usize>, ledger: &mut PyLedger, changed_inputs: Option<Vec<usize>>) -> PyResult<()> {
        if let Some(changes) = changed_inputs {
             let change_ids: Vec<NodeId> = changes.into_iter().map(NodeId::new).collect();
             let dirty = topology::downstream_from(&self.registry, &change_ids);
             ledger.inner.invalidate(dirty);
        }
        
        let t_ids: Vec<NodeId> = targets.into_iter().map(NodeId::new).collect();
        Engine::new(&self.registry).compute(&t_ids, &mut ledger.inner)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
    
    pub fn solve(&self) -> PyResult<PyLedger> {
        let vars: Vec<NodeId> = self.registry.kinds.iter().enumerate()
            .filter(|(_, k)| matches!(k, NodeKind::SolverVariable))
            .map(|(i, _)| NodeId::new(i))
            .collect();
        
        let residuals: Vec<NodeId> = self.constraints.iter().map(|c| c.0).collect();
        
        // Precompute independents
        let dependent_set = topology::downstream_from(&self.registry, &vars);
        let all_nodes: Vec<NodeId> = (0..self.registry.count()).map(NodeId::new).collect();
        let independents: Vec<NodeId> = all_nodes.iter().filter(|n| !dependent_set.contains(n)).cloned().collect();
        
        let mut base_ledger = Ledger::new();
        Engine::new(&self.registry).compute(&independents, &mut base_ledger)
             .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let result_ledger = optimizer::solve(&self.registry, vars, residuals, base_ledger)
             .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
             
        Ok(PyLedger { inner: result_ledger })
    }

    pub fn validate(&self) -> PyResult<()> {
        validation::validate(&self.registry)
            .map_err(|errs| {
                let msg = errs.iter().map(|e| format!("{}: {}", e.node_name, e.message)).collect::<Vec<_>>().join("\n");
                PyValueError::new_err(msg)
            })
    }
    
    pub fn trace_node(&self, node_id: usize, ledger: &PyLedger) -> String {
        trace::format_trace(&self.registry, &ledger.inner, NodeId::new(node_id), &self.constraints)
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        topology::sort(&self.registry)
            .map(|v| v.into_iter().map(|id| id.index()).collect())
            .map_err(|e| PyValueError::new_err(e))
    }
    
    pub fn node_count(&self) -> usize { self.registry.count() }
}