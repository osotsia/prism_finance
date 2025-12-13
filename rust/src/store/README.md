# The Store

**Role**: High-Performance Graph Storage.

The `Registry` implements a Data-Oriented Design (DOD) approach to graph storage. Instead of representing nodes as heap-allocated objects with pointers to other objects, the graph is represented as a set of aligned, columnar arrays indexed by a simple integer (`NodeId`).

## Memory Layout

### 1. Primary Columns
The system uses parallel `Vec<T>` structures where the index `i` corresponds to Node `i`.
*   `kinds: Vec<NodeKind>`: Discriminator for node behavior (Scalar, Formula, etc.).
*   `meta: Vec<NodeMetadata>`: Heavy metadata (Strings, Options). Kept separate to keep the `kinds` array dense and cache-friendly.
*   `parents_ranges: Vec<(u32, u32)>`: Defines the slice `[start, start+count)` into the `parents_flat` array for upstream lookup.

### 2. Flattened Topology
*   **Upstream**: `parents_flat: Vec<NodeId>`. All dependencies for all nodes are packed into one array.
*   **Downstream (Adjacency)**: Implements a "linked-list-in-vector" approach to avoid `Vec<Vec<NodeId>>` overhead.
    *   `first_child`: Index of the first child edge for a node.
    *   `child_targets`: The `NodeId` of the child.
    *   `next_child`: Index of the next edge in the list.
    *   *Benefit*: This mimics a Compressed Sparse Row (CSR) format, significantly reducing cache misses during topological sorting and traversal.

### 3. Data Separation
*   `constants_data: Vec<Vec<f64>>`: Large time-series input data is stored here, referenced by index in `NodeKind::TimeSeries`. This keeps the topology lightweight while allowing heavy data to live on the heap.
