use crate::store::{Registry, NodeId, NodeKind, NodeMetadata, Operation, TemporalType, Unit};
use crate::compute::{engine::Engine, ledger::Ledger};
use crate::analysis::{topology, validation};
use crate::display::trace;
use crate::solver::optimizer;
use pyo3::prelude::*;
use pyo3::exceptions::{PyValueError, PyRuntimeError};
use std::time::Instant;

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
        self.inner.get(NodeId::new(node_id))
            .and_then(|r| r.ok())
            .map(|v| v.to_vec())
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

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self { Self { state: seed } }
    
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    fn next_u32_range(&mut self, max: u32) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.state >> 32) as u32) % max
    }
}

#[pyfunction]
pub fn benchmark_pure_rust(num_nodes: usize, input_fraction: f64) -> PyResult<(f64, f64, f64, usize)> {
    let mut registry = Registry::new();
    let num_inputs = (num_nodes as f64 * input_fraction) as usize;
    let mut rng = Lcg::new(42);

    let start_gen = Instant::now();

    // 1. Generate Inputs
    for i in 0..num_inputs {
        let val = rng.next_f64() * 100.0;
        let meta = NodeMetadata { name: format!("Input_{}", i), ..Default::default() };
        registry.add_node(NodeKind::Scalar(val), &[], meta);
    }

    // 2. Generate Formulas
    for i in num_inputs..num_nodes {
        let p1 = rng.next_u32_range(i as u32);
        let p2 = rng.next_u32_range(i as u32);
        
        let op = match rng.next_u32_range(3) {
            0 => Operation::Add,
            1 => Operation::Subtract,
            _ => Operation::Multiply,
        };

        let parents = vec![NodeId::new(p1 as usize), NodeId::new(p2 as usize)];
        let meta = NodeMetadata { name: format!("Formula_{}", i), ..Default::default() };
        registry.add_node(NodeKind::Formula(op), &parents, meta);
    }
    
    let gen_duration = start_gen.elapsed().as_secs_f64();

    // 3. Benchmark Full Compute
    let mut ledger = Ledger::new();
    let targets: Vec<NodeId> = (0..registry.count()).map(NodeId::new).collect();
    
    // SCOPED ENGINE: Ensure borrow of registry ends before mutation
    let compute_duration = {
        let engine = Engine::new(&registry);
        let start_compute = Instant::now();
        engine.compute(&targets, &mut ledger)
            .map_err(|e| PyRuntimeError::new_err(format!("Full compute failed: {:?}", e)))?;
        start_compute.elapsed().as_secs_f64()
    };

    // 4. Benchmark Incremental Recompute
    let num_changes = 5;
    let mut changed_ids = Vec::with_capacity(num_changes);
    
    for _ in 0..num_changes {
        let idx = rng.next_u32_range(num_inputs as u32) as usize;
        match &mut registry.kinds[idx] {
            NodeKind::Scalar(v) => *v = rng.next_f64() * 100.0,
            _ => {}
        }
        changed_ids.push(NodeId::new(idx));
    }

    let start_incr = Instant::now();
    let dirty = topology::downstream_from(&registry, &changed_ids);
    ledger.invalidate(dirty);

    // Create NEW engine instance for the incremental pass
    let engine = Engine::new(&registry);
    engine.compute(&targets, &mut ledger)
        .map_err(|e| PyRuntimeError::new_err(format!("Incremental compute failed: {:?}", e)))?;

    let incr_duration = start_incr.elapsed().as_secs_f64();

    Ok((gen_duration, compute_duration, incr_duration, num_nodes))
}