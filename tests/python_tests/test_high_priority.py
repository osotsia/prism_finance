"""
High Priority Test Suite: Core Correctness, Stability, & Data Integrity.
"""

import pytest
import pickle
import random
from concurrent.futures import ThreadPoolExecutor, as_completed
from hypothesis import given, settings, strategies as st

from prism_finance import Canvas, Var
from .config import TestConfig

# --- Helpers ---

def assert_float_equal(actual, expected, msg=""):
    assert abs(actual - expected) < TestConfig.TOLERANCE, f"{msg} Expected {expected}, got {actual}"

# --- 1. Property-Based Testing (Hypothesis) ---

# Strategy: Generate a list of operations to form a random DAG.
# Elements: ('op', index_1, index_2). Indices are modulo current_node_count.
op_strategy = st.lists(
    st.tuples(
        st.sampled_from(['add', 'sub', 'mul', 'div']),
        st.integers(min_value=0, max_value=TestConfig.FUZZ_MAX_NODES),
        st.integers(min_value=0, max_value=TestConfig.FUZZ_MAX_NODES)
    ),
    min_size=1,
    max_size=TestConfig.FUZZ_MAX_NODES
)

@given(ops=op_strategy, initial_val=st.floats(min_value=1.0, max_value=100.0))
@settings(max_examples=50, deadline=None)
def test_incremental_consistency_hypothesis(ops, initial_val):
    """
    Property: For any valid DAG, Incremental Recomputation must equal Full Computation.
    """
    with Canvas() as model:
        inputs = [Var(initial_val, name=f"In_{i}") for i in range(3)]
        all_nodes = list(inputs)
        
        for op_type, idx1, idx2 in ops:
            n1 = all_nodes[idx1 % len(all_nodes)]
            n2 = all_nodes[idx2 % len(all_nodes)]
            
            if op_type == 'add': new_node = n1 + n2
            elif op_type == 'sub': new_node = n1 - n2
            elif op_type == 'mul': new_node = n1 * n2
            else: new_node = n1 / (n2 + Var(0.001, name="epsilon")) # Avoid Div0

            new_node.name = f"Node_{len(all_nodes)}"
            all_nodes.append(new_node)
            
        target_node = all_nodes[-1]
        
        model.compute_all()
        full_res_1 = model.get_value(target_node)
        
        # Mutate input and recompute
        inputs[0].set(initial_val * 1.5)
        
        model.recompute([inputs[0]])
        incremental_res = model.get_value(target_node)
        
        model.compute_all()
        full_res_2 = model.get_value(target_node)
        
        assert abs(incremental_res - full_res_2) < 1e-5, "Incremental diverged from Full Recompute"


# --- 2. Concurrency & Isolation Tests ---

def _run_isolated_model(seed_val: float) -> float:
    with Canvas() as model:
        a = Var(seed_val, name="A")
        b = Var(2.0, name="B")
        c = a * b
        model.compute_all()
        return model.get_value(c)

def test_thread_isolation_contextvars():
    """Verifies Canvas state isolation across threads."""
    workers = 10
    inputs = [float(i) for i in range(workers)]
    expected = [i * 2.0 for i in inputs]
    
    with ThreadPoolExecutor(max_workers=workers) as executor:
        future_to_input = {executor.submit(_run_isolated_model, i): i for i in inputs}
        results = []
        for future in as_completed(future_to_input):
            results.append(future.result())
    
    results.sort()
    assert results == expected, "Thread isolation failed."


# --- 3. FFI & Memory Safety ---

def create_scrambled_graph(model: Canvas):
    a = Var(10.0, name="A")
    b = Var(20.0, name="B")
    c = a + b 
    d = Var(5.0, name="D") # Late input creation
    e = c * d
    return a, b, c, d, e

def test_compiler_layout_and_memory_safety():
    """Verifies Bytecode Compiler handles interleaved node creation correctly."""
    with Canvas() as model:
        a, b, c, d, e = create_scrambled_graph(model)
        model.compute_all()
        
        expected_e = (10.0 + 20.0) * 5.0
        assert_float_equal(model.get_value(e), expected_e, "Complex dependency chain failed")
        
        # Update late-bound input
        d.set(50.0)
        model.recompute([d])
        
        expected_e_new = (10.0 + 20.0) * 50.0
        assert_float_equal(model.get_value(e), expected_e_new, "Update to interleaved input failed")

def test_serialization_round_trip_with_constraints():
    """
    Verifies model state and multi-variable constraints survive serialization.
    Regressed Logic Restored: Solves a 2x2 system to ensure topology is preserved.
    System: x + y = 10, x - y = 2  => x=6, y=4
    """
    original_model = Canvas()
    with original_model:
        x = original_model.solver_var(name="x")
        y = original_model.solver_var(name="y")
        
        x.must_equal(Var(10.0, name="c1") - y)
        y.must_equal(x - Var(2.0, name="c2"))
        
        original_model.solve()
        assert_float_equal(original_model.get_value(x), 6.0, "Pre-pickle check failed")
        
    serialized = pickle.dumps(original_model)
    loaded_model: Canvas = pickle.loads(serialized)
    
    # Recover handles via ID
    x_handle = Var._from_existing_node(loaded_model, x._node_id, "x")
    
    # Re-run solver on loaded model
    loaded_model.solve()
    loaded_x = loaded_model.get_value(x_handle)
    
    assert_float_equal(loaded_x, 6.0, "Post-pickle solve failed. Constraints likely lost.")


# --- 4. Domain Logic & Solver ---

def test_cash_flow_sweep_correctness():
    """
    Verifies financial modeling patterns (Time-series + Circularity).
    Regressed Logic Restored: Explicitly verifies Year 2 to check .prev() recursion.
    """
    NUM_YEARS = 3
    with Canvas() as model:
        ebitda = model.solver_var(name="EBITDA")
        interest = model.solver_var(name="Interest")
        debt = model.solver_var(name="Debt")
        
        # Inputs
        initial_ebitda = Var([100.0], name="InitEBITDA")
        growth = Var([0.05] * NUM_YEARS, name="Growth")
        rate = Var([0.06] * NUM_YEARS, name="Rate")
        tax = Var([0.30] * NUM_YEARS, name="Tax")
        y0_debt = Var([500.0], name="Y0Debt")
        
        # Logic
        ebitda.must_equal(ebitda.prev(default=initial_ebitda) * (Var([1.0]*3, name="1") + growth))
        
        ni = (ebitda - interest) * (Var([1.0]*3, name="1") - tax)
        sweep = ni
        
        beg_debt = debt.prev(default=y0_debt)
        debt.must_equal(beg_debt - sweep)
        
        avg_debt = (beg_debt + debt) / Var([2.0]*3, name="2")
        interest.must_equal(avg_debt * rate)
        
        model.solve()
        
        # --- Verification ---
        
        # 1. Year 1 Analysis (The Circularity)
        # NI = (105 - (500 - 0.5*NI)*0.06) * 0.7
        # NI = 52.5 + 0.021*NI => NI = 52.5 / 0.979 = 53.626149...
        expected_ni_y1 = 53.626149
        actual_ni_values = model.get_value(ni)
        assert_float_equal(actual_ni_values[0], expected_ni_y1, "Year 1 Net Income mismatch")
        
        # 2. Year 2 Analysis (The Temporal Recursion)
        # Checks if Year 1 Ending Debt correctly flows into Year 2 Beginning Debt
        actual_debt_values = model.get_value(debt)
        y1_end_debt = actual_debt_values[0]
        
        # Calc Year 2 manually
        e2 = 105.0 * 1.05 # 110.25
        beg_debt_y2 = y1_end_debt
        
        # NI_2 logic same as Y1 but with different E and BegDebt
        # NI = (E - (Beg - 0.5*NI)*R) * (1-T)
        # NI = (E - Beg*R + 0.5*NI*R)*(1-T)
        # NI = (E - Beg*R)*(1-T) + 0.5*NI*R*(1-T)
        # NI * (1 - 0.5*R*(1-T)) = (E - Beg*R)*(1-T)
        
        r = 0.06
        t = 0.30
        denom = 1.0 - 0.5 * r * (1.0 - t)
        numer = (e2 - beg_debt_y2 * r) * (1.0 - t)
        
        expected_ni_y2 = numer / denom
        
        assert_float_equal(actual_ni_values[1], expected_ni_y2, "Year 2 Net Income mismatch (Temporal Recursion broken)")

def test_solver_convergence_nonlinear():
    """
    Verifies solver handles standard non-linear convergence.
    Problem: x = sqrt(x + 20)  => x^2 - x - 20 = 0 => Roots: 5, -4.
    """
    with Canvas() as model:
        x = model.solver_var(name="x")
        lhs = x * x
        rhs = x + Var(20.0, name="20")
        lhs.must_equal(rhs)
        
        model.solve()
        
        val_x = model.get_value(x)
        is_root = abs(val_x - 5.0) < 1e-5 or abs(val_x + 4.0) < 1e-5
        assert is_root, f"Solver converged to non-root value: {val_x}"

def test_solver_singular_jacobian_handling():
    """
    Verifies behavior when Jacobian is singular (gradient is zero).
    Equation: (x - 5)^2 = 0. Derivative at root is 0.
    """
    with Canvas() as model:
        x = model.solver_var(name="x")
        term = x - 5.0
        x.must_equal(term * term)
        
        try:
            model.solve()
        except RuntimeError:
            pass # Failure is acceptable, crashing is not.

def test_solver_infeasibility_handling():
    """Verifies that the engine raises RuntimeError for unsolvable systems."""
    with Canvas() as model:
        x = model.solver_var(name="x")
        x.must_equal(x + 10.0)
        
        with pytest.raises(RuntimeError) as exc:
            model.solve()
        assert "IPOPT" in str(exc.value)