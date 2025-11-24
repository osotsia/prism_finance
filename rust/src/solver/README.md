# The Solver

**Role**: Circular Dependency Resolution.

This module bridges the Prism engine with the IPOPT non-linear solver to resolve simultaneous equations.

## Workflow
1.  **Problem Definition**: Identifies "Solver Variables" (unknowns) and "Residual Nodes" (constraints, i.e., LHS - RHS).
2.  **Pre-computation**: Uses the `Engine` to calculate all values independent of the solver variables.
3.  **Iteration**:
    *   IPOPT suggests values for the Solver Variables.
    *   The `Engine` computes the graph for those values.
    *   The Residuals are fed back to IPOPT as the error term.
4.  **Result**: The final converged values are written back to the `Ledger`.