"""
Defines the user-facing graph construction API (Canvas and Var).

Architecture Overview:
1. Canvas: The graph lifecycle container. Manages the link between the 
   structural Registry and the physical Ledger (data storage).
2. Var: A lightweight proxy (handle) for a graph node. Contains only the 
   NodeId and a reference to its parent Canvas.
3. Scalar Promotion: Automatic conversion of Python constants (int/float) 
   into Var nodes during arithmetic to ensure DSL fluency.
4. Batching: Release of the GIL to allow Rust/Rayon to compute multiple 
   Ledger instances in parallel for scenario analysis.
"""

import warnings
from typing import List, Union, Dict, Any, Optional
from contextvars import ContextVar
from . import _core  # Compiled Rust extension module

# Thread-safe context for implicit Canvas reference
_active_canvas: ContextVar['Canvas'] = ContextVar("active_canvas")


def get_active_canvas() -> 'Canvas':
    """Retrieves the Canvas currently managing node creation in this thread."""
    try:
        return _active_canvas.get()
    except LookupError:
        raise RuntimeError(
            "Prism variables (Var) must be created within a 'with Canvas():' block. "
            "Ensure you have initialized a context manager."
        )


class ScenarioResult:
    """
    A read-only handle for data from a specific scenario in a parallel batch.
    
    This acts as a window into a specific Rust _Ledger, using the structural 
    metadata of the Canvas to correctly interpret vectorized vs. scalar data.
    """
    def __init__(self, canvas: 'Canvas', ledger: _core._Ledger):
        self._canvas = canvas
        self._ledger = ledger

    def get(self, target_var: 'Var') -> Union[float, List[float]]:
        """Retrieves values from the scenario-specific memory ledger."""
        values = self._canvas._graph.get_value(self._ledger, target_var._node_id)
        if values is None:
            raise ValueError(f"Value for '{target_var.name}' not found in scenario.")
        
        # Scalar Unwrapping Logic
        if len(values) == 1 or self._canvas._graph.is_scalar(target_var._node_id):
            return values[0]
        return values


class Var:
    """A proxy representing a node in the financial calculation graph."""

    @staticmethod
    def _normalize_value(value: Any) -> List[float]:
        """Consistently coerces scalars or iterables into Rust-compatible float vectors."""
        if isinstance(value, (int, float)):
            return [float(value)]
        try:
            return [float(v) for v in value]
        except (TypeError, ValueError):
            # Fallback for complex types that might be float-convertible
            return [float(value)]

    def __init__(
        self,
        value: Union[int, float, List[float]],
        *,
        name: str,
        unit: str = None,
        temporal_type: str = None,
    ):
        if name is None:
            raise ValueError("A human-readable 'name' is required for all Var nodes.")

        self._canvas = get_active_canvas()
        self._py_name = name
        
        normalized_value = Var._normalize_value(value)
        self._node_id = self._canvas._graph.add_constant_node(
            value=normalized_value,
            name=name,
            unit=unit,
            temporal_type=temporal_type
        )
        # Note: Name uniqueness is now handled by Rust backend (auto-suffixing)

    @classmethod
    def _from_existing_node(cls, canvas: 'Canvas', node_id: int, name: str) -> 'Var':
        """Internal: Wraps a raw Rust NodeId into the Python API."""
        var_instance = cls.__new__(cls)
        var_instance._canvas = canvas
        var_instance._node_id = node_id
        var_instance._py_name = name
        return var_instance

    @property
    def name(self) -> str:
        return self._py_name
        
    @name.setter
    def name(self, new_name: str):
        self._py_name = new_name
        self._canvas._graph.set_node_name(self._node_id, new_name)

    def set(self, value: Union[int, float, List[float]]):
        """Updates constant input values. Marks node dirty for incremental recompute."""
        normalized_value = Var._normalize_value(value)
        try:
            self._canvas._graph.update_constant_node(self._node_id, normalized_value)
        except ValueError:
            raise TypeError(f"Cannot 'set' Var '{self.name}'. It is a calculated formula.")

    def get_value(self) -> Union[float, List[float]]:
        """Retrieves current value from the parent Canvas ledger."""
        return self._canvas.get_value(self)

    def trace(self):
        """Generates an audit trail of the logic and data contributing to this node."""
        self._canvas.trace(self)

    def _promote(self, other: Any) -> 'Var':
        """
        Syntactic Sugar: Promotes Python constants to Var nodes.
        This allows 'x + 10' to be equivalent to 'x + Var(10)'.
        """
        if isinstance(other, Var):
            return other
        # Create an internal constant node for the scalar
        return Var(other, name=f"const({other})")

    def _create_binary_op(self, other: Any, op_name: str, op_symbol: str) -> 'Var':
        """Helper to register binary arithmetic formulas in the Rust core."""
        other_var = self._promote(other)
        
        if self._canvas is not other_var._canvas:
            raise ValueError("Cross-canvas operations are prohibited.")

        new_name = f"({self.name} {op_symbol} {other_var.name})"
        child_id = self._canvas._graph.add_binary_formula(
            op_name=op_name,
            parents=[self._node_id, other_var._node_id],
            name=new_name
        )
        return Var._from_existing_node(self._canvas, child_id, new_name)

    # Arithmetic Operator Overloading
    def __add__(self, other): return self._create_binary_op(other, "add", "+")
    def __radd__(self, other): return self._promote(other) + self

    def __sub__(self, other): return self._create_binary_op(other, "subtract", "-")
    def __rsub__(self, other): return self._promote(other) - self

    def __mul__(self, other): return self._create_binary_op(other, "multiply", "*")
    def __rmul__(self, other): return self._promote(other) * self

    def __truediv__(self, other): return self._create_binary_op(other, "divide", "/")
    def __rtruediv__(self, other): return self._promote(other) / self

    def must_equal(self, other: Any) -> None:
        """Syntax sugar: delegates constraint registration to the Canvas."""
        other_var = self._promote(other)
        self._canvas.must_equal(self, other_var)

    def prev(self, lag: int = 1, *, default: Any) -> 'Var':
        """
        Registers a temporal lookback operation. 
        'default' is used for periods before the lag horizon.
        """
        default_var = self._promote(default)
        new_name = f"{self.name}.prev(lag={lag})"
        child_id = self._canvas._graph.add_formula_previous_value(
            self._node_id,
            default_var._node_id,
            lag,
            new_name
        )
        return Var._from_existing_node(self._canvas, child_id, new_name)

    def declare_type(self, *, unit: str = None, temporal_type: str = None) -> 'Var':
        """Declares metadata and issues warnings if existing types are overwritten."""
        old_u, old_t = self._canvas._graph.set_node_metadata(self._node_id, unit, temporal_type)
        if unit and old_u and unit != old_u:
            warnings.warn(f"Overwriting existing unit '{old_u}' with '{unit}' for Var '{self.name}'.", UserWarning, stacklevel=2)
        if temporal_type and old_t and temporal_type != old_t:
            warnings.warn(f"Overwriting existing temporal_type '{old_t}' with '{temporal_type}' for Var '{self.name}'.", UserWarning, stacklevel=2)
        return self


class Canvas:
    """
    The orchestrator for the calculation graph. 
    Encapsulates topology (the Registry) and state (the Ledger).
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

    def __getstate__(self):
        # We only need to serialize the graph. Token and last_ledger are transient.
        return {'graph': self._graph}

    def __setstate__(self, state):
        self._graph = state['graph']
        self._token = None
        self._last_ledger = None
    # ----------------------

    def solver_var(self, name: str) -> Var:
        """Adds an unknown variable to be determined by the numerical solver."""
        node_id = self._graph.add_solver_variable(name=name)
        return Var._from_existing_node(canvas=self, node_id=node_id, name=name)

    def must_equal(self, var1: Var, var2: Var) -> None:
        """Registers a simultaneous equation constraint: var1 - var2 = 0."""
        constraint_name = f"Constraint: {var1.name} == {var2.name}"
        self._graph.must_equal(var1._node_id, var2._node_id, name=constraint_name)
    
    def solve(self, tol: float = 1e-9, max_iter: int = 3000) -> None:
        """
        Executes the non-linear solver.
        
        Args:
            tol: Convergence tolerance for the solver.
            max_iter: Maximum number of iterations before stopping.
        """
        config = _core.PySolverConfig(tol=tol, max_iter=max_iter)
        self._last_ledger = self._graph.solve(config)

    def compute_all(self) -> None:
        """Performs a full pass of the calculation engine."""
        self._last_ledger = _core._Ledger()
        self._graph.compute(ledger=self._last_ledger, changed_inputs=None)

    def recompute(self, changed_vars: List[Var]) -> None:
        """Triggers incremental calculation for a dirty subset of the graph."""
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before recomputing.")
        
        changed_ids = [v._node_id for v in changed_vars]
        self._graph.compute(ledger=self._last_ledger, changed_inputs=changed_ids)

    def run_batch(
            self, 
            scenarios: Dict[str, Dict[Var, Any]], 
            chunk_size: Optional[int] = None
        ): # Removed Dict return type hint to support Generator
            """
            Executes multiple scenarios in parallel using a generator to manage memory.
            Yields: (scenario_name, ScenarioResult)
            """
            if not scenarios:
                return

            self._graph.topological_order()

            from .graph import Var
            items = list(scenarios.items())
            batch_limit = chunk_size if chunk_size else len(items)

            for i in range(0, len(items), batch_limit):
                # 1. Prepare only the current chunk
                chunk_slice = items[i : i + batch_limit]
                prepared_chunk = {
                    name: {v._node_id: Var._normalize_value(val) for v, val in overrides.items()}
                    for name, overrides in chunk_slice
                }

                # 2. Compute the chunk in Rust
                raw_batch_results = self._graph.compute_batch(prepared_chunk)
                
                # 3. Yield results one by one
                for name, ledger in raw_batch_results.items():
                    yield name, ScenarioResult(self, ledger)
                
                # 4. Explicitly clear the raw results to help GC
                raw_batch_results.clear()

    def get_value(self, target_var: Var) -> Union[float, List[float]]:
        """Retrieves values for a Var from the last computed ledger."""
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before requesting a value.")
        
        values = self._graph.get_value(self._last_ledger, target_var._node_id)
        if values is None:
            raise ValueError(f"Value for '{target_var.name}' not found in ledger.")
            
        # Optimization: Return scalar if model length is 1 or node is structurally constant
        if len(values) == 1 or self._graph.is_scalar(target_var._node_id):
            return values[0]
        return values

    def trace(self, target_var: Var):
        """Prints the recursive audit trace for the specified variable."""
        if self._last_ledger is None:
            raise RuntimeError("Must call .compute_all() or .solve() before tracing.")
        
        trace_output = self._graph.trace_node(target_var._node_id, self._last_ledger)
        print(trace_output)

    def validate(self) -> None:
        """Performs static analysis to detect unit mismatches or logical errors."""
        self._graph.validate()
    
    def telemetry(self) -> Dict[str, Any]:
        """
        Returns statistical analysis of the compiled execution plan.
        Useful for auditing performance and cache efficiency.
        """
        return self._graph.get_telemetry()

    def get_evaluation_order(self) -> List[int]:
        """Returns the topological execution sequence of the graph."""
        return self._graph.topological_order()

    @property
    def node_count(self) -> int:
        """Total number of nodes currently in the Registry."""
        return self._graph.node_count()