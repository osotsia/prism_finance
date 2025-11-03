# Prism: A Verifiable Calculation Engine for Financial Modeling

![CI Status](https://github.com/your-org/prism/actions/workflows/build_and_publish.yml/badge.svg)
[![PyPI version](https://badge.fury.io/py/prism-finance.svg)](https://badge.fury.io/py/prism-finance)

Prism is a code-first financial modeling framework for building auditable and high-performance models in Python. It is designed for complex financial analysis where transparency, testability, and version control are critical.

It combines an ergonomic Python API with a high-performance Rust core to provide a robust environment for developers and analysts who treat financial models as mission-critical software systems.

## Key Features

*   **Dependency Tracing:** Generate a step-by-step trace for any value, showing how it was derived from its ultimate inputs.
*   **Declarative Solver:** Model complex circular dependencies with a clean, declarative syntax (`.must_equal`). An integrated solver handles the underlying numerical methods.
*   **Financial Type System:** Perform static analysis on the model graph to detect common errors like temporal mismatches (stock vs. flow) or inconsistent units before running calculations.
*   **Integrated Testing Framework:** Write unit, invariant, and performance tests directly alongside your model logic using a declarative API.
*   **Hybrid Execution Model:** An eager, "PyTorch-style" execution model for interactive development, with an opt-in static graph context for whole-model optimization and solving.
*   **High-Performance Rust Core:** The calculation graph, solver, and validation engine are written in Rust for memory safety and performance on large-scale models.

## Quick Start

Prism's hybrid model lets you build interactively and bring in a powerful solver when needed.

### Part 1: Eager Calculation & Tracing

Every calculation happens immediately. This is ideal for building the linear parts of your model. The `.trace()` method can be called on any variable to generate a complete audit log.

```python
from prism_finance import Var

# Define inputs with metadata
revenue = Var(100.0, name="Revenue", meta={'source': 'Q1-2024 Report'})
cogs_margin = Var(0.4, name="COGS Margin", meta={'source': 'Analyst Estimate'})

# Formulas run instantly
cogs = revenue * cogs_margin
gross_profit = revenue - cogs

# Inspect the result
print(f"Calculated Gross Profit: {gross_profit.value}")

# Trace the result to its source
gross_profit.trace()
```

Calling `gross_profit.trace()` produces this output, showing a clear path from the result to the original inputs:
```text
AUDIT TRACE for node 'gross_profit':
--------------------------------------------------------------------
[L1] gross_profit[60.0] = revenue[100.0] - cogs[40.0]
  |
  +-- [L2] revenue[100.0] -> Var(100.0)
  |  |
  |  +-- meta: {'source': 'Q1-2024 Report'}
  |
  +-- [L2] cogs[40.0] = revenue[100.0] * cogs_margin[0.4]
     |
     +-- [L3] revenue[100.0] -> (Ref to L2)
     |
     +-- [L3] cogs_margin[0.4] -> Var(0.4)
        |
        +-- meta: {'source': 'Analyst Estimate'}
```

### Part 2: Solving Circularities & Tracing

For circular logic (e.g., a financing fee that's part of the total funds raised), use the `Canvas` to define constraints and solve. The trace is transparent even through the solver.

```python
from prism_finance import Var, Canvas

# Define eager inputs for the scenario
project_cost = Var(1000.0, name="Project Cost")
fee_rate = Var(0.02, name="Fee Rate") # 2%

# Use a Canvas for the static graph and solver
with Canvas(from_vars=locals()) as model:
    # Define the variables involved in the circularity
    model.total_funds = Var(name="Total Funds")
    model.financing_fee = Var(name="Financing Fee")
    
    # Declare the rules of the system
    model.total_funds.must_equal(model.project_cost + model.financing_fee)
    model.financing_fee.must_equal(model.total_funds * model.fee_rate)
    
    # Solve the system of equations
    model.solve()

# Extract and trace the solved value
print(f"Solved Financing Fee: {model.compute(model.financing_fee):.2f}")
model.trace(model.financing_fee)
```

Calling `model.trace(model.financing_fee)` on the solved model generates this log, showing the solver's iterative work:
```text
AUDIT TRACE for node 'financing_fee':
--------------------------------------------------------------------
[L1] financing_fee[20.41] [SOLVED VALUE]
  |
  +-- Determined by Internal Newton-Raphson Solver
  |  |
  |  +-- Initial Guess (k=0):
  |  |  | financing_fee = 0.0 -> Error = -20.0
  |  |
  |  +-- Iteration (k=1):
  |  |  | -> Updated guess: financing_fee = 20.0 -> Error = -0.4
  |  |
  |  +-- Iteration (k=2):
  |  |  | -> Updated guess: financing_fee = 20.408 -> Error = 0.000
  |  |
  |  +-- CONVERGED, satisfying constraints dependent on:
  |     |
  |     +-- [L2] project_cost[1000.0] -> Var(1000.0)
  |     |
  |     +-- [L2] fee_rate[0.02] -> Var(0.02)
```

## Frequently Asked Questions

**Q1: Why not just use Excel or Python with Pandas?**

**A:** Prism focuses on two areas where traditional tools can be challenging for large models: auditability and managing circular dependencies. While spreadsheets are excellent for rapid prototyping, tracing dependencies can be a manual process. Prism provides an automatic, declarative trace for any value. Similarly, its integrated solver is a first-class feature designed to handle complex circularities transparently.

**Q2: What do you mean by a "Financial Type System"?**

**A:** Just as a programming language's type system helps detect errors like adding a string to an integer, Prism's Financial Type System helps detect common logical errors in a model's structure. Before running calculations, it analyzes the graph for issues like adding a "stock" (a balance at a point in time, like Debt) to a "flow" (a value over a period, like Revenue), or mixing incompatible units. It's a layer of automated validation.

**Q3: Is the internal solver required? Can I use my own?**

**A:** The internal solver is the recommended default because it is deeply integrated and enables the step-by-step audit trail for solved values. However, the architecture is pluggable. You can configure Prism to use established external solvers (like IPOPT or commercial alternatives) for problems that may require their specific capabilities, though this may result in a less detailed audit trace.
