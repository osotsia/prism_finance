"""
A basic example demonstrating the .trace() functionality for auditing calculations.
"""
import sys
import os

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def demonstrate_tracing():
    """Builds a simple graph and traces a final value back to its inputs."""
    print("--- Demonstrating Audit Trace Functionality ---")

    with Canvas() as model:
        # --- 1. Define Inputs ---
        revenue = Var(100.0, name="Revenue")
        cogs_margin = Var(0.4, name="COGS_Margin")
        opex = Var(25.0, name="Operating_Expenses")

        # --- 2. Define Formulas ---
        cogs = revenue * cogs_margin
        cogs._name = "COGS" # Manually name intermediate variables for clarity

        gross_profit = revenue - cogs
        gross_profit._name = "Gross_Profit"

        ebit = gross_profit - opex
        ebit._name = "EBIT"

        # --- 3. Compute the Graph ---
        model.compute_all()
        
        ebit_value = model.get_value(ebit)
        print(f"Model computed. Final EBIT: {ebit_value:.3f}\n")

        # --- 4. Trace the Final Result ---
        # The trace() method generates a step-by-step breakdown of the calculation.
        ebit.trace()
        
        print("\n--- Tracing an Input Variable ---")
        # Tracing an input variable shows its base case.
        revenue.trace()


if __name__ == "__main__":
    demonstrate_tracing()