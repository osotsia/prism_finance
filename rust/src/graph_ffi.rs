//! FFI bindings for the `graph` module, exposing its functionality to Python.

use crate::computation::{ComputationEngine, ComputationError, Ledger};
use crate::graph::dag::ComputationGraph;
use crate::graph::{NodeId, NodeKind, NodeMetadata, Operation, TemporalType, Unit};
use crate::solver::{optimizer as solver_optimizer, problem::PrismProblem};
use crate::type_system::TypeChecker;
use crate::display::trace;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::collections::HashSet;
use std::sync::Mutex;

fn to_py_err(e: ComputationError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

#[pyclass(name = "_Ledger")]
#[derive(Debug, Clone, Default)]
pub struct PyLedger {
    pub(crate) ledger: Ledger,
}

#[pymethods]
impl PyLedger {
    #[new]
    pub fn new() -> Self { Self::default() }

    pub fn get_value(&self, node_id: usize) -> PyResult<Option<Vec<f64>>> {
        match self.ledger.get(NodeId::new(node_id)) {
            Some(Ok(val)) => Ok(Some(val.to_vec())), // Value::to_vec handles conversion
            Some(Err(e)) => Err(to_py_err(e.clone())),
            None => Ok(None),
        }
    }
}

#[pyclass(name = "_ComputationGraph")]
#[derive(Debug, Clone, Default)]
pub struct PyComputationGraph {
    graph: ComputationGraph,
    constraints: Vec<(NodeId, String)>, // (residual_id, constraint_name)
}

impl PyComputationGraph {
    fn prepare_solver_inputs(&self) -> (Vec<NodeId>, Vec<NodeId>, HashSet<NodeId>, usize) {
        // Filter via the Registry Kinds column
        let solver_vars: Vec<NodeId> = self.graph.store.kinds.iter().enumerate()
            .filter(|(_, k)| matches!(k, NodeKind::SolverVariable))
            .map(|(i, _)| NodeId::new(i))
            .collect();

        let residual_nodes: Vec<NodeId> = self.constraints.iter().map(|(id, _)| *id).collect();
        let solver_dependent_nodes = self.graph.downstream_from(&solver_vars);

        let residual_predecessors = self.graph.upstream_from(&residual_nodes);
        let mut model_len = 1;
        
        for node_id in residual_predecessors {
            if let Some(val) = self.graph.get_constant_value(node_id) {
                model_len = model_len.max(val.len());
            }
        }
        (solver_vars, residual_nodes, solver_dependent_nodes, model_len)
    }

    fn precompute_independent_nodes(
        &self,
        engine: &ComputationEngine,
        solver_dependent_nodes: &HashSet<NodeId>,
    ) -> Result<Ledger, PyErr> {
        let mut base_ledger = Ledger::new();
        // Iterate all node indices
        let precompute_targets: Vec<NodeId> = (0..self.graph.node_count())
            .map(NodeId::new)
            .filter(|id| !solver_dependent_nodes.contains(id))
            .collect();
            
        engine.compute(&precompute_targets, &mut base_ledger).map_err(to_py_err)?;
        Ok(base_ledger)
    }
    
    fn parse_temporal_type(temporal_type: Option<String>) -> PyResult<Option<TemporalType>> {
        match temporal_type.as_deref() {
            Some("Stock") => Ok(Some(TemporalType::Stock)),
            Some("Flow") => Ok(Some(TemporalType::Flow)),
            Some(other) => Err(PyValueError::new_err(format!("Invalid temporal_type: '{}'", other))),
            None => Ok(None),
        }
    }
}


#[pymethods]
impl PyComputationGraph {
    #[new]
    pub fn new() -> Self { Self::default() }
    
    pub fn add_constant_node(&mut self, value: Vec<f64>, name: String, unit: Option<String>, temporal_type: Option<String>) -> PyResult<usize> {
        let meta = NodeMetadata {
            name,
            unit: unit.map(Unit),
            temporal_type: Self::parse_temporal_type(temporal_type)?,
        };
        Ok(self.graph.add_constant(value, meta).index())
    }

    pub fn set_node_name(&mut self, node_id: usize, new_name: String) -> PyResult<()> {
        let node_idx = NodeId::new(node_id);
        if node_idx.index() < self.graph.node_count() {
             self.graph.store.meta[node_idx.index()].name = new_name;
             Ok(())
        } else {
             Err(PyValueError::new_err(format!("Node with id {} not found", node_id)))
        }
    }
    
    pub fn update_constant_node(&mut self, node_id: usize, new_value: Vec<f64>) -> PyResult<()> {
        self.graph.update_constant(NodeId::new(node_id), new_value).map_err(PyValueError::new_err)
    }

    pub fn set_node_metadata(&mut self, node_id: usize, unit: Option<String>, temporal_type: Option<String>) -> PyResult<(Option<String>, Option<String>)> {
        let node_idx = NodeId::new(node_id);
        if node_idx.index() < self.graph.node_count() {
            let meta = &mut self.graph.store.meta[node_idx.index()];
            let old_unit = meta.unit.as_ref().map(|u| u.0.clone());
            let old_temporal_type = meta.temporal_type.as_ref().map(|tt| format!("{:?}", tt));
            if unit.is_some() { meta.unit = unit.map(Unit); }
            if temporal_type.is_some() { meta.temporal_type = Self::parse_temporal_type(temporal_type)?; }
            Ok((old_unit, old_temporal_type))
        } else {
            Err(PyValueError::new_err(format!("Node with id {} not found", node_id)))
        }
    }

    #[pyo3(name = "add_binary_formula")]
    pub fn py_add_binary_formula(&mut self, op_name: &str, parents: Vec<usize>, name: String) -> PyResult<usize> {
        let op = match op_name {
            "add" => Operation::Add, "subtract" => Operation::Subtract,
            "multiply" => Operation::Multiply, "divide" => Operation::Divide,
            _ => return Err(PyValueError::new_err(format!("Unsupported operation: {}", op_name))),
        };
        let parent_ids: Vec<NodeId> = parents.into_iter().map(NodeId::new).collect();
        let meta = NodeMetadata { name, ..Default::default() };
        
        // Call the high-level helper, which handles dependency registration internally
        Ok(self.graph.add_formula(op, parent_ids, meta).index())
    }

    pub fn add_formula_previous_value(&mut self, main_parent_idx: usize, default_parent_idx: usize, lag: u32, name: String) -> usize {
        let parents = vec![NodeId::new(main_parent_idx), NodeId::new(default_parent_idx)];
        let op = Operation::PreviousValue { lag, default_node: NodeId::new(default_parent_idx) };
        let meta = NodeMetadata { name, ..Default::default() };
        self.graph.add_formula(op, parents, meta).index()
    }

    pub fn add_solver_variable(&mut self, name: String) -> usize {
        let meta = NodeMetadata { name, ..Default::default() };
        self.graph.add_solver_var(meta).index()
    }

    pub fn must_equal(&mut self, lhs_id: usize, rhs_id: usize, name: String) {
        let lhs = NodeId::new(lhs_id);
        let rhs = NodeId::new(rhs_id);

        // Create the residual node (lhs - rhs)
        let meta = NodeMetadata { name: format!("residual_for_{}", name), ..Default::default() };
        let residual_id = self.graph.add_formula(Operation::Subtract, vec![lhs, rhs], meta);
        
        self.constraints.push((residual_id, name));
    }

    #[pyo3(name = "compute")]
    pub fn py_compute(&self, targets: Vec<usize>, ledger: &mut PyLedger, changed_inputs: Option<Vec<usize>>) -> PyResult<()> {
        if let Some(changes) = changed_inputs {
            let change_ids = changes.into_iter().map(NodeId::new).collect::<Vec<_>>();
            let dirty_set = self.graph.downstream_from(&change_ids);
            ledger.ledger.invalidate(dirty_set);
        }
        let engine = ComputationEngine::new(&self.graph);
        let target_ids: Vec<NodeId> = targets.into_iter().map(NodeId::new).collect();
        engine.compute(&target_ids, &mut ledger.ledger).map_err(to_py_err)?;
        Ok(())
    }

    #[pyo3(name = "solve")]
    pub fn py_solve(&self) -> PyResult<PyLedger> {
        let engine = ComputationEngine::new(&self.graph);
        let (solver_vars, residual_nodes, solver_dependent_nodes, model_len) = self.prepare_solver_inputs();

        if solver_vars.is_empty() {
            let mut ledger = Ledger::new();
            let all_nodes: Vec<NodeId> = (0..self.graph.node_count()).map(NodeId::new).collect();
            engine.compute(&all_nodes, &mut ledger).map_err(to_py_err)?;
            return Ok(PyLedger { ledger });
        }

        if residual_nodes.is_empty() {
            return Err(PyRuntimeError::new_err("Solver variables exist but no constraints were defined."));
        }

        let base_ledger = self.precompute_independent_nodes(&engine, &solver_dependent_nodes)?;

        let problem = PrismProblem {
            graph: &self.graph,
            variables: solver_vars,
            residuals: residual_nodes,
            model_len,
            sync_engine: engine,
            base_ledger,
            iteration_history: Mutex::new(Vec::new()),
        };
        let mut solved_ledger = solver_optimizer::solve(problem).map_err(to_py_err)?;

        let post_engine = ComputationEngine::new(&self.graph);
        let all_nodes: Vec<NodeId> = (0..self.graph.node_count()).map(NodeId::new).collect();
        post_engine.compute(&all_nodes, &mut solved_ledger).map_err(to_py_err)?;

        Ok(PyLedger { ledger: solved_ledger })
    }

    pub fn trace_node(&self, node_id: usize, ledger: &PyLedger) -> PyResult<String> {
        let trace_str = trace::format_trace(&self.graph, &ledger.ledger, NodeId::new(node_id), &self.constraints);
        Ok(trace_str)
    }

    #[pyo3(name = "validate")]
    pub fn py_validate(&self) -> PyResult<()> {
        let mut checker = TypeChecker::new(&self.graph);
        match checker.check_and_infer() {
            Ok(()) => Ok(()),
            Err(errors) => {
                let msgs: Vec<String> = errors.iter().map(|e| format!("Node '{}': {}", e.node_name, e.message)).collect();
                Err(PyValueError::new_err(format!("Validation failed with {} error(s):\n- {}", errors.len(), msgs.join("\n- "))))
            }
        }
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        match self.graph.topological_order() {
            Ok(order) => Ok(order.into_iter().map(|id| id.index()).collect()),
            Err(msg) => Err(PyValueError::new_err(msg)),
        }
    }

    pub fn node_count(&self) -> usize { self.graph.node_count() }
}