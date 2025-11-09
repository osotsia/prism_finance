//! Implements the recursive logic for generating a human-readable audit trace.

use crate::computation::ledger::SolverIteration;
use crate::computation::Ledger;
use crate::graph::{ComputationGraph, Node, NodeId, Operation};
use std::collections::HashMap;
use std::fmt::Write;

struct Tracer<'a> {
    graph: &'a ComputationGraph,
    ledger: &'a Ledger,
    constraints: &'a [(NodeId, String)],
    visited: HashMap<NodeId, usize>,
    output: String,
    solver_log_printed: bool,
}

impl<'a> Tracer<'a> {
    fn new(
        graph: &'a ComputationGraph, 
        ledger: &'a Ledger, 
        constraints: &'a [(NodeId, String)]
    ) -> Self {
        Self {
            graph,
            ledger,
            constraints,
            visited: HashMap::new(),
            output: String::new(),
            solver_log_printed: false,
        }
    }

    fn format_value(values: &[f64]) -> String {
        if values.is_empty() {
            "[N/A]".to_string()
        } else if values.len() == 1 {
            format!("[{:.3}]", values[0])
        } else {
            // For vectors, just show the first value to keep the trace clean
            format!("[{:.3}, ...]", values[0])
        }
    }

    fn trace_recursive(&mut self, node_id: NodeId, level: usize, prefix: &str, _is_last: bool) {
        if let Some(&first_seen_level) = self.visited.get(&node_id) {
            let _ = writeln!(self.output, "{}-> (Ref to L{})", prefix, first_seen_level);
            return;
        }

        let node = self.graph.get_node(node_id).unwrap();
        let value_str = match self.ledger.get(node_id) {
            Some(Ok(val_arc)) => Self::format_value(val_arc),
            _ => "[Error/Not Computed]".to_string(),
        };

        let line_prefix = format!("[L{}] ", level);
        let _ = write!(self.output, "{}{}{}", prefix, line_prefix, node.meta().name);

        match node {
            Node::Constant { meta: _ } => {
                let initial_val_str = Self::format_value(self.graph.get_constant_value(node_id).unwrap());
                let _ = writeln!(self.output, "{} -> Var({})", value_str, initial_val_str);
            }
            Node::Formula { op, parents, .. } => {
                let op_symbol = match op {
                    Operation::Add => "+",
                    Operation::Subtract => "-",
                    Operation::Multiply => "*",
                    Operation::Divide => "/",
                    Operation::PreviousValue { .. } => ".prev(...)",
                };

                let parent_details: Vec<String> = parents
                    .iter()
                    .map(|p_id| {
                        let p_node = self.graph.get_node(*p_id).unwrap();
                        let p_val_str = match self.ledger.get(*p_id) {
                            Some(Ok(val)) => Self::format_value(val),
                            _ => "[?]".to_string(),
                        };
                        format!("{}{}", p_node.meta().name, p_val_str)
                    })
                    .collect();

                let formula_str = if parents.len() == 2 {
                    format!("{} {} {}", parent_details[0], op_symbol, parent_details[1])
                } else {
                    op_symbol.to_string() // Fallback for other ops
                };

                let _ = writeln!(self.output, "{} = {}", value_str, formula_str);
                
                self.visited.insert(node_id, level);

                let new_prefix_stem = format!("{}  ", prefix.replace("`--", "|--").replace(" ", " "));
                for (i, parent_id) in parents.iter().enumerate() {
                    let is_child_last = i == parents.len() - 1;
                    let child_prefix = format!("{}{}", new_prefix_stem, if is_child_last { "`--" } else { "|--" });
                    self.trace_recursive(*parent_id, level + 1, &child_prefix, is_child_last);
                }
            }
            Node::SolverVariable { is_temporal_dependency, .. } => {
                let label = if *is_temporal_dependency {
                    "[SOLVED VIA TEMPORAL RECURSION]"
                } else {
                    "[SOLVED VIA SIMULTANEOUS EQUATION]"
                };
                let _ = writeln!(self.output, "{} {}", value_str, label);
                
                let new_prefix_stem = format!("{}  ", prefix.replace("`--", "|--").replace(" ", " "));
                let _ = writeln!(self.output, "{}|", new_prefix_stem);
                let _ = writeln!(self.output, "{}`-- Determined by solving constraints:", new_prefix_stem);

                let relevant_constraints: Vec<_> = self.constraints.iter().filter(|(residual_id, _)| {
                    self.graph.upstream_from(&[*residual_id]).contains(&node_id)
                }).collect();

                for (idx, (_, constraint_name)) in relevant_constraints.iter().enumerate() {
                    let log_prefix = format!("{}   |", new_prefix_stem);
                    let constraint_prefix = if idx == relevant_constraints.len() - 1 { "|  `--" } else { "|  |--" };
                    let _ = writeln!(self.output, "{}{} {}", log_prefix, constraint_prefix, constraint_name);
                }

                if !self.solver_log_printed {
                    if let Some(trace) = &self.ledger.solver_trace {
                        let log_prefix = format!("{}   | ", new_prefix_stem);
                        if !trace.is_empty() {
                            let _ = writeln!(self.output, "{}{}", log_prefix, "   | --- IPOPT Convergence ---");
                            let _ = writeln!(self.output, "{}{: >9}{: >11} {: >11} {: >11}", log_prefix, "iter", "obj_val", "inf_pr", "inf_du");
                            
                            for iter in trace.iter() {
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
            }
        }
    }
}

pub fn format_trace(graph: &ComputationGraph, ledger: &Ledger, target_id: NodeId, constraints: &[(NodeId, String)]) -> String {
    let mut tracer = Tracer::new(graph, ledger, constraints);
    let node_name = graph.get_node(target_id).unwrap().meta().name.clone();
    
    let _ = writeln!(tracer.output, "AUDIT TRACE for node '{}':", node_name);
    let _ = writeln!(tracer.output, "--------------------------------------------------");
    
    tracer.trace_recursive(target_id, 1, "", true);
    
    tracer.output
}