# The Store

**Role**: Passive Memory Storage.

This module contains the `Registry`, a Data-Oriented Design (DOD) structure that holds the entire graph definition.

## Architecture
*   **Columnar Arrays**: Nodes are indices (`u32`). Data is stored in parallel vectors (`kinds`, `meta`, `parents`).
*   **Zero Logic**: The Store does not know how to calculate, sort, or validate. It only stores data.
*   **Jagged Arrays**: Parents and children are stored in flattened vectors (`parents_flat`, `first_child`) to minimize cache misses and heap allocations during traversal.

## Key Files
*   `types.rs`: The primitives (`NodeId`, `Operation`, `TemporalType`).
*   `registry.rs`: The struct holding the vectors.