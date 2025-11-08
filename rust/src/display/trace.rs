//! Implements the recursive logic for generating a human-readable audit trace.

use crate::computation::Ledger;
use crate::graph::{ComputationGraph, Node, NodeId, Operation};
use std::collections::HashMap;
use std::fmt::Write;

struct Tracer<'a> {
    graph: &'a ComputationGraph,
    ledger: &'a Ledger,
    visited: HashMap<NodeId, usize>,
    output: String,
}

impl<'a> Tracer<'a> {
    fn new(graph: &'a ComputationGraph, ledger: &'a Ledger) -> Self {
        Self {
            graph,
            ledger,
            visited: HashMap::new(),
            output: String::new(),
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

    fn trace_recursive(&mut self, node_id: NodeId, level: usize, prefix: &str, is_last: bool) {
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
            Node::Constant { meta } => {
                let initial_val_str = Self::format_value(self.graph.get_constant_value(node_id).unwrap());
                let _ = writeln!(self.output, "{} -> Var({})", value_str, initial_val_str);
                // In the future, metadata could be printed here
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
            Node::SolverVariable { .. } => {
                let _ = writeln!(self.output, "{} [SOLVED VALUE]", value_str);
            }
            Node::Constraint { .. } => {
                 let _ = writeln!(self.output, " [CONSTRAINT]");
            }
        }
    }
}

pub fn format_trace(graph: &ComputationGraph, ledger: &Ledger, target_id: NodeId) -> String {
    let mut tracer = Tracer::new(graph, ledger);
    let node_name = graph.get_node(target_id).unwrap().meta().name.clone();
    
    let _ = writeln!(tracer.output, "AUDIT TRACE for node '{}':", node_name);
    let _ = writeln!(tracer.output, "--------------------------------------------------");
    
    tracer.trace_recursive(target_id, 1, "", true);
    
    tracer.output
}