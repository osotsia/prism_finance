use super::types::*;

#[derive(Debug, Clone, Default)]
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
}

impl Registry {
    pub fn new() -> Self { Self::default() }
    pub fn count(&self) -> usize { self.kinds.len() }

    pub fn add_node(&mut self, kind: NodeKind, parents: &[NodeId], meta: NodeMetadata) -> NodeId {
        let id = NodeId(self.kinds.len() as u32);

        // 1. Register Parents (for upstream lookups)
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