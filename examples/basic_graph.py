"""
A basic example demonstrating the construction and analysis of a computation graph.
"""
import sys
import os

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def demonstrate_graph_operations():
    """Builds and analyzes a simple graph using the public API."""
    print("--- Demonstrating Graph Operations ---")

    # The Canvas is the container for our model
    model = Canvas()

    # Create constant variables (nodes) using the Canvas factory method
    revenue = model.add_var(100.0, name="Revenue")
    costs = model.add_var(40.0, name="Costs")
    units_sold = model.add_var(10.0, name="Units Sold")
    initial_balance = model.add_var(1000.0, name="Initial Balance")

    # --- 1. Arithmetic Operations ---
    # The standard operators (+, -, *, /) are overloaded to build the graph.
    # Node names are automatically generated for clarity.
    gross_profit = revenue - costs
    price_per_unit = revenue / units_sold

    print(f"Graph constructed with {model.node_count} nodes.")

    # --- 2. Time-Series (.prev) Operation ---
    # The .prev() method creates a node that lags another, with a specified default.
    # This correctly builds a graph with semantic Temporal and DefaultValue edges.
    closing_balance = gross_profit.prev(default=initial_balance) + gross_profit

    print(f"\nExample Nodes:")
    print(f"  - Gross Profit: {gross_profit}")
    print(f"  - Price Per Unit: {price_per_unit}")
    print(f"  - Closing Balance: {closing_balance}")

    # --- 3. Graph Analysis ---
    # The core can compute a valid evaluation order (topological sort).
    # This detects structural errors like circular dependencies.
    try:
        order = model.get_evaluation_order()
        print(f"\nValid evaluation order (by node ID): {order}")
        print("This confirms the graph is a valid DAG and can be computed.")

        # Find the position of key nodes in the evaluation order
        pos_revenue = order.index(revenue._node_id)
        pos_profit = order.index(gross_profit._node_id)
        assert pos_revenue < pos_profit
        print(f"  - 'Revenue' (pos {pos_revenue}) is correctly calculated before 'Gross Profit' (pos {pos_profit}).")

    except ValueError as e:
        print(f"\nCaught an unexpected error: {e}")


if __name__ == "__main__":
    demonstrate_graph_operations()