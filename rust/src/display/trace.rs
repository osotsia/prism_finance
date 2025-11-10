//! Implements the recursive logic for generating a human-readable audit trace.

use crate::computation::ledger::SolverIteration;
use crate::computation::Ledger;
use crate::graph::{ComputationGraph, Node, NodeId, Operation};
use std::collections::HashMap;
use std::fmt::Write;

// --- Public Entry Point ---

/// Generates a human-readable, step-by-step audit trace for a target node.
pub fn format_trace(
    graph: &ComputationGraph,
    ledger: &Ledger,
    target_id: NodeId,
    constraints: &[(NodeId, String)],
) -> String {
    // A Tracer acts like a detective, walking backward from a result to uncover its origins.
    // It maintains the state of the investigation (e.g., which clues have been seen)
    // and assembles the final report.
    let mut tracer = Tracer::new(graph, ledger, constraints);

    if let Some(node) = graph.get_node(target_id) {
        let _ = writeln!(tracer.output, "AUDIT TRACE for node '{}':", node.meta().name);
        let _ = writeln!(tracer.output, "--------------------------------------------------");
        tracer.trace_node(target_id, 1, "", true);
    } else {
        // This case is unlikely if the graph is well-formed but is handled for robustness.
        let _ = writeln!(tracer.output, "Error: Could not find target node with ID {:?}.", target_id);
    }
    
    tracer.output
}

// --- Core Tracing Logic ---

/// Manages the state of a single trace operation.
struct Tracer<'a> {
    graph: &'a ComputationGraph,
    ledger: &'a Ledger,
    constraints: &'a [(NodeId, String)],
    /// Tracks nodes already traced to avoid infinite loops and redundant output.
    /// Maps a NodeId to the level at which it was first seen.
    visited_at_level: HashMap<NodeId, usize>,
    /// The string buffer for the final trace output.
    output: String,
    /// A flag to ensure the solver's convergence log is only printed once per trace.
    solver_log_printed: bool,
}

impl<'a> Tracer<'a> {
    fn new(
        graph: &'a ComputationGraph,
        ledger: &'a Ledger,
        constraints: &'a [(NodeId, String)],
    ) -> Self {
        Self {
            graph,
            ledger,
            constraints,
            visited_at_level: HashMap::new(),
            output: String::new(),
            solver_log_printed: false,
        }
    }

    /// The main recursive function that dispatches to the correct handler for each node type.
    fn trace_node(&mut self, node_id: NodeId, level: usize, prefix: &str, _is_last_child: bool) {
        // If we've seen this node before, just print a reference to avoid redundancy.
        if let Some(&first_seen_level) = self.visited_at_level.get(&node_id) {
            let _ = writeln!(self.output, "{}-> (Ref to L{})", prefix, first_seen_level);
            return;
        }
        self.visited_at_level.insert(node_id, level);

        // It is assumed that any node being traced exists in the graph.
        // Panicking here indicates a logic error elsewhere (e.g., a dangling NodeId).
        let node = self.graph.get_node(node_id).expect("Node must exist in graph to be traced");
        let node_value_str = self.format_node_value(node_id);

        let line_header = format!("[L{}] {}", level, node.meta().name);

        match node {
            Node::Constant { .. } => self.handle_constant(prefix, &line_header, &node_value_str, node_id),
            Node::Formula { op, parents, .. } => self.handle_formula(prefix, &line_header, &node_value_str, op, parents, level),
            Node::SolverVariable { is_temporal_dependency, .. } => self.handle_solver_variable(prefix, &line_header, &node_value_str, *is_temporal_dependency, node_id),
        }
    }

    // --- Node-Specific Handlers ---

    /// Handles tracing for a `Constant` node (a model input).
    fn handle_constant(&mut self, prefix: &str, line_header: &str, node_value_str: &str, node_id: NodeId) {
        let initial_val_str = self.graph.get_constant_value(node_id)
            .map(|v| self.format_values(v))
            .unwrap_or_else(|| "[N/A]".to_string());
        let _ = writeln!(
            self.output,
            "{}{}{} -> Var({})",
            prefix, line_header, node_value_str, initial_val_str
        );
    }

    /// Handles tracing for a `Formula` node (a calculation).
    fn handle_formula(&mut self, prefix: &str, line_header: &str, node_value_str: &str, op: &Operation, parents: &[NodeId], level: usize) {
        let formula_str = self.format_formula_string(op, parents);
        let _ = writeln!(
            self.output,
            "{}{}{} = {}",
            prefix, line_header, node_value_str, formula_str
        );
        self.trace_children(prefix, parents, level);
    }

    /// Handles tracing for a `SolverVariable` node.
    fn handle_solver_variable(&mut self, prefix: &str, line_header: &str, node_value_str: &str, is_temporal: bool, node_id: NodeId) {
        let label = if is_temporal {
            "[SOLVED VIA TEMPORAL RECURSION]"
        } else {
            "[SOLVED VIA SIMULTANEOUS EQUATION]"
        };
        let _ = writeln!(self.output, "{}{}{} {}", prefix, line_header, node_value_str, label);
        
        let child_prefix_stem = self.build_child_prefix_stem(prefix);
        
        self.print_solver_constraints(&child_prefix_stem, node_id);
        self.print_solver_convergence_log(&child_prefix_stem);
    }

    // --- Formatting and Printing Helpers ---

    /// Recursively traces the children (dependencies) of the current node.
    fn trace_children(&mut self, current_prefix: &str, children: &[NodeId], level: usize) {
        let child_prefix_stem = self.build_child_prefix_stem(current_prefix);
        let num_children = children.len();

        for (i, &child_id) in children.iter().enumerate() {
            let is_last = i == num_children - 1;
            let connector = if is_last { "`--" } else { "|--" };
            let full_prefix_for_child = format!("{}{}", child_prefix_stem, connector);
            self.trace_node(child_id, level + 1, &full_prefix_for_child, is_last);
        }
    }

    /// Prints the list of constraints that determined a solver variable's value.
    fn print_solver_constraints(&mut self, prefix_stem: &str, node_id: NodeId) {
        let _ = writeln!(self.output, "{}|", prefix_stem);
        let _ = writeln!(self.output, "{}`-- Determined by solving constraints:", prefix_stem);

        let relevant_constraints: Vec<_> = self.constraints.iter().filter(|(residual_id, _)| {
            // A constraint is relevant if the solver variable is an upstream dependency of its residual.
            self.graph.upstream_from(&[*residual_id]).contains(&node_id)
        }).collect();

        for (idx, (_, constraint_name)) in relevant_constraints.iter().enumerate() {
            let log_prefix = format!("{}   |", prefix_stem);
            let connector = if idx == relevant_constraints.len() - 1 { "|  `--" } else { "|  |--" };
            let _ = writeln!(self.output, "{}{} {}", log_prefix, connector, constraint_name);
        }
    }

    /// Prints the IPOPT convergence log, if available and not already printed.
    fn print_solver_convergence_log(&mut self, prefix_stem: &str) {
        if self.solver_log_printed { return; }
        
        if let Some(trace) = &self.ledger.solver_trace {
            if !trace.is_empty() {
                let log_prefix = format!("{}   | ", prefix_stem);
                let _ = writeln!(self.output, "{}{}", log_prefix, "   | --- IPOPT Convergence ---");
                let _ = writeln!(self.output, "{}{: >9}{: >11} {: >11} {: >11}", log_prefix, "iter", "obj_val", "inf_pr", "inf_du");
                
                for iter in trace {
                    let _ = writeln!(
                        self.output,
                        "{}{: >9}{: >11.4e} {: >11.4e} {: >11.4e}",
                        log_prefix, iter.iter_count, iter.obj_value, iter.inf_pr, iter.inf_du
                    );
                }
            }
        }
        self.solver_log_printed = true;
    }

    // --- Low-Level Formatting Helpers ---
    
    /// Formats the parents of a formula into a readable string like "Revenue[100] * COGS_Margin[0.4]".
    fn format_formula_string(&self, op: &Operation, parents: &[NodeId]) -> String {
        let op_symbol = match op {
            Operation::Add => "+",
            Operation::Subtract => "-",
            Operation::Multiply => "*",
            Operation::Divide => "/",
            Operation::PreviousValue { .. } => ".prev(...)",
        };
        
        if parents.len() == 2 && op_symbol.len() == 1 {
            let lhs = self.format_parent_summary(parents[0]);
            let rhs = self.format_parent_summary(parents[1]);
            format!("{} {} {}", lhs, op_symbol, rhs)
        } else {
            // Fallback for non-binary or more complex operations.
            op_symbol.to_string()
        }
    }
    
    /// Creates a compact summary of a parent node, e.g., "Revenue[100.000]".
    fn format_parent_summary(&self, node_id: NodeId) -> String {
        // It is assumed that parent nodes exist in the graph.
        let p_node = self.graph.get_node(node_id).unwrap();
        let p_val_str = self.format_node_value(node_id);
        format!("{}{}", p_node.meta().name, p_val_str)
    }

    /// Gets and formats the computed value of a node from the ledger.
    fn format_node_value(&self, node_id: NodeId) -> String {
        match self.ledger.get(node_id) {
            Some(Ok(val_arc)) => self.format_values(val_arc),
            _ => "[Error/Not Computed]".to_string(),
        }
    }

    /// Formats a slice of f64 values into a string for display in the trace.
    fn format_values(&self, values: &[f64]) -> String {
        if values.is_empty() {
            "[N/A]".to_string()
        } else if values.len() == 1 {
            format!("[{:.3}]", values[0])
        } else {
            // For time-series vectors, show only the first value to keep the trace concise.
            format!("[{:.3}, ...]", values[0])
        }
    }

    /// Builds the prefix for child nodes, correctly handling the tree structure.
    /// Example: A prefix of `|--` becomes a stem of `|  ` for its children.
    /// Example: A prefix of ``--` becomes a stem of `   ` for its children.
    fn build_child_prefix_stem(&self, current_prefix: &str) -> String {
        current_prefix
            .replace("`--", "   ")
            .replace("|--", "|  ")
    }
}