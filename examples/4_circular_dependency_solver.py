"""
Demonstrates the declarative solver handling a circular dependency.

This example models a classic circular reference in project finance:
A financing fee is calculated as a percentage of the total funds raised,
but the total funds raised must include the fee itself.

Let:
  F = Financing Fee
  R = Total Funds Raised
  C = Project Cost
  r = Fee Rate

The system of equations is:
  1) R = C + F
  2) F = R * r

This cannot be solved with a simple one-pass calculation. Prism's solver
is designed to handle exactly this kind of problem declaratively.
"""
import sys
import os

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def demonstrate_solver():
    """Builds and solves a model with a circular dependency."""
    print("--- Demonstrating Circular Dependency Solver ---")

    # --- 1. Define Known Inputs ---
    # These are the constants of our model. They are created outside the Canvas
    # for clarity, but they will be linked to the graph when used in formulas.
    # Note: For this to work, they must be created within an active Canvas context.
    model = Canvas()
    with model:
        project_cost = Var(1000.0, name="Project Cost")
        fee_rate = Var(0.02, name="Fee Rate")  # 2% fee

        # --- 2. Define Solver Variables ---
        # These are the unknown values we need the solver to find.
        # We declare them using `model.solver_var`.
        total_funds = model.solver_var(name="Total Funds")
        financing_fee = model.solver_var(name="Financing Fee")

        # --- 3. Declare the System of Equations ---
        # We build the right-hand side of each equation as a normal formula.
        # Then, we use `.must_equal()` to create a constraint.

        # Constraint 1: total_funds must equal (project_cost + financing_fee)
        rhs1 = project_cost + financing_fee
        total_funds.must_equal(rhs1)
        print(f"Declared constraint: {total_funds.name} == {rhs1.name}")

        # Constraint 2: financing_fee must equal (total_funds * fee_rate)
        rhs2 = total_funds * fee_rate
        financing_fee.must_equal(rhs2)
        print(f"Declared constraint: {financing_fee.name} == {rhs2.name}")

        # --- 4. Execute the Solver ---
        print("\nSolving the system...")
        model.solve()
        print("Solver finished.")

    # --- 5. Retrieve and Verify Results ---
    # We use `model.get_value(var)` to get the final values from the internal ledger.
    solved_funds = model.get_value(total_funds)
    solved_fee = model.get_value(financing_fee)

    print(f"\n--- Solved Values ---")
    print(f"  - Total Funds Raised: {solved_funds:,.2f}")
    print(f"  - Financing Fee:      {solved_fee:,.2f}")

    # --- Verification ---
    # We can manually check if the constraints are met.
    # Algebraically, the solution is F = C * r / (1 - r)
    expected_fee = 1000.0 * 0.02 / (1 - 0.02)
    expected_funds = 1000.0 + expected_fee

    print("\n--- Verification ---")
    print(f"  - Expected Fee:   {expected_fee:,.2f}")
    print(f"  - Expected Funds: {expected_funds:,.2f}")
    
    assert abs(solved_fee - expected_fee) < 1e-6, "Solved fee does not match expected value."
    assert abs(solved_funds - expected_funds) < 1e-6, "Solved funds do not match expected value."
    
    print("\nResults match the analytical solution. The model is correct.")

    # --- Demonstrate Tracing on a Key Output ---
    print("\n--- Audit Trace for Financing fee ---")
    model.trace(financing_fee)

if __name__ == "__main__":
    demonstrate_solver()