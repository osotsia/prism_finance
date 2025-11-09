"""
A large-scale Leveraged Buyout (LBO) financial model demonstrating Prism's capabilities.

This example builds a 5-year, three-statement (Income Statement, Balance Sheet,
Cash Flow Statement) model for a hypothetical company acquisition.

It showcases several key features of the Prism engine:
1.  **Time-Series Calculation:** All core financials are modeled as 5-element
    vectors, representing a 5-year forecast period.
2.  **Inter-statement Dependencies:** The model correctly links the three financial
    statements, a common source of complexity.
3.  **Temporal Logic (`.prev()`):** Balance sheet roll-forwards (e.g., PP&E, Debt)
    are handled declaratively using the `.prev()` operator.
4.  **Declarative Solver for Circularities:** It models the classic LBO circular
    reference:
    - Interest Expense depends on the average Debt balance.
    - The Debt balance is paid down by Free Cash Flow.
    - Free Cash Flow depends on Net Income.
    - Net Income is net of Interest Expense.
    This circularity is resolved by the internal solver without manual goal-seeking.
5.  **Auditability (`.trace()`):** Demonstrates tracing a key output (Free Cash Flow)
    back through the complex model to its original assumptions.
"""
import sys
import os
import time

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def build_and_run_lbo_model():
    """Constructs, solves, and analyzes the LBO model."""
    print("--- Building and Running Large-Scale LBO Model ---")

    # --- 1. Global Assumptions ---
    NUM_YEARS = 5
    TAX_RATE = 0.25

    # --- 2. Create Canvas and Define Eager Inputs ---
    # Inputs are defined eagerly and will be linked into the static graph
    # when the `model.solve()` context is entered.
    
    canvas = Canvas()
    with canvas:
        # --- Transaction Assumptions ---
        entry_ebitda = Var([100.0], name="LTM EBITDA")
        entry_multiple = Var([10.0], name="Entry Multiple")
        exit_multiple = Var([11.0], name="Exit Multiple")
        
        purchase_price = entry_ebitda * entry_multiple
        
        # --- Capital Structure Assumptions ---
        initial_term_loan = Var([400.0], name="Initial Term Loan")
        sponsor_equity = purchase_price - initial_term_loan
        
        # --- Operations Assumptions (as time-series) ---
        revenue_growth_rate = Var([0.10, 0.09, 0.08, 0.07, 0.06], name="Revenue Growth Rate")
        cogs_margin = Var([0.60] * NUM_YEARS, name="COGS Margin")
        sga_percent_revenue = Var([0.15] * NUM_YEARS, name="SG&A % Revenue")
        capex_percent_revenue = Var([0.03] * NUM_YEARS, name="CapEx % Revenue")
        da_percent_revenue = Var([0.02] * NUM_YEARS, name="D&A % Revenue")
        
        # --- Balance Sheet Assumptions ---
        nwc_percent_revenue = Var([0.10] * NUM_YEARS, name="NWC % Revenue")
        
        # --- Debt Assumptions ---
        term_loan_interest_rate = Var([0.05] * NUM_YEARS, name="Term Loan Interest Rate")
        mandatory_amortization = Var([20.0] * NUM_YEARS, name="Mandatory Amortization")
        
        # --- Initial Balance Sheet State (Year 0) ---
        y0_revenue = Var([500.0], name="Y0 Revenue")
        y0_cash = Var([50.0], name="Y0 Cash")
        y0_nwc = y0_revenue * nwc_percent_revenue
        y0_ppe = Var([250.0], name="Y0 PP&E")


    # --- 3. Build Static Graph with Solver ---
    print("\nDefining model logic and circular dependencies...")
    with canvas as model:
        # --- Income Statement ---
        revenue = model.solver_var(name="Revenue")
        cogs = model.solver_var(name="COGS")
        gross_profit = model.solver_var(name="Gross Profit")
        sga = model.solver_var(name="SG&A")
        ebitda = model.solver_var(name="EBITDA")
        depreciation_amortization = model.solver_var(name="D&A")
        ebit = model.solver_var(name="EBIT")
        interest_expense = model.solver_var(name="Interest Expense")
        ebt = model.solver_var(name="EBT")
        taxes = model.solver_var(name="Taxes")
        net_income = model.solver_var(name="Net Income")

        # --- Cash Flow Statement ---
        cfo = model.solver_var(name="Cash Flow from Ops")
        change_in_nwc = model.solver_var(name="Change in NWC")
        capex = model.solver_var(name="Capital Expenditures")
        free_cash_flow = model.solver_var(name="Free Cash Flow")
        cash_available_for_repayment = model.solver_var(name="Cash for Repayment")
        optional_prepayment = model.solver_var(name="Optional Prepayment")
        total_debt_repayment = model.solver_var(name="Total Debt Repayment")
        net_change_in_cash = model.solver_var(name="Net Change in Cash")
        
        # --- Balance Sheet & Debt Schedule ---
        cash = model.solver_var(name="Cash")
        nwc = model.solver_var(name="NWC")
        ppe = model.solver_var(name="PP&E")
        total_assets = model.solver_var(name="Total Assets")
        term_loan_balance = model.solver_var(name="Term Loan Balance")
        shareholders_equity = model.solver_var(name="Shareholders Equity")
        total_liabilities_and_equity = model.solver_var(name="Total Liab & Equity")

        # === Link the model with `.must_equal` constraints ===

        # --- Income Statement Logic ---
        revenue.must_equal(revenue.prev(default=y0_revenue) * (Var([1.0] * NUM_YEARS, name="one") + revenue_growth_rate))
        cogs.must_equal(revenue * cogs_margin)
        gross_profit.must_equal(revenue - cogs)
        sga.must_equal(revenue * sga_percent_revenue)
        ebitda.must_equal(gross_profit - sga)
        depreciation_amortization.must_equal(revenue * da_percent_revenue)
        ebit.must_equal(ebitda - depreciation_amortization)
        ebt.must_equal(ebit - interest_expense)
        taxes.must_equal(ebt * Var([TAX_RATE] * NUM_YEARS, name="Tax Rate"))
        net_income.must_equal(ebt - taxes)
        
        # --- Cash Flow Logic ---
        nwc.must_equal(revenue * nwc_percent_revenue)
        change_in_nwc.must_equal(nwc - nwc.prev(default=y0_nwc))
        cfo.must_equal(net_income + depreciation_amortization - change_in_nwc)
        capex.must_equal(revenue * capex_percent_revenue)
        free_cash_flow.must_equal(cfo - capex)
        
        # --- Debt Schedule & Circularity ---
        # Note: `.prev()` creates the year-over-year dependency
        term_loan_beginning_balance = term_loan_balance.prev(default=initial_term_loan)
        avg_term_loan_balance = (term_loan_beginning_balance + term_loan_balance) / Var([2.0] * NUM_YEARS, name="two")
        
        # THE CORE CIRCULARITY: Interest expense depends on the ending balance, which depends on cash flow,
        # which depends on net income, which depends on interest expense.
        interest_expense.must_equal(avg_term_loan_balance * term_loan_interest_rate)
        
        cash_available_for_repayment.must_equal(free_cash_flow)
        optional_prepayment.must_equal(cash_available_for_repayment - mandatory_amortization)
        total_debt_repayment.must_equal(mandatory_amortization + optional_prepayment)
        term_loan_balance.must_equal(term_loan_beginning_balance - total_debt_repayment)
        
        # --- Balance Sheet Logic ---
        net_change_in_cash.must_equal(free_cash_flow - total_debt_repayment)
        cash.must_equal(cash.prev(default=y0_cash) + net_change_in_cash)
        ppe.must_equal(ppe.prev(default=y0_ppe) + capex - depreciation_amortization)
        total_assets.must_equal(cash + nwc + ppe)
        
        # Equity is a "plug" that makes the balance sheet balance
        shareholders_equity.must_equal(total_assets - term_loan_balance)
        total_liabilities_and_equity.must_equal(term_loan_balance + shareholders_equity)

        # --- Solve the System ---
        print(f"\nModel constructed with {model.node_count} nodes.")
        print("Executing solver to resolve circular dependencies...")
        start_time = time.perf_counter()
        model.solve()
        end_time = time.perf_counter()
        print(f"Solver finished in {end_time - start_time:.4f} seconds.")


    # --- 4. Retrieve and Analyze Results ---
    print("\n--- Key Financial Outputs (Years 1-5) ---")
    
    # Helper to format time-series output
    def print_series(var_name, var_obj):
        values = model.get_value(var_obj)
        formatted_values = ", ".join([f"{v:8.2f}" for v in values])
        print(f"  - {var_name:<25}: [{formatted_values} ]")

    print_series("Revenue", revenue)
    print_series("EBITDA", ebitda)
    print_series("Net Income", net_income)
    print_series("Free Cash Flow", free_cash_flow)
    print_series("Term Loan Balance (EOP)", term_loan_balance)
    print_series("Shareholders Equity (EOP)", shareholders_equity)
    print_series("Cash (EOP)", cash)

    # --- 5. Verification and Returns Analysis ---
    print("\n--- Verification and Returns ---")
    
    # Verify the balance sheet balances in the final year
    final_assets = model.get_value(total_assets)[-1]
    final_liab_equity = model.get_value(total_liabilities_and_equity)[-1]
    
    print(f"  - Final Year Total Assets:      {final_assets:,.2f}")
    print(f"  - Final Year Liab. & Equity:  {final_liab_equity:,.2f}")
    
    balance_sheet_check = abs(final_assets - final_liab_equity)
    assert balance_sheet_check < 1e-6, "Balance sheet does not balance!"
    print("  - VERIFIED: Balance sheet balances correctly.")

    # Calculate returns
    exit_year_ebitda = model.get_value(ebitda)[-1]
    exit_enterprise_value = exit_year_ebitda * model.get_value(exit_multiple)[0]
    final_debt = model.get_value(term_loan_balance)[-1]
    final_cash = model.get_value(cash)[-1]
    
    exit_equity_value = exit_enterprise_value - final_debt + final_cash
    initial_equity = model.get_value(sponsor_equity)[0]
    
    moic = exit_equity_value / initial_equity
    
    print(f"\n  - Initial Equity Investment:  {initial_equity:,.2f}")
    print(f"  - Exit Equity Value:          {exit_equity_value:,.2f}")
    print(f"  - Multiple on Invested Capital (MoIC): {moic:.2f}x")

    # --- 6. Demonstrate Tracing on a Key Output ---
    print("\n--- Audit Trace for Final Year Free Cash Flow ---")
    # To trace a specific time-step, you would need an API to select an index.
    # For now, tracing the Var traces the entire series calculation.
    model.trace(free_cash_flow)


if __name__ == "__main__":
    build_and_run_lbo_model()