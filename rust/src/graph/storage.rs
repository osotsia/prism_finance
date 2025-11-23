//! storage.rs
//! Dense Columnar Layout with Scalar Inlining.

use crate::graph::{NodeMetadata, Operation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn index(&self) -> usize { self.0 as usize }
    pub fn new(idx: usize) -> Self { Self(idx as u32) }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// Optimization: Single float stored inline (8 bytes). 
    /// No heap allocation, no pointer chasing.
    Scalar(f64),
    /// Index into `constants_data` for vectors length > 1.
    TimeSeries(u32),
    Formula(Operation),
    SolverVariable,
}

#[derive(Debug, Clone, Default)]
pub struct GraphRegistry {
    pub kinds: Vec<NodeKind>,

    // Dense Topology
    pub parents_flat: Vec<NodeId>,
    pub parents_ranges: Vec<(u32, u32)>,

    // Adjacency List (Children)
    pub first_child: Vec<u32>,
    pub child_targets: Vec<NodeId>,
    pub next_child: Vec<u32>,

    pub meta: Vec<NodeMetadata>,
    pub constants_data: Vec<Vec<f64>>,
}

impl GraphRegistry {
    pub fn new() -> Self { Self::default() }
    pub fn count(&self) -> usize { self.kinds.len() }

    fn push_node(&mut self, kind: NodeKind, parents: &[NodeId], meta: NodeMetadata) -> NodeId {
        let id = NodeId(self.kinds.len() as u32);

        // 1. Children (Adjacency List append)
        for &parent in parents {
            let p_idx = parent.index();
            let head = self.first_child[p_idx];
            let new_edge = self.child_targets.len() as u32;
            self.child_targets.push(id);
            self.next_child.push(head);
            self.first_child[p_idx] = new_edge;
        }

        // 2. Parents (CSR append)
        let start = self.parents_flat.len() as u32;
        let count = parents.len() as u32;
        self.parents_flat.extend_from_slice(parents);
        self.parents_ranges.push((start, count));

        // 3. Metadata
        self.kinds.push(kind);
        self.first_child.push(u32::MAX);
        self.meta.push(meta);

        id
    }

    pub fn add_constant(&mut self, value: Vec<f64>, meta: NodeMetadata) -> NodeId {
        // OPTIMIZATION: If len is 1, inline it.
        if value.len() == 1 {
            self.push_node(NodeKind::Scalar(value[0]), &[], meta)
        } else {
            let idx = self.constants_data.len() as u32;
            self.constants_data.push(value);
            self.push_node(NodeKind::TimeSeries(idx), &[], meta)
        }
    }

    pub fn add_formula(&mut self, op: Operation, parents: Vec<NodeId>, meta: NodeMetadata) -> NodeId {
        self.push_node(NodeKind::Formula(op), &parents, meta)
    }

    pub fn add_solver_var(&mut self, meta: NodeMetadata) -> NodeId {
        self.push_node(NodeKind::SolverVariable, &[], meta)
    }

    pub fn get_constant(&self, id: NodeId) -> Option<&Vec<f64>> {
        match self.kinds[id.index()] {
            NodeKind::TimeSeries(idx) => Some(&self.constants_data[idx as usize]),
            _ => None, // Scalars are retrieved via get_scalar_value logic usually
        }
    }

    pub fn update_constant(&mut self, id: NodeId, value: Vec<f64>) -> Result<(), String> {
        match &mut self.kinds[id.index()] {
            NodeKind::Scalar(old_val) => {
                if value.len() == 1 {
                    *old_val = value[0];
                    Ok(())
                } else {
                    // This is a rare edge case: Scalar became Vector.
                    // Ideally we prevent this, but for now we could promote it.
                    // For Simplicity in this minimalist implementation: Error.
                    Err("Cannot change a Scalar constant to a TimeSeries".to_string())
                }
            }
            NodeKind::TimeSeries(idx) => {
                self.constants_data[*idx as usize] = value;
                Ok(())
            }
            _ => Err(format!("Node {:?} is not a constant.", id)),
        }
    }

    #[inline(always)]
    pub fn get_parents(&self, id: NodeId) -> &[NodeId] {
        let (start, count) = self.parents_ranges[id.index()];
        &self.parents_flat[start as usize..(start + count) as usize]
    }
}