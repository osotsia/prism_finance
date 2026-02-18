"""
High Priority Test Suite: Core Correctness, Stability, & Data Integrity.

Failure in these tests indicates critical flaws in the engine's memory model,
serialization logic, or numerical solver.
"""

import pytest
import pickle
import random
import math
from prism_finance import Canvas, Var

# --- Constants & Helpers ---
TOLERANCE = 1e-9

def assert_float_equal(actual, expected, msg=""):
    assert abs(actual - expected) < TOLERANCE, f"{msg} Expected {expected}, got {actual}"

def create_scrambled_graph(model: Canvas, seed: int = 42):
    """
    Creates a graph where Node Creation Order != Dependency Order.
    This stresses the Compiler's ability to map Logical IDs to Physical Indices.
    """
    rng = random.Random(seed)
    
    # 1. Create standard inputs
    a = Var(10.0, name="A")
    b = Var(20.0, name="B")
    
    # 2. Create a formula immediately
    c = a + b  # C depends on A, B
    
    # 3. Create a NEW input after a formula (Interleaved creation)
    d = Var(5.0, name="D")
    
    # 4. Create formula using the late input and early formula
    e = c * d  # E depends on C, D
    
    # 5. Create a disconnected node
    f = Var(99.0, name="F_Disconnected")
    
    return a, b, c, d, e

# --- Tests ---

def test_compiler_layout_and_memory_safety():
    """
    Verifies that the Bytecode Compiler correctly handles graphs where
    inputs and formulas are interleaved during creation.
    
    Failure Mode: Segfault or reading garbage data due to incorrect 
    Physical Index mapping in Rust.
    """
    with Canvas() as model:
        a, b, c, d, e = create_scrambled_graph(model)
        
        model.compute_all()
        
        # Verify correctness independently
        val_a = 10.0
        val_b = 20.0
        val_d = 5.0
        expected_c = val_a + val_b
        expected_e = expected_c * val_d
        
        assert_float_equal(model.get_value(e), expected_e, "Complex dependency chain failed")
        
        # Verify updates work on the late-bound input
        new_d = 50.0
        d.set(new_d)
        model.recompute([d])
        
        expected_e_new = expected_c * new_d
        assert_float_equal(model.get_value(e), expected_e_new, "Update to interleaved input failed")


def test_incremental_consistency_fuzz():
    """
    Property-Based Test: Asserts that Incremental Recompute yields 
    identical results to Full Compute for random input perturbations.
    """
    ITERATIONS = 5
    NODES = 50
    
    with Canvas() as model:
        # 1. Generate a random deep DAG
        rng = random.Random(1337)
        inputs = [Var(rng.uniform(1, 100), name=f"In_{i}") for i in range(10)]
        nodes = list(inputs)
        
        for i in range(NODES):
            # Pick two random existing nodes to combine
            p1 = rng.choice(nodes)
            p2 = rng.choice(nodes)
            op = rng.choice(['add', 'sub', 'mul'])
            
            if op == 'add': child = p1 + p2
            elif op == 'sub': child = p1 - p2
            else: child = p1 * p2
            
            child.name = f"Node_{i}"
            nodes.append(child)
            
        final_node = nodes[-1]
        
        # 2. Initial Full Compute
        model.compute_all()
        baseline_full = model.get_value(final_node)
        
        # 3. Fuzz Loop
        for i in range(ITERATIONS):
            # a. Pick a random input and mutate it
            target_input = rng.choice(inputs)
            new_val = rng.uniform(1, 100)
            target_input.set(new_val)
            
            # b. Run Incremental
            model.recompute([target_input])
            incremental_result = model.get_value(final_node)
            
            # c. Run Full (Force clean state)
            model.compute_all()
            full_result = model.get_value(final_node)
            
            assert_float_equal(
                incremental_result, 
                full_result, 
                f"Iter {i}: Incremental recompute diverged from full compute."
            )


def test_serialization_round_trip_with_constraints():
    """
    Verifies that the entire model state—including Solver Constraints—survives serialization.
    
    Previous Failure: `__getstate__` dropped constraints, causing `solve()` 
    to be a no-op after loading.
    """
    # 1. Build Model
    original_model = Canvas()
    with original_model:
        # Solve x + y = 10, x - y = 2  => x=6, y=4
        x = original_model.solver_var(name="x")
        y = original_model.solver_var(name="y")
        
        x.must_equal(Var(10.0, name="c1") - y)
        y.must_equal(x - Var(2.0, name="c2"))
        
        # Solve once to prove it works pre-pickle
        original_model.solve()
        pre_pickle_x = original_model.get_value(x)
        assert_float_equal(pre_pickle_x, 6.0, "Pre-pickle solve failed")

    # 2. Serialize
    serialized_bytes = pickle.dumps(original_model)
    
    # 3. Deserialize into NEW instance
    loaded_model: Canvas = pickle.loads(serialized_bytes)
    
    # 4. Recover Handles
    # We use _from_existing_node to wrap the internal integer ID into a Python Var
    # without trying to register a new node in the graph (which requires an active Canvas).
    
    # We know x was node 0 in the original graph (first created).
    # NOTE: In a real app, users should rely on a naming convention lookup or save IDs.
    x_handle = Var._from_existing_node(loaded_model, x._node_id, "x")

    # 5. Solve on Loaded Model
    # If constraints were lost, this would do nothing (x would remain 0.0 or default).
    # If constraints are present, IPOPT runs and finds x=6.0.
    loaded_model.solve()
    
    loaded_x = loaded_model.get_value(x_handle)
    assert_float_equal(loaded_x, 6.0, "Post-pickle solve failed. Constraints likely lost.")


def test_parallel_execution_isolation():
    """
    Verifies that `run_batch` scenarios are perfectly isolated.
    A scenario with huge numbers must not corrupt a scenario with small numbers.
    """
    with Canvas() as model:
        # Logic: result = input * 2
        inp = Var(0.0, name="Input")
        res = inp * 2.0
        
        # Scenario 1: Small
        # Scenario 2: Large
        scenarios = {
            "Small": {inp: 10.0},
            "Large": {inp: 1_000_000.0}
        }
        
        # Use generator
        results = {}
        for name, handle in model.run_batch(scenarios):
            results[name] = handle.get(res)
            
        assert_float_equal(results["Small"], 20.0, "Small scenario corrupted")
        assert_float_equal(results["Large"], 2_000_000.0, "Large scenario corrupted")


def test_solver_convergence_nonlinear():
    """
    Verifies the solver handles non-linear convergence correctly.
    Problem: Find x where x = sqrt(x + 20).
    Analytical: x^2 - x - 20 = 0 => (x-5)(x+4)=0. Positive root x=5.
    """
    with Canvas() as model:
        x = model.solver_var(name="x")
        
        # x^2 = x + 20
        lhs = x * x
        rhs = x + Var(20.0, name="20")
        
        lhs.must_equal(rhs)
        
        # Solve
        model.solve()
        
        # Check result
        val_x = model.get_value(x)
        
        # It could converge to 5 or -4 depending on initialization.
        is_root = abs(val_x - 5.0) < 1e-5 or abs(val_x + 4.0) < 1e-5
        assert is_root, f"Solver converged to non-root value: {val_x}"
        
        # Check Residual explicitly
        val_lhs = model.get_value(lhs)
        val_rhs = model.get_value(rhs)
        assert_float_equal(val_lhs, val_rhs, "Constraint residual is not zero")


def test_cash_flow_sweep_correctness():
    """
    Tests specific financial modeling patterns to ensure domain accuracy.
    Adapts 'examples/5_simple_cash_flow_sweep.py'
    
    Validates:
    1. Inter-period dependency (.prev)
    2. Intra-period circularity (Interest -> NI -> Sweep -> Debt -> Interest)
    3. Vectorized solver execution (3-year model)
    """
    NUM_YEARS = 3
    
    with Canvas() as model:
        # --- 1. Inputs ---
        initial_ebitda = Var([100.0], name="Initial EBITDA")
        ebitda_growth = Var([0.05] * NUM_YEARS, name="EBITDA Growth Rate")
        tax_rate = Var([0.30] * NUM_YEARS, name="Tax Rate")
        y0_debt_balance = Var([500.0], name="Y0 Debt Balance")
        interest_rate = Var([0.06] * NUM_YEARS, name="Interest Rate")
        
        # Helper constants
        one = Var([1.0] * NUM_YEARS, name="one")
        two = Var([2.0] * NUM_YEARS, name="two")

        # --- 2. Solver Variables ---
        ebitda = model.solver_var(name="EBITDA")
        interest_expense = model.solver_var(name="Interest Expense")
        debt_balance = model.solver_var(name="Debt Balance")

        # --- 3. Logic ---
        # EBITDA Forecast
        ebitda.must_equal(ebitda.prev(default=initial_ebitda) * (one + ebitda_growth))

        # Income Statement
        ebt = ebitda - interest_expense
        taxes = ebt * tax_rate
        net_income = ebt - taxes
        
        # Cash Flow & Debt Paydown (The Circularity)
        cash_flow_for_sweep = net_income # Simplified: CF = NI
        
        beginning_debt = debt_balance.prev(default=y0_debt_balance)
        
        # Ending debt is simply Begin - Paydown
        debt_balance.must_equal(beginning_debt - cash_flow_for_sweep)
        
        # Average Debt for Interest Calc
        avg_debt_balance = (beginning_debt + debt_balance) / two
        interest_expense.must_equal(avg_debt_balance * interest_rate)
        
        # --- 4. Solve ---
        model.solve()
        
        # --- 5. Verification ---
        
        # A. Analytical Check for Year 1
        # Variables for Year 1:
        # E1 = 100 * 1.05 = 105
        # BegDebt = 500
        # Int = 0.06
        # Tax = 0.30
        
        # Algebra:
        # NI = (EBITDA - Interest) * (1 - Tax)
        # Interest = AvgDebt * Rate = ((BegDebt + EndDebt) / 2) * Rate
        # EndDebt = BegDebt - NI
        # -> Interest = ((BegDebt + BegDebt - NI) / 2) * Rate
        # -> Interest = (BegDebt - 0.5*NI) * Rate
        
        # Substitute Interest into NI:
        # NI = (EBITDA - (BegDebt - 0.5*NI)*Rate) * (1 - Tax)
        # NI = (EBITDA - BegDebt*Rate + 0.5*NI*Rate) * (1 - Tax)
        # NI = (EBITDA - BegDebt*Rate)*(1-Tax) + 0.5*NI*Rate*(1-Tax)
        # NI - 0.5*NI*Rate*(1-Tax) = (EBITDA - BegDebt*Rate)*(1-Tax)
        # NI * (1 - 0.5*Rate*(1-Tax)) = (EBITDA - BegDebt*Rate)*(1-Tax)
        # NI = ((EBITDA - BegDebt*Rate)*(1-Tax)) / (1 - 0.5*Rate*(1-Tax))
        
        e1 = 105.0
        beg_debt = 500.0
        r = 0.06
        t = 0.30
        
        numerator = (e1 - beg_debt * r) * (1 - t)
        denominator = 1 - 0.5 * r * (1 - t)
        
        expected_ni_y1 = numerator / denominator
        
        actual_ni_values = model.get_value(net_income)
        actual_ni_y1 = actual_ni_values[0]
        
        assert_float_equal(
            actual_ni_y1, 
            expected_ni_y1, 
            "Year 1 Net Income does not match analytical solution."
        )

        # B. Temporal Integrity Check
        # Ensure that Year 2 depends on Year 1 correctly
        actual_debt_values = model.get_value(debt_balance)
        y1_end_debt = actual_debt_values[0]
        
        # Year 2 Calculation manually
        # E2 = 105 * 1.05 = 110.25
        e2 = 110.25
        beg_debt_y2 = y1_end_debt # The roll-forward
        
        numerator_y2 = (e2 - beg_debt_y2 * r) * (1 - t)
        # denominator stays same
        expected_ni_y2 = numerator_y2 / denominator
        
        actual_ni_y2 = actual_ni_values[1]
        
        assert_float_equal(
            actual_ni_y2,
            expected_ni_y2,
            "Year 2 Net Income failed. Time-series roll-forward likely broken."
        )


def test_solver_infeasibility_handling():
    """
    Verifies that the engine raises a clean RuntimeError when the solver
    cannot find a solution (e.g., x = x + 1).
    """
    with Canvas() as model:
        x = model.solver_var(name="x")
        
        # Impossible constraint: x = x + 10
        # Residual: 0 = 10 (Impossible)
        x.must_equal(x + 10.0)
        
        # IPOPT should return status 2 (Infeasible_Problem_Detected) or similar.
        # The Rust engine maps this to a ComputationError, which becomes a RuntimeError in Python.
        with pytest.raises(RuntimeError) as exc_info:
            model.solve()
        
        # Verify the message contains useful info
        assert "IPOPT Solver failed" in str(exc_info.value)