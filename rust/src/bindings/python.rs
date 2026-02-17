use crate::store::{Registry, NodeId, NodeKind, NodeMetadata, Operation, TemporalType, Unit};
use crate::compute::{engine::Engine, ledger::Ledger, bytecode::{Compiler, Program}};
use crate::analysis::{topology, validation};
use crate::display::trace;
use crate::solver::optimizer::{self, SolverConfig};
use pyo3::prelude::*;
use pyo3::exceptions::{PyValueError, PyRuntimeError};
use pyo3::types::{PyBytes};
use std::time::Instant;

use rayon::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Clone)]
pub struct PySolverConfig {
    #[pyo3(get, set)]
    pub tol: f64,
    #[pyo3(get, set)]
    pub max_iter: i32,
}

#[pymethods]
impl PySolverConfig {
    #[new]
    fn new(tol: Option<f64>, max_iter: Option<i32>) -> Self {
        Self {
            tol: tol.unwrap_or(1e-9),
            max_iter: max_iter.unwrap_or(3000),
        }
    }
}

impl From<PySolverConfig> for SolverConfig {
    fn from(py_conf: PySolverConfig) -> Self {
        SolverConfig { tol: py_conf.tol, max_iter: py_conf.max_iter }
    }
}

#[pyclass(name = "_Ledger")]
#[derive(Debug, Clone, Default)]
pub struct PyLedger {
    pub inner: Ledger,
}

#[pymethods]
impl PyLedger {
    #[new]
    pub fn new() -> Self { Self::default() }
}

#[pyclass(name = "_ComputationGraph")]
pub struct PyComputationGraph {
    registry: Registry,
    constraints: Vec<(NodeId, String)>,
    cached_program: Option<Program>,
}

/// Internal Rust methods (Not exposed to Python)
impl PyComputationGraph {
    fn invalidate_cache(&mut self) {
        self.cached_program = None;
    }

    fn check_bounds(&self, id: usize) -> PyResult<()> {
        if id >= self.registry.count() {
            Err(PyValueError::new_err(format!("Node ID {} out of bounds (count: {})", id, self.registry.count())))
        } else {
            Ok(())
        }
    }    
    fn determine_model_len(&self) -> PyResult<usize> {
        let mut len = 1;
        for vec in &self.registry.constants_data {
            if vec.len() > len { len = vec.len(); }
        }
        Ok(len)
    }

    fn ensure_compiled(&mut self) -> PyResult<()> {
        if self.cached_program.is_none() {
            let order = topology::sort(&self.registry).map_err(|e| PyValueError::new_err(e))?;
            let prog = Compiler::new(&self.registry).compile(order)
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            self.cached_program = Some(prog);
        }
        Ok(())
    }

    fn load_constants(&self, ledger: &mut Ledger, program: &Program, subset: Option<&[usize]>) -> PyResult<()> {
        let mut load_node = |id: usize| -> PyResult<()> {
            let node_id = NodeId::new(id);
            match &self.registry.kinds[id] {
                NodeKind::Scalar(v) => program.set_value(ledger, node_id, &[*v]),
                NodeKind::TimeSeries(idx) => program.set_value(ledger, node_id, &self.registry.constants_data[*idx as usize]),
                _ => Ok(()) 
            }.map_err(|e| PyRuntimeError::new_err(e.to_string()))
        };

        if let Some(indices) = subset {
            for &id in indices { load_node(id)?; }
        } else {
            for id in 0..self.registry.count() { load_node(id)?; }
        }
        Ok(())
    }

    fn check_is_scalar(&self, id: NodeId, cache: &mut Vec<Option<bool>>) -> bool {
        if let Some(res) = cache[id.index()] { return res; }
        let res = match &self.registry.kinds[id.index()] {
            NodeKind::Scalar(_) => true,
            NodeKind::TimeSeries(_) | NodeKind::SolverVariable => false,
            NodeKind::Formula(op) => {
                match op {
                    Operation::PreviousValue { .. } => false,
                    _ => {
                        let parents = self.registry.get_parents(id);
                        parents.iter().all(|p| self.check_is_scalar(*p, cache))
                    }
                }
            }
        };
        cache[id.index()] = Some(res);
        res
    }
}

#[pymethods]
impl PyComputationGraph {
    #[new]
    pub fn new() -> Self { 
        Self { 
            registry: Registry::new(),
            constraints: Vec::new(),
            cached_program: None,
        } 
    }

    // --- Serialization Support (Pickle/JSON) ---
    
    pub fn __getstate__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        // We serialize a tuple: (Registry, Constraints).
        // The Program (bytecode) is a derived artifact (cache) and is discarded.
        let state = (&self.registry, &self.constraints);
        
        let json_str = serde_json::to_string(&state)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))?;
            
        Ok(PyBytes::new(py, json_str.as_bytes()))
    }

    pub fn __setstate__(&mut self, state: &Bound<'_, PyBytes>) -> PyResult<()> {
        let bytes = state.as_bytes();
        
        // Deserialize the tuple (Registry, Constraints)
        let (mut registry, constraints): (Registry, Vec<(NodeId, String)>) = 
            serde_json::from_slice(bytes)
            .map_err(|e| PyRuntimeError::new_err(format!("Deserialization failed: {}", e)))?;
        
        // 1. Rebuild ephemeral state in Registry (used_names HashSet)
        registry.rebuild_name_cache();
        
        // 2. Validate Constraints Integrity
        // Ensures that loaded constraints reference valid nodes in the loaded registry.
        let count = registry.count();
        for (id, name) in &constraints {
            if id.index() >= count {
                return Err(PyRuntimeError::new_err(
                    format!("Corrupt state: Constraint '{}' references non-existent NodeId {}", name, id.index())
                ));
            }
        }

        // 3. Restore State
        self.registry = registry;
        self.constraints = constraints;
        
        // 4. Reset Cache to force recompilation on next compute/solve
        self.invalidate_cache();
        Ok(())
    }
    // --------------------------------------

    pub fn add_constant_node(&mut self, value: Vec<f64>, name: String, unit: Option<String>, temporal_type: Option<String>) -> PyResult<usize> {
        self.invalidate_cache();
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
        self.invalidate_cache();
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
        self.invalidate_cache();
        let op = Operation::PreviousValue { lag, default_node: NodeId::new(def) };
        let p = vec![NodeId::new(main), NodeId::new(def)];
        self.registry.add_node(NodeKind::Formula(op), &p, NodeMetadata { name, ..Default::default() }).index()
    }
    
    pub fn add_solver_variable(&mut self, name: String) -> usize {
        self.invalidate_cache();
        self.registry.add_node(NodeKind::SolverVariable, &[], NodeMetadata { name, ..Default::default() }).index()
    }

    pub fn must_equal(&mut self, lhs: usize, rhs: usize, name: String) -> PyResult<()> {
        self.check_bounds(lhs)?; // Added Safety
        self.check_bounds(rhs)?; // Added Safety
        self.invalidate_cache();
        let p = vec![NodeId::new(lhs), NodeId::new(rhs)];
        let resid = self.registry.add_node(
            NodeKind::Formula(Operation::Subtract), 
            &p, 
            NodeMetadata { name: format!("Residual: {}", name), ..Default::default() }
        );
        self.constraints.push((resid, name));
        Ok(())
    }

    pub fn update_constant_node(&mut self, id: usize, val: Vec<f64>) -> PyResult<()> {
        self.check_bounds(id)?; // Added Safety
        match &mut self.registry.kinds[id] {
            NodeKind::Scalar(s) => if val.len() == 1 { *s = val[0]; Ok(()) } else { Err(PyValueError::new_err("Cannot change scalar to vector")) },
            NodeKind::TimeSeries(idx) => { self.registry.constants_data[*idx as usize] = val; Ok(()) },
            _ => Err(PyValueError::new_err("Not a constant"))
        }
    }
    
    pub fn set_node_name(&mut self, id: usize, name: String) -> PyResult<()> {
        self.check_bounds(id)?; // Added Safety
        self.registry.meta[id].name = name; 
        Ok(()) 
    }

    pub fn set_node_metadata(&mut self, id: usize, unit: Option<String>, temporal_type: Option<String>) -> PyResult<(Option<String>, Option<String>)> {
        self.check_bounds(id)?; // Added Safety
        let meta = &mut self.registry.meta[id];
        let old_u = meta.unit.as_ref().map(|u| u.0.clone());
        let old_t = meta.temporal_type.as_ref().map(|t| format!("{:?}", t));
        if let Some(u) = unit { meta.unit = Some(Unit(u)); }
        if let Some(t) = temporal_type { meta.temporal_type = Some(if t == "Stock" { TemporalType::Stock } else { TemporalType::Flow }); }
        Ok((old_u, old_t))
    }

    pub fn compute(&mut self, ledger: &mut PyLedger, changed_inputs: Option<Vec<usize>>) -> PyResult<()> {
        self.ensure_compiled()?;
        let program = self.cached_program.as_ref().unwrap();
        let model_len = self.determine_model_len()?;
        
        ledger.inner.resize(self.registry.count(), model_len);
        self.load_constants(&mut ledger.inner, program, changed_inputs.as_deref())?;

        Engine::run(program, &mut ledger.inner)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
    
    pub fn get_value(&mut self, ledger: &PyLedger, node_id: usize) -> PyResult<Option<Vec<f64>>> {
        self.check_bounds(node_id)?; // Added Safety
        self.ensure_compiled()?;
        let program = self.cached_program.as_ref().unwrap();
        // Centralized retrieval logic
        Ok(program.get_value(&ledger.inner, NodeId::new(node_id)).map(|s| s.to_vec()))
    }

    pub fn solve(&mut self, config: Option<PySolverConfig>) -> PyResult<PyLedger> {
        let model_len = self.determine_model_len()?;
        self.ensure_compiled()?;
        let program = self.cached_program.as_ref().unwrap();

        // Convert PySolverConfig to Rust internal config
        let rust_config = match config {
            Some(c) => SolverConfig::from(c),
            None => SolverConfig::default()
        };

        let vars: Vec<NodeId> = self.registry.kinds.iter().enumerate()
            .filter(|(_, k)| matches!(k, NodeKind::SolverVariable))
            .map(|(i, _)| NodeId::new(i))
            .collect();
        let residuals: Vec<NodeId> = self.constraints.iter().map(|c| c.0).collect();
        
        let mut base_ledger = Ledger::new();
        base_ledger.resize(self.registry.count(), model_len);
        self.load_constants(&mut base_ledger, program, None)?;
        
        let result_ledger = optimizer::solve(
            &self.registry, 
            program, 
            vars, 
            residuals, 
            base_ledger,
            model_len,
            rust_config // Pass config
        ).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
             
        Ok(PyLedger { inner: result_ledger })
    }

    pub fn validate(&self) -> PyResult<()> {
        validation::validate(&self.registry)
            .map_err(|errs| {
                let msg = errs.iter().map(|e| format!("{}: {}", e.node_name, e.message)).collect::<Vec<_>>().join("\n");
                PyValueError::new_err(msg)
            })
    }
    
    pub fn trace_node(&mut self, node_id: usize, ledger: &PyLedger) -> PyResult<String> {
        self.check_bounds(node_id)?;
        self.ensure_compiled()?;
        let program = self.cached_program.as_ref().unwrap();
        
        Ok(trace::format_trace(
            &self.registry, 
            &ledger.inner, 
            NodeId::new(node_id), 
            &self.constraints,
            &program.layout
        ))
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        topology::sort(&self.registry)
            .map(|v| v.into_iter().map(|id| id.index()).collect())
            .map_err(|e| PyValueError::new_err(e))
    }
    
    pub fn node_count(&self) -> usize { self.registry.count() }
    
    pub fn is_scalar(&self, node_id: usize) -> bool {
        let mut cache = vec![None; self.registry.count()];
        self.check_is_scalar(NodeId::new(node_id), &mut cache)
    }

    /// Internal parallel executor for a batch of scenarios.
    // Note: compute_batch might need checking of bounds on overrides keys too.
    pub fn compute_batch(
        &mut self, 
        py: Python<'_>,
        scenarios: HashMap<String, HashMap<usize, Vec<f64>>>
    ) -> PyResult<HashMap<String, PyLedger>> {
        self.ensure_compiled()?;
        let program = self.cached_program.as_ref().unwrap();
        let model_len = self.determine_model_len()?;
        
        let mut base_ledger = Ledger::new();
        base_ledger.resize(self.registry.count(), model_len);
        self.load_constants(&mut base_ledger, program, None)?;

        let results: Result<HashMap<String, PyLedger>, String> = py.allow_threads(|| {
            scenarios.into_par_iter().map(|(name, overrides)| {
                let mut ledger = base_ledger.clone();
                for (idx, val) in overrides {
                    program.set_value(&mut ledger, NodeId::new(idx), &val).map_err(|e| e.to_string())?;
                }
                Engine::run(program, &mut ledger).map_err(|e| e.to_string())?;
                Ok((name, PyLedger { inner: ledger }))
            }).collect()
        });
        results.map_err(PyRuntimeError::new_err)
    }


}

#[pyfunction]
pub fn benchmark_pure_rust(num_nodes: usize, input_fraction: f64) -> PyResult<(f64, f64, f64, usize)> {
    let mut registry = Registry::new();
    let num_inputs = (num_nodes as f64 * input_fraction) as usize;
    struct Lcg { state: u64 }
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
    let mut rng = Lcg::new(42);
    let start_gen = Instant::now();

    for i in 0..num_inputs {
        let val = rng.next_f64() * 100.0;
        let meta = NodeMetadata { name: format!("Input_{}", i), ..Default::default() };
        registry.add_node(NodeKind::Scalar(val), &[], meta);
    }

    for i in num_inputs..num_nodes {
        let p1 = rng.next_u32_range(i as u32);
        let p2 = rng.next_u32_range(i as u32);
        let op = match rng.next_u32_range(3) {
            0 => Operation::Add, 1 => Operation::Subtract, _ => Operation::Multiply,
        };
        let parents = vec![NodeId::new(p1 as usize), NodeId::new(p2 as usize)];
        let meta = NodeMetadata { name: format!("Formula_{}", i), ..Default::default() };
        registry.add_node(NodeKind::Formula(op), &parents, meta);
    }
    let gen_duration = start_gen.elapsed().as_secs_f64();

    let order = topology::sort(&registry).map_err(|e| PyValueError::new_err(e))?;
    let program = Compiler::new(&registry).compile(order)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    
    let mut ledger = Ledger::new();
    ledger.resize(registry.count(), 1); 

    let start_compute = Instant::now();
    for i in 0..num_inputs {
        if let NodeKind::Scalar(v) = registry.kinds[i] {
            // Using logic interface even in benchmark
            program.set_value(&mut ledger, NodeId::new(i), &[v]).unwrap();
        }
    }
    Engine::run(&program, &mut ledger)
        .map_err(|e| PyRuntimeError::new_err(format!("Full compute failed: {:?}", e)))?;
    let compute_duration = start_compute.elapsed().as_secs_f64();

    Ok((gen_duration, compute_duration, 0.0, num_nodes))
}