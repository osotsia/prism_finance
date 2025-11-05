"""
Defines the user-facing graph construction API (Canvas and Var).
"""
from typing import List, Union
from . import _core  # Import the compiled Rust extension module


class Var:
    """Represents a variable (a node) in the financial model."""

    def __init__(self, canvas: 'Canvas', node_id: int, name: str):
        if not isinstance(canvas, Canvas):
            raise TypeError("Var must be associated with a Canvas.")

        self._canvas = canvas
        self._node_id = node_id
        self._name = name

    def __repr__(self) -> str:
        return f"Var(name='{self._name}', id={self._node_id})"

    def __add__(self, other: 'Var') -> 'Var':
        """Overloads the '+' operator to build the graph."""
        if self._canvas is not other._canvas:
            raise ValueError("Cannot perform operations on Vars from different Canvases.")

        # 1. Create a new formula node in the Rust graph
        new_name = f"({self._name} + {other._name})"
        child_id = self._canvas._graph.add_formula_add(
            parents=[self._node_id, other._node_id],
            name=new_name
        )

        # 2. Add dependencies from parents to the new child node
        self._canvas._graph.add_dependency(self._node_id, child_id)
        self._canvas._graph.add_dependency(other._node_id, child_id)

        # 3. Return a new Var representing the result
        return Var(canvas=self._canvas, node_id=child_id, name=new_name)


class Canvas:
    """
    The main container for a financial model's computation graph.

    Acts as a factory for `Var` objects and an interface to the
    underlying Rust calculation engine.
    """

    def __init__(self):
        # Instantiate the Rust graph object from the `_core` module
        self._graph = _core._ComputationGraph()

    # --- UPDATED METHOD ---
    def add_var(
        self,
        value: Union[int, float, List[float]],
        name: str,
        *, # Makes subsequent arguments keyword-only
        unit: str = None,
        temporal_type: str = None,
    ) -> Var:
        """
        Adds a new constant variable to the graph with optional type metadata.

        Args:
            value: The constant value.
            name: A human-readable name for the variable.
            unit: The unit of measurement (e.g., "USD", "kW").
            temporal_type: The temporal type ("Stock" or "Flow").
        
        Returns:
            A `Var` object representing this new variable.
        """
        val_list = [float(value)] if isinstance(value, (int, float)) else [float(v) for v in value]
        
        # NOTE: The FFI layer `add_constant_node` must be updated to accept this metadata.
        # This change is included in the next step.
        node_id = self._graph.add_constant_node(
            value=val_list,
            name=name,
            unit=unit,
            temporal_type=temporal_type
        )
        return Var(canvas=self, node_id=node_id, name=name)
    
    # --- NEW METHOD ---
    def validate(self) -> None:
        """
        Performs static analysis on the graph.
        
        Raises `ValueError` if any logical inconsistencies are found.
        """
        self._graph.validate()

    def get_evaluation_order(self) -> List[int]:
        """
        Computes and returns a valid evaluation order for all nodes.

        Raises:
            ValueError: If the model contains a circular dependency.
        """
        return self._graph.topological_order()

    @property
    def node_count(self) -> int:
        """Returns the total number of nodes in the graph."""
        return self._graph.node_count()