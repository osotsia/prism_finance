"""
Defines the user-facing graph construction API (Canvas and Var).
"""
import warnings
from typing import List, Union, overload, Any
from contextvars import ContextVar
from . import _core  # Import the compiled Rust extension module

# A context variable to hold the currently active Canvas instance.
_active_canvas: ContextVar['Canvas'] = ContextVar("active_canvas")


def get_active_canvas() -> 'Canvas':
    """Returns the active Canvas from the current context."""
    try:
        return _active_canvas.get()
    except LookupError:
        raise RuntimeError(
            "A Var can only be created or used inside a 'with Canvas():' block."
        )


class Var:
    """Represents a variable (a node) in the financial model."""

    @staticmethod
    def _normalize_value(value: Any) -> List[float]:
        """A private helper to consistently coerce input values into a list of floats."""
        if isinstance(value, (int, float)):
            return [float(value)]
        return [float(v) for v in value]

    def __init__(
        self,
        value: Union[int, float, List[float]],
        *,
        name: str,
        unit: str = None,
        temporal_type: str = None,
    ):
        if name is None:
            raise ValueError("A 'name' must be provided for each Var.")

        self._canvas = get_active_canvas()
        self._py_name = name
        
        normalized_value = Var._normalize_value(value)
        
        self._node_id = self._canvas._graph.add_constant_node(
            value=normalized_value,
            name=name,
            unit=unit,
            temporal_type=temporal_type
        )

    @classmethod
    def _from_existing_node(cls, canvas: 'Canvas', node_id: int, name: str) -> 'Var':
        """Internal constructor to wrap an existing node_id in a Var object."""
        var_instance = cls.__new__(cls)
        var_instance._canvas = canvas
        var_instance._node_id = node_id
        var_instance._py_name = name
        # The name is already set in the core graph during formula creation.
        return var_instance

    @property
    def name(self) -> str:
        """The human-readable name of the Var."""
        return self._py_name
        
    @name.setter
    def name(self, new_name: str):
        """Sets the name of the Var, updating both Python and the core engine."""
        self._py_name = new_name
        self._canvas._graph.set_node_name(self._node_id, new_name)

    def __repr__(self) -> str:
        return f"Var(name='{self.name}', id={self._node_id})"
    
    def set(self, value: Union[int, float, List[float]]):
        """
        Updates the value of this constant Var. This operation marks the Var
        as 'dirty' for subsequent incremental recomputations.
        
        Raises:
            TypeError: If called on a Var that is not a constant input (e.g., a formula).
        """
        normalized_value = Var._normalize_value(value)
        try:
            self._canvas._graph.update_constant_node(self._node_id, normalized_value)
        except ValueError as e:
            # Re-raise with a more user-friendly message
            raise TypeError(f"Cannot set value for Var '{self.name}'. It may not be a constant input Var.") from e

    def trace(self):
        """
        Convenience method to trace this Var using its parent Canvas.
        Equivalent to `canvas.trace(var)`.
        """
        self._canvas.trace(self)

    def _create_binary_op(self, other: 'Var', op_name: str, op_symbol: str) -> 'Var':
        """Helper method to create a new Var from a binary operation."""
        if not isinstance(other, Var) or self._canvas is not other._canvas:
            raise ValueError("Operations are only supported between Vars from the same Canvas.")

        new_name = f"({self.name} {op_symbol} {other.name})"
        
        child_id = self._canvas._graph.add_binary_formula(
            op_name=op_name,
            parents=[self._node_id, other._node_id],
            name=new_name
        )
        return Var._from_existing_node(canvas=self._canvas, node_id=child_id, name=new_name)

    def __add__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "add", "+")

    def __sub__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "subtract", "-")

    def __mul__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "multiply", "*")

    def __truediv__(self, other: 'Var') -> 'Var':
        return self._create_binary_op(other, "divide", "/")

    def must_equal(self, other: 'Var') -> None:
        """
        Declares a constraint that this Var must equal another Var.
        This is syntactic sugar for `canvas.must_equal(self, other)`.
        """
        if not isinstance(other, Var) or self._canvas is not other._canvas:
            raise ValueError("Constraints can only be set between Vars from the same Canvas.")
        self._canvas.must_equal(self, other)

    def prev(self, lag: int = 1, *, default: 'Var') -> 'Var':
        if not isinstance(default, Var) or self._canvas is not default._canvas:
            raise ValueError("Default for .prev() must be a Var from the same Canvas.")
        if not isinstance(lag, int) or lag < 1:
            raise ValueError("Lag must be a positive integer.")
        new_name = f"{self.name}.prev(lag={lag})"
        
        # Note: We use positional arguments here because the Rust argument name 'def'
        # conflicts with the Python reserved keyword `def`.
        # Rust signature: (main: usize, def: usize, lag: u32, name: String)
        child_id = self._canvas._graph.add_formula_previous_value(
            self._node_id,
            default._node_id,
            lag,
            new_name
        )
        return Var._from_existing_node(canvas=self._canvas, node_id=child_id, name=new_name)

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
        # Rust signature: (id: usize, unit: Option<String>, temporal_type: Option<String>)
        old_unit, old_temporal_type = self._canvas._graph.set_node_metadata(
            id=self._node_id,
            unit=unit,
            temporal_type=temporal_type
        )
        if unit is not None and old_unit is not None and unit != old_unit:
            warnings.warn(f"Overwriting existing unit '{old_unit}' with '{unit}' for Var '{self.name}'.", UserWarning, stacklevel=2)
        if temporal_type is not None and old_temporal_type is not None and temporal_type != old_temporal_type:
             warnings.warn(f"Overwriting existing temporal_type '{old_temporal_type}' with '{temporal_type}' for Var '{self.name}'.", UserWarning, stacklevel=2)
        return self


class Canvas:
    """
    The main container for a financial model's computation graph.
    Designed to be used as a context manager.
    """

    def __init__(self):
        self._graph = _core._ComputationGraph()
        self._token = None
        self._last_ledger: _core._Ledger = None

    def __enter__(self) -> 'Canvas':
        if self._token is not None:
            raise RuntimeError("Canvas context is not re-entrant.")
        self._token = _active_canvas.set(self)
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        _active_canvas.reset(self._token)
        self._token = None

    def solver_var(self, name: str) -> Var:
        """
        Adds a new variable to the graph whose value will be determined by the solver.
        
        Args:
            name: A unique, human-readable name for the variable.
            
        Returns:
            A `Var` instance representing the solver variable.
        """
        node_id = self._graph.add_solver_variable(name=name)
        return Var._from_existing_node(canvas=self, node_id=node_id, name=name)

    def must_equal(self, var1: Var, var2: Var) -> None:
        """
        Declares a constraint that two Vars must be equal. This forms the basis
        of the system of equations for the solver.
        """
        constraint_name = f"Constraint: {var1.name} == {var2.name}"
        self._graph.must_equal(var1._node_id, var2._node_id, name=constraint_name)
    
    def solve(self) -> None:
        """
        Solves the system of equations and constraints defined in the graph.
        
        This orchestrates the entire process:
        1. Pre-computes all values that are independent of the solver variables.
        2. Runs the numerical solver to find the values of the solver variables.
        3. Runs a final computation pass to calculate all values that depend on the solved variables.
        
        The final, complete set of results is stored internally. Use `.get_value(var)` to retrieve them.
        """
        self._last_ledger = self._graph.solve()

    def compute_all(self) -> None:
        """
        Performs a full computation of all nodes in the graph. This should be
        called once to establish the initial state of the model before any
        incremental recomputations.
        """
        if self._last_ledger is None:
            self._last_ledger = _core._Ledger()
        
        # The `targets` parameter in the core engine is currently unused but
        # required by the function signature. We pass all nodes. The engine
        # will compute everything not already in the ledger.
        all_node_ids = list(range(self._graph.node_count()))
        self._graph.compute(targets=all_node_ids, ledger=self._last_ledger, changed_inputs=None)

    def recompute(self, changed_vars: List[Var]) -> None:
        """
        Performs an incremental recomputation of the graph based on a list
        of constant Vars that have been updated via `.set()`.

        Only the provided Vars and their downstream dependencies will be
        recalculated.

        Args:
            changed_vars: A list of `Var` objects whose values have changed.

        Raises:
            RuntimeError: If `.compute_all()` or `.solve()` has not been called first.
        """
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before recomputing.")
        
        changed_ids = [v._node_id for v in changed_vars]
        all_node_ids = list(range(self._graph.node_count())) # `targets` is required but unused.
        self._graph.compute(targets=all_node_ids, ledger=self._last_ledger, changed_inputs=changed_ids)

    def get_value(self, target_var: Var) -> Union[float, List[float]]:
        """
        Retrieves the value of a target Var from the most recent computation.
        
        Args:
            target_var: The Var whose value you want to retrieve.
        
        Returns:
            The computed value, either as a float (for scalar results) or a list of floats
            (for time-series results).
            
        Raises:
            RuntimeError: If a computation has not been run yet.
            ValueError: If the value for the target Var is not found in the results.
        """
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before requesting a value.")
        
        values = self._last_ledger.get_value(target_var._node_id)
        if values is None:
            raise ValueError(f"Value for '{target_var.name}' not found in the ledger.")
            
        # Optimization #1 unified the ledger to vectors (length N).
        # We need to unwrap this back to float if it is effectively a scalar.
        
        # Case 1: The model (or this variable) has length 1. It is structurally a scalar.
        if len(values) == 1:
            return values[0]

        # Case 2: The model is a time-series (len > 1), but this variable is a constant scalar
        # that was broadcasted (e.g., "Tax Rate"). We check the structural type in Rust.
        if self._graph.is_scalar(target_var._node_id):
            return values[0]
            
        # Case 3: It is a true time-series.
        return values

    def trace(self, target_var: Var):
        """
        Generates and prints a step-by-step audit trace for a given Var,
        showing how it was derived from its ultimate inputs.

        Args:
            target_var: The Var to trace.
        
        Raises:
            RuntimeError: If a computation has not been run yet.
        """
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before tracing.")
        
        trace_output = self._graph.trace_node(target_var._node_id, self._last_ledger)
        print(trace_output)

    def validate(self) -> None:
        self._graph.validate()

    def get_evaluation_order(self) -> List[int]:
        return self._graph.topological_order()

    @property
    def node_count(self) -> int:
        return self._graph.node_count()