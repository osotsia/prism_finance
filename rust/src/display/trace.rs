//! Implements the recursive logic for generating a human-readable audit trace.
use crate::computation::ledger::{Value}; // Removed unused SolverIteration import
use crate::computation::Ledger;
use crate::graph::{ComputationGraph, NodeId, NodeKind, Operation};
use std::collections::HashMap;
use std::fmt::Write;

pub fn format_trace(
    graph: &ComputationGraph,
    ledger: &Ledger,
    target_id: NodeId,
    constraints: &[(NodeId, String)],
) -> String {
    let mut tracer = Tracer::new(graph, ledger, constraints);

    if target_id.index() < graph.node_count() {
        let name = &graph.get_node_meta(target_id).name;
        let _ = writeln!(tracer.output, "AUDIT TRACE for node '{}':", name);
        let _ = writeln!(tracer.output, "--------------------------------------------------");
        tracer.trace_node(target_id, 1, "", true);
    } else {
        let _ = writeln!(tracer.output, "Error: Invalid Node ID {:?}", target_id);
    }
    tracer.output
}

struct Tracer<'a> {
    graph: &'a ComputationGraph,
    ledger: &'a Ledger,
    constraints: &'a [(NodeId, String)],
    visited_at_level: HashMap<NodeId, usize>,
    output: String,
    solver_log_printed: bool,
}

impl<'a> Tracer<'a> {
    fn new(graph: &'a ComputationGraph, ledger: &'a Ledger, constraints: &'a [(NodeId, String)]) -> Self {
        Self {
            graph, ledger, constraints,
            visited_at_level: HashMap::new(),
            output: String::new(),
            solver_log_printed: false,
        }
    }

    fn trace_node(&mut self, node_id: NodeId, level: usize, prefix: &str, _is_last_child: bool) {
        if let Some(&first_seen) = self.visited_at_level.get(&node_id) {
            let _ = writeln!(self.output, "{}-> (Ref to L{})", prefix, first_seen);
            return;
        }
        self.visited_at_level.insert(node_id, level);

        let meta = self.graph.get_node_meta(node_id);
        let kind = self.graph.get_node_kind(node_id);
        let node_value_str = self.format_node_value(node_id);
        let line_header = format!("[L{}] {}", level, meta.name);

        match kind {
            // Updated to handle both constant types
            NodeKind::Scalar(_) | NodeKind::TimeSeries(_) => 
                self.handle_constant(prefix, &line_header, &node_value_str, node_id),
            
            NodeKind::Formula(op) => {
                let parents = self.graph.get_parents(node_id);
                self.handle_formula(prefix, &line_header, &node_value_str, op, parents, level)
            },
            NodeKind::SolverVariable => {
                self.handle_solver_variable(prefix, &line_header, &node_value_str, node_id)
            },
        }
    }

    fn handle_constant(&mut self, prefix: &str, line_header: &str, val_str: &str, node_id: NodeId) {
        // Fetch constant value logic
        let initial_val_str = if let Some(vec) = self.graph.get_constant_value(node_id) {
             self.format_vec_values(vec)
        } else if let NodeKind::Scalar(v) = self.graph.get_node_kind(node_id) {
             format!("[{:.3}]", v)
        } else {
             "[N/A]".to_string()
        };

        let _ = writeln!(self.output, "{}{}{} -> Var({})", prefix, line_header, val_str, initial_val_str);
    }

    fn handle_formula(&mut self, prefix: &str, line_header: &str, val_str: &str, op: &Operation, parents: &[NodeId], level: usize) {
        let formula_str = self.format_formula_string(op, parents);
        let _ = writeln!(self.output, "{}{}{} = {}", prefix, line_header, val_str, formula_str);
        self.trace_children(prefix, parents, level);
    }

    fn handle_solver_variable(&mut self, prefix: &str, line_header: &str, val_str: &str, node_id: NodeId) {
        let _ = writeln!(self.output, "{}{}{} [SOLVER VARIABLE]", prefix, line_header, val_str);
        let child_stem = self.build_child_prefix_stem(prefix);
        self.print_solver_constraints(&child_stem, node_id);
        self.print_solver_convergence_log(&child_stem);
    }

    fn trace_children(&mut self, current_prefix: &str, children: &[NodeId], level: usize) {
        let stem = self.build_child_prefix_stem(current_prefix);
        for (i, &child_id) in children.iter().enumerate() {
            let is_last = i == children.len() - 1;
            let connector = if is_last { "`--" } else { "|--" };
            let full_prefix = format!("{}{}", stem, connector);
            self.trace_node(child_id, level + 1, &full_prefix, is_last);
        }
    }

    fn print_solver_constraints(&mut self, stem: &str, node_id: NodeId) {
        let _ = writeln!(self.output, "{}|", stem);
        let _ = writeln!(self.output, "{}`-- Determined by solving constraints:", stem);
        let relevant: Vec<_> = self.constraints.iter().filter(|(res_id, _)| {
            self.graph.upstream_from(&[*res_id]).contains(&node_id)
        }).collect();

        for (idx, (_, name)) in relevant.iter().enumerate() {
            let connector = if idx == relevant.len() - 1 { "|  `--" } else { "|  |--" };
            let _ = writeln!(self.output, "{}   {} {}", stem, connector, name);
        }
    }

    fn print_solver_convergence_log(&mut self, stem: &str) {
        if self.solver_log_printed { return; }
        if let Some(trace) = &self.ledger.solver_trace {
            if !trace.is_empty() {
                let prefix = format!("{}   | ", stem);
                let _ = writeln!(self.output, "{}{}", prefix, "   | --- IPOPT Convergence ---");
                let _ = writeln!(self.output, "{}{: >9}{: >11} {: >11} {: >11}", prefix, "iter", "obj", "inf_pr", "inf_du");
                for iter in trace {
                    let _ = writeln!(self.output, "{}{: >9}{: >11.4e} {: >11.4e} {: >11.4e}", prefix, iter.iter_count, iter.obj_value, iter.inf_pr, iter.inf_du);
                }
            }
        }
        self.solver_log_printed = true;
    }

    fn format_formula_string(&self, op: &Operation, parents: &[NodeId]) -> String {
        let sym = match op {
            Operation::Add => "+", Operation::Subtract => "-", 
            Operation::Multiply => "*", Operation::Divide => "/",
            Operation::PreviousValue { .. } => ".prev",
        };
        if parents.len() == 2 && sym.len() == 1 {
            format!("{} {} {}", self.format_parent_summary(parents[0]), sym, self.format_parent_summary(parents[1]))
        } else {
            sym.to_string()
        }
    }

    fn format_parent_summary(&self, id: NodeId) -> String {
        let name = &self.graph.get_node_meta(id).name;
        let val = self.format_node_value(id);
        format!("{}{}", name, val)
    }

    fn format_node_value(&self, id: NodeId) -> String {
        match self.ledger.get(id) {
            Some(Ok(v)) => self.format_value_enum(v),
            _ => "[?]".to_string(),
        }
    }
    
    // New helper to handle Value enum
    fn format_value_enum(&self, v: &Value) -> String {
        match v {
            Value::Scalar(s) => format!("[{:.3}]", s),
            Value::Series(vec) => self.format_vec_values(vec),
        }
    }

    fn format_vec_values(&self, v: &[f64]) -> String {
        if v.is_empty() { "[N/A]".to_string() }
        else if v.len() == 1 { format!("[{:.3}]", v[0]) }
        else { format!("[{:.3}, ...]", v[0]) }
    }

    fn build_child_prefix_stem(&self, s: &str) -> String {
        s.replace("`--", "   ").replace("|--", "|  ")
    }
}