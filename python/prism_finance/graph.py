"""
Defines the user-facing graph construction API (Canvas and Var).
"""
import warnings
from typing import List, Union, overload
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

    def _create_binary_op(self, other: 'Var', op_name: str, op_symbol: str) -> 'Var':
        """Helper method to create a new Var from a binary operation."""
        if not isinstance(other, Var) or self._canvas is not other._canvas:
            raise ValueError("Operations are only supported between Vars from the same Canvas.")

        new_name = f"({self._name} {op_symbol} {other._name})"
        
        # Call the unified FFI function for binary operations.
        # This is more robust than dynamically looking up function names.
        child_id = self._canvas._graph.add_binary_formula(
            op_name=op_name,
            parents=[self._node_id, other._node_id],
            name=new_name
        )
        return Var(canvas=self._canvas, node_id=child_id, name=new_name)

    def __add__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "add", "+")

    def __sub__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "subtract", "-")

    def __mul__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "multiply", "*")

    def __truediv__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "divide", "/")

    def prev(self, lag: int = 1, *, default: 'Var') -> 'Var':
        """
        Creates a new Var that represents the value of this Var in a previous period.
        
        Args:
            lag: The number of periods to look back (default is 1).
            default: The Var to use as a value for the initial periods.
        """
        if not isinstance(default, Var) or self._canvas is not default._canvas:
            raise ValueError("Default for .prev() must be a Var from the same Canvas.")
        if not isinstance(lag, int) or lag < 1:
            raise ValueError("Lag must be a positive integer.")

        new_name = f"{self._name}.prev(lag={lag})"
        child_id = self._canvas._graph.add_formula_previous_value(
            main_parent_idx=self._node_id,
            default_parent_idx=default._node_id,
            lag=lag,
            name=new_name
        )
        return Var(canvas=self._canvas, node_id=child_id, name=new_name)

    def declare_type(self, *, unit: str = None, temporal_type: str = None) -> 'Var':
        """
        Declares the expected type of this Var for static analysis.
        
        When `validate()` is called, the type checker will verify that its
        inferred type for this node matches the type declared here.
        If a type was already set (e.g., during `add_var`), this method
        will overwrite it and issue a warning.

        Args:
            unit: The expected unit (e.g., "USD", "MWh").
            temporal_type: The expected temporal type ("Stock" or "Flow").

        Returns:
            The Var instance, allowing for method chaining.
        """
        old_unit, old_temporal_type = self._canvas._graph.set_node_metadata(
            node_id=self._node_id,
            unit=unit,
            temporal_type=temporal_type
        )

        # A warning is issued only if a new value is provided for an existing,
        # different value, preventing warnings on initial type declaration.
        if unit is not None and old_unit is not None and unit != old_unit:
            warnings.warn(
                f"Overwriting existing unit '{old_unit}' with '{unit}' for Var '{self._name}'.",
                UserWarning,
                stacklevel=2
            )
        
        if temporal_type is not None and old_temporal_type is not None and temporal_type != old_temporal_type:
             warnings.warn(
                f"Overwriting existing temporal_type '{old_temporal_type}' with '{temporal_type}' for Var '{self._name}'.",
                UserWarning,
                stacklevel=2
            )

        return self


class Canvas:
    """
    The main container for a financial model's computation graph.
    """

    def __init__(self):
        self._graph = _core._ComputationGraph()

    def add_var(
        self,
        value: Union[int, float, List[float]],
        name: str,
        *, 
        unit: str = None,
        temporal_type: str = None,
    ) -> Var:
        """
        Adds a new constant variable to the graph with optional type metadata.
        """
        val_list = [float(value)] if isinstance(value, (int, float)) else [float(v) for v in value]
        
        node_id = self._graph.add_constant_node(
            value=val_list,
            name=name,
            unit=unit,
            temporal_type=temporal_type
        )
        return Var(canvas=self, node_id=node_id, name=name)
    
    def validate(self) -> None:
        """
        Performs static analysis on the graph.
        
        This process involves two steps for each formula node:
        1. Inference: The type is inferred from its parents. Errors like
           adding "USD" and "MWh" are caught here.
        2. Verification: If a type was explicitly set using `.declare_type()`,
           the inferred type is checked against the declared type.
        """
        self._graph.validate()

    def get_evaluation_order(self) -> List[int]:
        """
        Computes a valid evaluation order for all nodes.
        """
        return self._graph.topological_order()

    @property
    def node_count(self) -> int:
        return self._graph.node_count()