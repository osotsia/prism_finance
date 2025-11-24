use crate::store::{Registry, NodeId, NodeKind, Operation};
use crate::compute::ledger::{Ledger, Value, ComputationError};
use crate::analysis::topology;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub fn format_trace(
    registry: &Registry,
    ledger: &Ledger,
    target: NodeId,
    constraints: &[(NodeId, String)]
) -> String {
    let mut tracer = Tracer {
        registry,
        ledger,
        constraints,
        visited_at_level: HashMap::new(),
        printed_constraints: HashSet::new(),
        output: String::new(),
        solver_log_printed: false,
        in_solver_block: false,
        downstream_cache: HashMap::new(),
    };

    if target.index() < registry.count() {
        let name = &registry.meta[target.index()].name;
        let _ = writeln!(tracer.output, "AUDIT TRACE for node '{}':", name);
        let _ = writeln!(tracer.output, "--------------------------------------------------");
        tracer.trace_node(target, 1, "", true);
    } else {
        let _ = writeln!(tracer.output, "Error: Invalid Node ID {:?}", target);
    }
    tracer.output
}

struct Tracer<'a> {
    registry: &'a Registry,
    ledger: &'a Ledger,
    constraints: &'a [(NodeId, String)],
    visited_at_level: HashMap<NodeId, usize>,
    printed_constraints: HashSet<NodeId>, // New: Deduplication set
    output: String,
    solver_log_printed: bool,
    in_solver_block: bool,
    downstream_cache: HashMap<NodeId, HashSet<NodeId>>, 
}

impl<'a> Tracer<'a> {
    fn trace_node(&mut self, node_id: NodeId, level: usize, prefix: &str, _is_last: bool) {
        if let Some(&first_seen) = self.visited_at_level.get(&node_id) {
            let _ = writeln!(self.output, "{}-> (Ref to L{})", prefix, first_seen);
            return;
        }
        self.visited_at_level.insert(node_id, level);

        let idx = node_id.index();
        let meta = &self.registry.meta[idx];
        let kind = &self.registry.kinds[idx];
        
        let node_val_str = self.format_value(node_id);
        let line_header = format!("[L{}] {}{}", level, meta.name, node_val_str);

        match kind {
            NodeKind::Scalar(_) | NodeKind::TimeSeries(_) => {
                let raw_val = match kind {
                    NodeKind::Scalar(v) => format!("[{:.3}]", v),
                    NodeKind::TimeSeries(i) => {
                         let vec = &self.registry.constants_data[*i as usize];
                         if vec.len() == 1 { format!("[{:.3}]", vec[0]) } else { format!("[len={}]", vec.len()) }
                    },
                    _ => unreachable!(),
                };
                let _ = writeln!(self.output, "{}{} -> Var({})", prefix, line_header, raw_val);
            }

            NodeKind::Formula(op) => {
                let parents = self.registry.get_parents(node_id);
                let formula_str = self.format_formula(op, parents);
                let _ = writeln!(self.output, "{}{} = {}", prefix, line_header, formula_str);
                self.recurse_children(prefix, parents, level);
            }

            NodeKind::SolverVariable => {
                let _ = writeln!(self.output, "{}{} [SOLVED]", prefix, line_header);

                if self.in_solver_block {
                    return;
                }
                self.in_solver_block = true;

                // 1. Context
                let all_vars: Vec<String> = self.registry.kinds.iter().enumerate()
                    .filter(|(_, k)| matches!(k, NodeKind::SolverVariable))
                    .map(|(i, _)| self.registry.meta[i].name.clone())
                    .collect();
                
                let child_stem = self.build_child_stem(prefix);
                if all_vars.len() > 1 {
                    let _ = writeln!(self.output, "{}|  Co-dependents: {:?}", child_stem, all_vars);
                }

                // 2. Convergence Log (Once per trace)
                self.print_solver_convergence(&child_stem);

                // 3. Explode Constraints
                self.print_exploded_constraints(&child_stem, node_id, level);

                self.in_solver_block = false;
            }
        }
    }

    fn recurse_children(&mut self, prefix: &str, children: &[NodeId], level: usize) {
        let stem = self.build_child_stem(prefix);
        for (i, &child) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            let connector = if is_last_child { "`--" } else { "|--" };
            let full_prefix = format!("{}{}", stem, connector);
            self.trace_node(child, level + 1, &full_prefix, is_last_child);
        }
    }

    fn print_exploded_constraints(&mut self, stem: &str, var_id: NodeId, level: usize) {
        let _ = writeln!(self.output, "{}|", stem);
        let _ = writeln!(self.output, "{}`-- Defining Constraints:", stem);
        
        if !self.downstream_cache.contains_key(&var_id) {
            let ds = topology::downstream_from(self.registry, &[var_id]);
            self.downstream_cache.insert(var_id, ds);
        }
        let downstream_nodes = self.downstream_cache.get(&var_id).unwrap();

        let relevant: Vec<_> = self.constraints.iter()
            .filter(|(res_id, _)| downstream_nodes.contains(res_id))
            .collect();

        let constr_stem = format!("{}   ", stem);

        for (i, (res_id, name)) in relevant.iter().enumerate() {
            let is_last_constr = i == relevant.len() - 1;
            let connector = if is_last_constr { "`--" } else { "|--" };
            
            // Deduplication Check
            if self.printed_constraints.contains(res_id) {
                let _ = writeln!(self.output, "{}{} Constraint: {} (See above)", constr_stem, connector, name);
                continue;
            }
            self.printed_constraints.insert(*res_id);

            let parents = self.registry.get_parents(*res_id);
            if parents.len() != 2 {
                let _ = writeln!(self.output, "{}{} {}", constr_stem, connector, name);
                continue;
            }
            let lhs_id = parents[0];
            let rhs_id = parents[1];

            let lhs_val = self.get_scalar_or_first(lhs_id);
            let rhs_val = self.get_scalar_or_first(rhs_id);
            let diff = (lhs_val - rhs_val).abs();
            
            let _ = writeln!(self.output, "{}{} Constraint: {}", constr_stem, connector, name);
            
            let inner_stem = if is_last_constr { "    " } else { "|   " };
            let inner_prefix = format!("{}{}", constr_stem, inner_stem);

            // Print LHS Branch
            let lhs_line = format!("{}|-- LHS [{:.4}]", inner_prefix, lhs_val);
            let _ = writeln!(self.output, "{}", lhs_line);
            self.trace_node(lhs_id, level + 2, &format!("{}|  `-- ", inner_prefix), true);

            // Print RHS Branch
            let rhs_line = format!("{}|-- RHS [{:.4}]", inner_prefix, rhs_val);
            let _ = writeln!(self.output, "{}", rhs_line);
            self.trace_node(rhs_id, level + 2, &format!("{}|  `-- ", inner_prefix), true);

            let _ = writeln!(self.output, "{}`-- Diff: {:.6} (Converged)", inner_prefix, diff);
            
            if !is_last_constr {
                let _ = writeln!(self.output, "{}|", constr_stem);
            }
        }
    }

    fn print_solver_convergence(&mut self, stem: &str) {
        if self.solver_log_printed { return; }
        
        if let Some(trace) = &self.ledger.solver_trace {
            if !trace.is_empty() {
                let _ = writeln!(self.output, "{}|  --- IPOPT Convergence ---", stem);
                let _ = writeln!(self.output, "{}|   iter        obj      inf_pr      inf_du", stem);
                for iter in trace {
                    let _ = writeln!(self.output, "{}|  {: >5}{: >11.4e} {: >11.4e} {: >11.4e}", 
                        stem, iter.iter_count, iter.obj_value, iter.inf_pr, iter.inf_du);
                }
            }
        }
        self.solver_log_printed = true;
    }

    fn format_formula(&self, op: &Operation, parents: &[NodeId]) -> String {
        match op {
            Operation::PreviousValue { lag, .. } => {
                // Special formatting for .prev(): "MainVar.prev(lag=X)"
                if !parents.is_empty() {
                    let main_name = &self.registry.meta[parents[0].index()].name;
                    format!("{}.prev(lag={})", main_name, lag)
                } else {
                    ".prev(?)".into()
                }
            },
            _ => {
                let sym = match op {
                    Operation::Add => "+",
                    Operation::Subtract => "-",
                    Operation::Multiply => "*",
                    Operation::Divide => "/",
                    _ => "?",
                };

                if parents.len() == 2 {
                    let lhs = self.format_parent_ref(parents[0]);
                    let rhs = self.format_parent_ref(parents[1]);
                    format!("{} {} {}", lhs, sym, rhs)
                } else {
                    sym.to_string()
                }
            }
        }
    }

    fn format_parent_ref(&self, id: NodeId) -> String {
        let name = &self.registry.meta[id.index()].name;
        let val = self.format_value(id);
        format!("{}{}", name, val)
    }

    fn format_value(&self, id: NodeId) -> String {
        match self.ledger.get(id) {
            Some(Ok(v)) => match v {
                Value::Scalar(s) => format!("[{:.3}]", s),
                Value::Series(vec) => {
                    if vec.len() == 1 { format!("[{:.3}]", vec[0]) } 
                    else { format!("[{:.3}, ...]", vec[0]) }
                }
            },
            Some(Err(ComputationError::MathError(e))) => format!("[Err: {}]", e),
            _ => "[?]".to_string(),
        }
    }
    
    fn get_scalar_or_first(&self, id: NodeId) -> f64 {
        match self.ledger.get(id) {
            Some(Ok(v)) => v.get_at(0),
            _ => 0.0,
        }
    }

    fn build_child_stem(&self, current_prefix: &str) -> String {
        current_prefix.replace("`--", "   ").replace("|--", "|  ")
    }
}