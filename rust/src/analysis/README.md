# The Architect (Analysis)

**Role**: Static Graph Analysis.

This module provides algorithms to inspect the structure of the `Registry` without executing it. It is responsible for ordering operations and enforcing semantic correctness.

## Algorithms

### 1. Topology (`topology.rs`)
*   **Kahn’s Algorithm**: Implements a non-recursive topological sort using an in-degree vector and a queue. This converts the DAG (Directed Acyclic Graph) into a linear execution sequence.
*   **Cycle Detection**: If the sorted list length does not match the node count, a cycle is detected.
*   **Downstream Traversal**: A BFS implementation (`downstream_from`) used to identify the subgraph affected by a change in inputs. This is the core logic enabling incremental recomputation.

### 2. Validation Engine (`validation.rs`)
*   **Two-Pass Inference**:
    1.  **Inference**: Traverses the graph in topological order. For each node, it computes the expected `TemporalType` and `Unit` based on its parents and operation type (e.g., `Flow + Flow = Flow`, `m * s = m*s`).
    2.  **Verification**: Compares the inferred properties against user-declared metadata (`.declare_type()`). Mismatches are collected into a `Vec<ValidationError>`.
*   **Caching**: Inferred types are cached during traversal to ensure $O(N)$ complexity.

### 3. Unit Algebra (`units.rs`)
*   **Parsing**: Parses string representations (e.g., "USD/MWh") into a `HashMap<BaseUnit, Exponent>`.
*   **Arithmetic**:
    *   Multiplication adds exponents (e.g., `m` * `m` = `m^2`).
    *   Division subtracts exponents (e.g., `m` / `s` = `m*s^-1`).
    *   Addition/Subtraction requires exact signature matching.
*   **Canonicalization**: Re-serializes the internal map to a standard string format to allow equality checking.