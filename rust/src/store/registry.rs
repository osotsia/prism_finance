use super::types::*;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    // Columnar Arrays
    pub kinds: Vec<NodeKind>,
    pub meta: Vec<NodeMetadata>,
    
    // Topology (CSR-ish + Adjacency)
    pub parents_flat: Vec<NodeId>,
    pub parents_ranges: Vec<(u32, u32)>, // (start, count)
    
    // Downstream traversal helpers
    pub first_child: Vec<u32>,
    pub child_targets: Vec<NodeId>,
    pub next_child: Vec<u32>,

    // Data Blobs
    pub constants_data: Vec<Vec<f64>>,

    // Ephemeral state for uniqueness checks (Not serialized, rebuilt on load)
    #[serde(skip)]
    pub used_names: HashSet<String>,
}

impl Registry {
    pub fn new() -> Self { Self::default() }
    pub fn count(&self) -> usize { self.kinds.len() }

    /// Rebuilds the `used_names` set after deserialization.
    pub fn rebuild_name_cache(&mut self) {
        self.used_names = self.meta.iter().map(|m| m.name.clone()).collect();
    }

    pub fn add_node(&mut self, kind: NodeKind, parents: &[NodeId], mut meta: NodeMetadata) -> NodeId {
        let id = NodeId(self.kinds.len() as u32);

        // --- Unique Name Enforcement ---
        let original_name = meta.name.clone();
        let mut candidate_name = original_name.clone();
        let mut counter = 1;

        while self.used_names.contains(&candidate_name) {
            candidate_name = format!("{}_{}", original_name, counter);
            counter += 1;
        }
        self.used_names.insert(candidate_name.clone());
        meta.name = candidate_name;
        // -------------------------------

        // 1. Register Parents
        let start = self.parents_flat.len() as u32;
        let count = parents.len() as u32;
        self.parents_flat.extend_from_slice(parents);
        self.parents_ranges.push((start, count));

        // 2. Register Children (Adjacency list for downstream lookups)
        for &parent in parents {
            let p_idx = parent.index();
            let head = self.first_child[p_idx];
            let new_edge = self.child_targets.len() as u32;
            self.child_targets.push(id);
            self.next_child.push(head);
            self.first_child[p_idx] = new_edge;
        }

        // 3. Metadata
        self.kinds.push(kind);
        self.meta.push(meta);
        self.first_child.push(u32::MAX);

        id
    }

    #[inline(always)]
    pub fn get_parents(&self, id: NodeId) -> &[NodeId] {
        let (start, count) = self.parents_ranges[id.index()];
        &self.parents_flat[start as usize..(start + count) as usize]
    }
}