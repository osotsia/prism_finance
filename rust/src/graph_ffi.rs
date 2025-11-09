//! FFI bindings for the `graph` module, exposing its functionality to Python.

use crate::computation::{ComputationEngine, ComputationError, Ledger};
use crate::graph::dag::ComputationGraph;
use crate::graph::edge::Edge;
use crate::graph::node::{Node, NodeId, NodeMetadata, Operation, TemporalType, Unit};
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_value(&self, node_id: usize) -> PyResult<Option<Vec<f64>>> {
        match self.ledger.get(NodeId::new(node_id)) {
            Some(Ok(arc_vec)) => Ok(Some(arc_vec.to_vec())),
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

// Private helper functions, moved outside of the `#[pymethods]` block.
impl PyComputationGraph {
    /// The "Surveyor": Analyzes the graph to find all inputs required for the solver.
    fn prepare_solver_inputs(&self) -> (Vec<NodeId>, Vec<NodeId>, HashSet<NodeId>, usize) {
        let solver_vars: Vec<NodeId> = self.graph.graph.node_indices()
            .filter(|&id| matches!(self.graph.get_node(id), Some(Node::SolverVariable { .. })))
            .collect();

        let residual_nodes: Vec<NodeId> = self.constraints.iter().map(|(id, _)| *id).collect();
        let solver_dependent_nodes = self.graph.downstream_from(&solver_vars);

        // The "Timekeeper": Determines the model's time-series length by inspecting the
        // max length of any constant that is an input to the solver system.
        let residual_predecessors = self.graph.upstream_from(&residual_nodes);
        let mut model_len = 1;
        for node_id in residual_predecessors {
            if let Some(Node::Constant { .. }) = self.graph.get_node(node_id) {
                if let Some(val) = self.graph.get_constant_value(node_id) {
                    model_len = model_len.max(val.len());
                }
            }
        }
        (solver_vars, residual_nodes, solver_dependent_nodes, model_len)
    }

    /// The "Accountant": Pre-calculates the opening balances for the solver by computing
    /// all nodes that are independent of the circular system.
    fn precompute_independent_nodes(
        &self,
        engine: &ComputationEngine,
        solver_dependent_nodes: &HashSet<NodeId>,
    ) -> Result<Ledger, PyErr> {
        let mut base_ledger = Ledger::new();
        let precompute_targets: Vec<NodeId> = self.graph.graph.node_indices()
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
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_constant_node(&mut self, value: Vec<f64>, name: String, unit: Option<String>, temporal_type: Option<String>) -> PyResult<usize> {
        let meta = NodeMetadata {
            name,
            unit: unit.map(Unit),
            temporal_type: Self::parse_temporal_type(temporal_type)?,
        };
        let node_id = self.graph.add_constant(value, meta);
        Ok(node_id.index())
    }

    pub fn set_node_name(&mut self, node_id: usize, new_name: String) -> PyResult<()> {
        let node_idx = NodeId::new(node_id);
        if let Some(node) = self.graph.graph.node_weight_mut(node_idx) {
            node.meta_mut().name = new_name;
            Ok(())
        } else {
            Err(PyValueError::new_err(format!("Node with id {} not found", node_id)))
        }
    }
    
    pub fn update_constant_node(&mut self, node_id: usize, new_value: Vec<f64>) -> PyResult<()> {
        self.graph
            .update_constant(NodeId::new(node_id), new_value)
            .map_err(PyValueError::new_err)
    }

    pub fn set_node_metadata(&mut self, node_id: usize, unit: Option<String>, temporal_type: Option<String>) -> PyResult<(Option<String>, Option<String>)> {
        let node_idx = NodeId::new(node_id);
        if let Some(node) = self.graph.graph.node_weight_mut(node_idx) {
            let meta = node.meta_mut();
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
        let node = Node::Formula { op, parents: parent_ids.clone(), meta: NodeMetadata { name, ..Default::default() }};
        let child_id = self.graph.graph.add_node(node);
        for parent_id in parent_ids { self.graph.add_dependency(parent_id, child_id, Edge::Arithmetic); }
        Ok(child_id.index())
    }

    pub fn add_formula_previous_value(&mut self, main_parent_idx: usize, default_parent_idx: usize, lag: u32, name: String) -> usize {
        let main_parent_id = NodeId::new(main_parent_idx);
        let default_parent_id = NodeId::new(default_parent_idx);
        let node = Node::Formula {
            op: Operation::PreviousValue { lag, default_node: default_parent_id },
            parents: vec![main_parent_id, default_parent_id],
            meta: NodeMetadata { name, ..Default::default() },
        };
        let child_id = self.graph.graph.add_node(node);
        self.graph.add_dependency(main_parent_id, child_id, Edge::Temporal);
        self.graph.add_dependency(default_parent_id, child_id, Edge::DefaultValue);
        child_id.index()
    }

    pub fn add_solver_variable(&mut self, name: String) -> usize {
        let meta = NodeMetadata { name, ..Default::default() };
        let node = Node::SolverVariable { meta, is_temporal_dependency: false };
        self.graph.graph.add_node(node).index()
    }

    pub fn must_equal(&mut self, lhs_id: usize, rhs_id: usize, name: String) {
        let lhs_node_id = NodeId::new(lhs_id);
        let rhs_node_id = NodeId::new(rhs_id);
        
        // --- Infer if this is a temporal dependency ---
        let mut is_temporal = false;
        if let Some(Node::Formula { op: Operation::PreviousValue {..}, parents, .. }) = self.graph.get_node(rhs_node_id) {
            // A constraint like `X.must_equal(X.prev(...))` defines a temporal dependency.
            // The first parent of a .prev() node is the main series.
            if let Some(main_parent_id) = parents.get(0) {
                if *main_parent_id == lhs_node_id {
                    is_temporal = true;
                }
            }
        }
        
        // If it is, update the solver variable node.
        if is_temporal {
            if let Some(Node::SolverVariable { is_temporal_dependency, ..}) = self.graph.graph.node_weight_mut(lhs_node_id) {
                *is_temporal_dependency = true;
            }
        }

        // --- Create the residual node for the solver ---
        let residual_node = Node::Formula {
            op: Operation::Subtract,
            parents: vec![lhs_node_id, rhs_node_id],
            meta: NodeMetadata { name: format!("residual_for_{}", name), ..Default::default() }
        };
        let residual_id = self.graph.graph.add_node(residual_node);
        self.graph.add_dependency(lhs_node_id, residual_id, Edge::Arithmetic);
        self.graph.add_dependency(rhs_node_id, residual_id, Edge::Arithmetic);
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

    /// The "Orchestrator": Manages the sequence of operations for solving the graph.
    #[pyo3(name = "solve")]
    pub fn py_solve(&self) -> PyResult<PyLedger> {
        let engine = ComputationEngine::new(&self.graph);
        let (solver_vars, residual_nodes, solver_dependent_nodes, model_len) = self.prepare_solver_inputs();

        // If no solver variables, the model is a simple DAG. Just do a full computation.
        if solver_vars.is_empty() {
            let mut ledger = Ledger::new();
            let all_nodes: Vec<NodeId> = self.graph.graph.node_indices().collect();
            engine.compute(&all_nodes, &mut ledger).map_err(to_py_err)?;
            return Ok(PyLedger { ledger });
        }

        if residual_nodes.is_empty() {
            return Err(PyRuntimeError::new_err("Solver variables exist but no constraints were defined."));
        }

        // --- Stage 1: Pre-computation ---
        let base_ledger = self.precompute_independent_nodes(&engine, &solver_dependent_nodes)?;

        // --- Stage 2: Solving ---
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

        // --- Stage 3: Post-computation ---
        // The "Finisher": Calculates any remaining nodes that depend on the solved values.
        let post_engine = ComputationEngine::new(&self.graph);
        let all_nodes: Vec<NodeId> = self.graph.graph.node_indices().collect();
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
                let error_messages: Vec<String> = errors.iter().map(|e| format!("Node '{}': {}", e.node_name, e.message)).collect();
                Err(PyValueError::new_err(format!("Validation failed with {} error(s):\n- {}", errors.len(), error_messages.join("\n- "))))
            }
        }
    }

    pub fn topological_order(&self) -> PyResult<Vec<usize>> {
        match self.graph.topological_order() {
            Ok(order) => Ok(order.into_iter().map(|id| id.index()).collect()),
            Err(cycle) => Err(PyValueError::new_err(format!("Graph contains a cycle: a node (id: {}) depends on itself.", cycle.node_id().index()))),
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}