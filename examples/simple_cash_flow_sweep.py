"""
A simplified model to verify the solver's handling of circular dependencies.

This model demonstrates a common circular reference in finance: a cash flow sweep
used to pay down debt. The circular logic is as follows:
1.  Interest Expense is calculated based on the average debt balance during a period.
2.  Net Income is calculated after subtracting Interest Expense.
3.  Cash Flow (simplified here as equal to Net Income) is used to repay debt.
4.  The ending Debt Balance is determined by this repayment.
5.  The average Debt Balance depends on the ending Debt Balance, closing the loop.

This script isolates this circularity to provide a clear test of the solver.
"""
import sys
import os
import time

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def run_simple_sweep_model():
    """Constructs, solves, and verifies the simplified sweep model."""
    print("--- Running Simplified Cash Flow Sweep Solver Verification ---")

    NUM_YEARS = 3
    canvas = Canvas()
    vars_to_report = {}

    # --- Build, Solve, and Analyze within a SINGLE Canvas Context ---
    print("\nDefining model logic and circular dependencies...")
    with canvas as model:
        # --- 1. Define All Eager Inputs First ---
        initial_ebitda = Var([100.0], name="Initial EBITDA")
        ebitda_growth = Var([0.05] * NUM_YEARS, name="EBITDA Growth Rate")
        tax_rate = Var([0.30] * NUM_YEARS, name="Tax Rate")
        y0_debt_balance = Var([500.0], name="Y0 Debt Balance")
        interest_rate = Var([0.06] * NUM_YEARS, name="Interest Rate")
        one = Var([1.0] * NUM_YEARS, name="one")
        two = Var([2.0] * NUM_YEARS, name="two")

        # --- 2. Declare Solver Variables ---
        # Any variable involved in a circular definition (temporal or simultaneous)
        # must be declared as a solver_var to get a handle for use in formulas.
        ebitda = model.solver_var(name="EBITDA")
        interest_expense = model.solver_var(name="Interest Expense")
        debt_balance = model.solver_var(name="Debt Balance")

        # --- 3. Define Dependent Variables as Formulas ---
        ebt = ebitda - interest_expense
        taxes = ebt * tax_rate
        net_income = ebt - taxes
        cash_flow_for_sweep = net_income
        
        # --- 4. Store Handles for Reporting ---
        net_income._name = "Net Income"
        vars_to_report['EBITDA'] = ebitda
        vars_to_report['Interest Expense'] = interest_expense
        vars_to_report['Net Income'] = net_income
        vars_to_report['Debt Balance (EOP)'] = debt_balance

        # --- 5. Define Constraints for Solver Variables ---
        # Constraint 1: Temporal roll-forward for EBITDA.
        ebitda.must_equal(ebitda.prev(default=initial_ebitda) * (one + ebitda_growth))
        
        # Debt Schedule & Core Circularity
        beginning_debt = debt_balance.prev(default=y0_debt_balance)
        avg_debt_balance = (beginning_debt + debt_balance) / two
        
        # Constraint 2: Defines interest_expense based on the debt balance.
        interest_expense.must_equal(avg_debt_balance * interest_rate)
        
        # Constraint 3: Defines debt_balance based on cash flow (which depends on interest).
        debt_balance.must_equal(beginning_debt - cash_flow_for_sweep)
        
        # --- 6. Solve the System ---
        print(f"Model constructed with {model.node_count} nodes.")
        print("Executing solver...")
        start_time = time.perf_counter()
        model.solve()
        end_time = time.perf_counter()
        print(f"Solver finished in {end_time - start_time:.4f} seconds.")

    # --- Post-Solver Analysis (occurs after the 'with' block) ---
    print("\n--- Key Financial Outputs (Years 1-3) ---")

    def print_series(var_name, var_obj):
        values = canvas.get_value(var_obj)
        formatted_values = ", ".join([f"{v:8.2f}" for v in values])
        print(f"  - {var_name:<25}: [{formatted_values} ]")

    for name, var_obj in vars_to_report.items():
        print_series(name, var_obj)

    print("\n--- Verification ---")
    
    # Analytical solution for the first year's Net Income (NI)
    ebitda1 = 100.0 * 1.05
    int_rate = 0.06
    beg_debt = 500.0
    tax = 0.30
    # NI = (EBITDA - (BegDebt - 0.5*NI)*IntRate) * (1-Tax)
    # NI = (EBITDA - BegDebt*IntRate + 0.5*NI*IntRate) * (1-Tax)
    # NI = (EBITDA - BegDebt*IntRate)*(1-Tax) + 0.5*NI*IntRate*(1-Tax)
    # NI * (1 - 0.5*IntRate*(1-Tax)) = (EBITDA - BegDebt*IntRate)*(1-Tax)
    # NI = (EBITDA - BegDebt*IntRate)*(1-Tax) / (1 - 0.5*IntRate*(1-Tax))
    expected_ni_y1 = (ebitda1 - beg_debt * int_rate) * (1-tax) / (1 - 0.5 * int_rate * (1-tax))
    solved_ni_y1 = canvas.get_value(vars_to_report['Net Income'])[0]
    
    print(f"  - Analytical Solution for Year 1 Net Income: {expected_ni_y1:,.4f}")
    print(f"  - Solver Result for Year 1 Net Income:   {solved_ni_y1:,.4f}")
    
    assert abs(solved_ni_y1 - expected_ni_y1) < 1e-6, "Solver result does not match analytical solution."
    print("  - VERIFIED: Solver result is correct.")


if __name__ == "__main__":
    run_simple_sweep_model()