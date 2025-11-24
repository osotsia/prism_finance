# The Architect (Analysis)

**Role**: Structural Inspection & Validation.

This module contains algorithms that inspect the `Registry` in store/ to derive structural properties or ensure correctness. It is read-only with respect to the graph structure.

## Capabilities
1.  **Topology**: Topological sorting (Kahn's algorithm), cycle detection, and upstream/downstream dependency traversal.
2.  **Validation**:
    *   **Unit Inference**: Dimensional analysis (e.g., ensuring you don't add "USD" to "MWh").
    *   **Temporal Inference**: Stock/Flow consistency checking (e.g., ensuring `Stock + Stock` is flagged as ambiguous).