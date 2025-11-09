
# Prism: A Verifiable Calculation Engine for Financial Modeling

![CI Status](https://github.com/osotsia/prism_finance/actions/workflows/build_and_publish.yml/badge.svg)
[![PyPI version](https://badge.fury.io/py/prism-finance.svg)](https://badge.fury.io/py/prism-finance)

Prism is a Python library with a high-performance Rust core for building auditable financial models. It is designed for complex analysis where transparency, testability, and the explicit management of circular dependencies are critical.

Models are constructed as calculation graphs, enabling features such as step-by-step dependency tracing, static validation of model logic, and efficient incremental recomputation.

## Quick Start
```bash
# Prism requires the IPOPT nonlinear optimization library.
brew install ipopt

# Install the `prism-finance` package from PyPI.
pip install prism-finance

# Clone the repository
git clone https://github.com/osotsia/prism_finance.git

# Navigate into the project directory
cd prism_finance

# Run the examples
python examples/4_circular_dependency_solver.py
```

## Key Features

*   **Dependency Tracing:** Automatically generate a step-by-step audit trace for any value, showing precisely how it was derived from its ultimate inputs.
*   **Declarative Solver:** Model complex circular dependencies with a clean, declarative syntax (`.must_equal`). The core engine integrates with the IPOPT nonlinear solver to find solutions for systems of simultaneous equations.
*   **Static Validation:** Perform pre-computation analysis on the model graph to detect logical errors like temporal mismatches (e.g., adding a "stock" to a "stock") or inconsistent units (e.g., adding "USD" to "MWh").
*   **Incremental Recomputation:** After an initial full computation, modify input values and re-evaluate only the affected downstream nodes, significantly accelerating scenario analysis in large models.
*   **High-Performance Rust Core:** The graph data structure, computation logic, solver integration, and validation engine are implemented in Rust for performance and memory safety.

## Core Concepts

All model logic is defined within a `Canvas` context, which contains the calculation graph.

### 1. Building a Calculation Graph & Tracing

Formulas are defined using standard Python operators on `Var` objects. The graph can be computed and any value can be traced to its source.

```python
from prism_finance import Canvas, Var

with Canvas() as model:
    # 1. Define Inputs
    revenue = Var(100.0, name="Revenue")
    cogs_margin = Var(0.4, name="COGS_Margin")
    operating_expenses = Var(25.0, name="Operating_Expenses")

    # 2. Define Formulas
    cogs = revenue * cogs_margin
    gross_profit = revenue - cogs
    ebit = gross_profit - operating_expenses

    # 3. Compute and Trace
    model.compute_all()
    print(f"Final EBIT: {model.get_value(ebit):.3f}\n")
    model.trace(ebit)
```

This script produces the following audit trace, detailing the calculation path for `ebit`:

```text
Final EBIT: 35.000

AUDIT TRACE for node '( (Revenue - (Revenue * COGS_Margin)) - Operating_Expenses )':
--------------------------------------------------
[L1] ( (Revenue - (Revenue * COGS_Margin)) - Operating_Expenses )[35.000] = (Revenue - (Revenue * COGS_Margin))[60.000] - Operating_Expenses[25.000]
  |--[L2] (Revenue - (Revenue * COGS_Margin))[60.000] = Revenue[100.000] - (Revenue * COGS_Margin)[40.000]
  |--  |--[L3] Revenue[100.000] -> Var([100.000])
  |--  `--[L3] (Revenue * COGS_Margin)[40.000] = Revenue[100.000] * COGS_Margin[0.400]
  |--  |--  |--[L4] Revenue[100.000] -> Var([100.000])
  |--  |--  `--[L4] COGS_Margin[0.400] -> Var([0.400])
  `--[L2] Operating_Expenses[25.000] -> Var([25.000])
```

### 2. Solving Circular Dependencies

For circular logic (e.g., a financing fee that is part of the total funds raised), define unknown variables with `model.solver_var` and declare constraints with `.must_equal`. The `.solve()` method orchestrates the numerical solver.

```python
from prism_finance import Canvas, Var

with Canvas() as model:
    # 1. Define Known Inputs
    project_cost = Var(1000.0, name="Project Cost")
    fee_rate = Var(0.02, name="Fee Rate")

    # 2. Declare Unknowns
    total_funds = model.solver_var(name="Total Funds")
    financing_fee = model.solver_var(name="Financing Fee")

    # 3. Declare Constraints
    total_funds.must_equal(project_cost + financing_fee)
    financing_fee.must_equal(total_funds * fee_rate)

    # 4. Solve the System
    model.solve()

    # 5. Retrieve and Trace Results
    print(f"Solved Financing Fee: {model.get_value(financing_fee):.2f}")
    model.trace(financing_fee)
```

The trace for a solved variable includes details from the solver, showing the constraints it satisfied and its convergence path:

```text
Solved Financing Fee: 20.41
AUDIT TRACE for node 'Financing Fee':
--------------------------------------------------
[L1] Financing Fee[20.408] [SOLVED VIA SIMULTANEOUS EQUATION]
  |
  `-- Determined by solving constraints:
     ||  |-- Constraint: Total Funds == (Project Cost + Financing Fee)
     ||  `-- Constraint: Financing Fee == (Total Funds * Fee Rate)
     |    | --- IPOPT Convergence ---
     |      iter    obj_val      inf_pr      inf_du
     |         0   0.0000e0    1.0000e3    0.0000e0
     |         1   0.0000e0   7.9328e-4    1.0204e3
     |         2   0.0000e0  6.2937e-10    0.0000e0
```

## Frequently Asked Questions

**Q1: Why not just use Excel or Python with Pandas?**

**A:** Prism focuses on two areas where traditional tools can be challenging for large models: auditability and managing circular dependencies. While spreadsheets are excellent for rapid prototyping, tracing dependencies can be a manual process. Prism provides an automatic, declarative trace for any value. Similarly, its integrated solver is a first-class feature designed to handle complex circularities transparently.

**Q2: What is the "Financial Type System"?**

**A:** Just as a programming language's type system helps detect errors like adding a string to an integer, Prism's validation engine helps detect common logical errors in a model's structure. Before running calculations, it analyzes the graph for issues like adding a "stock" (a balance at a point in time, like Debt) to another "stock", or mixing incompatible units. It is a layer of automated logical validation.

**Q3: Is the current solver required? Can I use my own?**

**A:** The current implementation requires the IPOPT solver via a Rust-to-C FFI bridge. This integration uses a mature solver, and enables the step-by-step audit trail for solved values. Future versions may include integration with other external solvers.