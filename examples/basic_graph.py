"""
A basic example demonstrating the construction of a computation graph.
"""
import sys
import os

# Add the project root to the Python path to allow importing `prism_finance`
# This is for demonstration purposes. In a real scenario, the package would be installed.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var


def demonstrate_valid_graph():
    """Builds and analyzes a simple, valid Directed Acyclic Graph (DAG)."""
    print("--- Demonstrating Valid Graph ---")

    # The Canvas is the container for our model
    model = Canvas()

    # Use the canvas to create constant variables (nodes)
    revenue = model.add_var(100.0, name="Revenue")
    costs = model.add_var(40.0, name="Costs")

    # The '+' operator is overloaded to create a new formula node
    # and automatically add the dependencies in the graph.
    profit = revenue + costs
    profit._name = "Profit"  # Manually assign a cleaner name for demonstration

    print(f"Graph constructed with {model.node_count} nodes.")
    print(f"Nodes are: {revenue}, {costs}, {profit}")

    try:
        # Get the evaluation order from the Rust core
        order = model.get_evaluation_order()
        print(f"\nValid evaluation order (by node ID): {order}")
        print("This means 'Profit' (id=2) must be calculated after 'Revenue' (id=0) and 'Costs' (id=1).")
    except ValueError as e:
        print(f"\nError: {e}")


def demonstrate_cyclic_graph():
    """Builds an invalid graph with a cycle to test error handling."""
    print("\n--- Demonstrating Cyclic Graph ---")

    # NOTE: The current high-level Python API makes it difficult to
    # create a cycle intentionally, which is a design feature.
    # To demonstrate the Rust core's cycle detection, we would need to
    # use the low-level `_core` API directly. This example shows
    # that the user-facing API provides a degree of safety.

    model = Canvas()
    a = model.add_var(1, name="A")

    # We can simulate creating a cycle for testing purposes by using internal APIs
    # This is not something a normal user would do.
    # Let's create a formula B = A + A (no cycle)
    b = a + a

    # Now, let's manually create a forbidden dependency from B back to A
    print("Intentionally creating a cycle by adding a dependency from B -> A...")
    try:
        model._graph.add_dependency(parent_idx=b._node_id, child_idx=a._node_id)

        # This line should now raise a ValueError from the Rust core
        model.get_evaluation_order()
    except ValueError as e:
        print(f"Successfully caught expected error: {e}")
    except Exception as e:
        print(f"Caught an unexpected error type: {type(e).__name__} - {e}")


if __name__ == "__main__":
    demonstrate_valid_graph()
    demonstrate_cyclic_graph()