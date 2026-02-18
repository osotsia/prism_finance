"""
Medium Priority Test Suite: Logic, Validation, & Edge Cases.
"""

import pytest
import math
import warnings
from prism_finance import Canvas, Var
from .config import TestConfig

# --- Fixtures ---

@pytest.fixture
def valid_model():
    """
    Provides a Canvas pre-populated with common financial variable types.
    Yields inside the context so tests can add new variables (e.g., scalars)
    without triggering a RuntimeError.
    """
    model = Canvas()
    with model:
        data = {
            "rev_usd": Var(100.0, name="Revenue", unit="USD", temporal_type="Flow"),
            "cost_usd": Var(60.0, name="Costs", unit="USD", temporal_type="Flow"),
            "vol_mwh": Var(50.0, name="Volume", unit="MWh", temporal_type="Flow"),
            "bal_stock": Var(100.0, name="Balance", unit="USD", temporal_type="Stock"),
            "untyped": Var(10.0, name="Untyped"),
        }
        yield model, data

# --- 1. Static Validation (Type System) ---

def test_validation_unit_mismatch(valid_model):
    """Verifies detection of invalid unit arithmetic (e.g., USD + MWh)."""
    model, v = valid_model
    
    # Action: Invalid Math
    _result = v["rev_usd"] + v["vol_mwh"]

    # Assert
    with pytest.raises(ValueError) as exc:
        model.validate()
    
    msg = str(exc.value)
    assert "Unit Mismatch" in msg
    assert "USD" in msg and "MWh" in msg

def test_validation_temporal_mismatch(valid_model):
    """
    Verifies detection of invalid temporal logic.
    Rule: Stock + Stock = Ambiguous (Error).
    """
    model, v = valid_model
    
    # Action: Invalid Temporal Logic
    _result = v["bal_stock"] + v["bal_stock"]

    # Assert
    with pytest.raises(ValueError) as exc:
        model.validate()
    assert "Ambiguous: Stock +/- Stock" in str(exc.value)

def test_declare_type_overwrite_warnings(valid_model):
    """
    Verifies that overwriting existing type metadata issues a UserWarning.
    """
    model, v = valid_model
    rev = v["rev_usd"] # Already USD/Flow

    # Action 1: Change Unit
    with pytest.warns(UserWarning, match="Overwriting existing unit 'USD' with 'EUR'"):
        rev.declare_type(unit="EUR")

    # Action 2: Change Temporal Type
    with pytest.warns(UserWarning, match="Overwriting existing temporal_type 'Flow'"):
        rev.declare_type(temporal_type="Stock")

def test_declare_type_on_untyped_is_silent(valid_model):
    """
    Verifies that declaring types on a previously untyped var is silent (no warning).
    """
    model, v = valid_model
    untyped = v["untyped"]

    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        untyped.declare_type(unit="USD", temporal_type="Flow")
        assert len(w) == 0, f"Expected no warnings, got: {[str(x.message) for x in w]}"

def test_validation_cache_invalidation(valid_model):
    """
    Verifies that the Rust validation cache updates when node metadata changes.
    """
    model, v = valid_model
    
    # 1. Valid Operation (USD + USD)
    extra_cost = Var(10.0, name="Extra", unit="USD")
    _c = v["rev_usd"] + extra_cost
    
    model.validate() # Should pass
    
    # 2. Mutate to incompatible unit
    # This emits a warning because we are overwriting "USD". 
    # We capture it to keep the test output clean.
    with pytest.warns(UserWarning, match="Overwriting"):
        extra_cost.declare_type(unit="MWh")
    
    # 3. Re-validate
    with pytest.raises(ValueError) as exc:
        model.validate()
    assert "Unit Mismatch" in str(exc.value)

# --- 2. Vector Semantics ---

def test_vector_broadcasting():
    """Verifies Scalar to Vector broadcasting rules."""
    with Canvas() as model:
        vec = Var([10.0, 20.0, 30.0], name="Vector")
        scalar = Var(5.0, name="Scalar")
        
        # [10, 20, 30] + [5, 5, 5]
        res = vec + scalar
        
        model.compute_all()
        assert model.get_value(res) == [15.0, 25.0, 35.0]

def test_vector_shape_mismatch():
    """Verifies that operations on mismatched vector lengths fail cleanly."""
    with Canvas() as model:
        v1 = Var([1.0, 2.0, 3.0], name="Len3")
        v2 = Var([1.0, 2.0], name="Len2")
        
        _res = v1 + v2
        
        with pytest.raises(RuntimeError) as exc:
            model.compute_all()
        assert "mismatch" in str(exc.value).lower()

# --- 3. Arithmetic Stability ---

def test_arithmetic_edge_cases():
    """
    Verifies IEEE 754 behavior for singular arithmetic operations.
    The engine must not panic/segfault on 0/0 or 1/0.
    """
    with Canvas() as model:
        zero = Var(0.0, name="Zero")
        one = Var(1.0, name="One")
        
        nan_res = zero / zero
        inf_res = one / zero
        
        model.compute_all()
        
        val_nan = model.get_value(nan_res)
        val_inf = model.get_value(inf_res)
        
        assert math.isnan(val_nan), "0/0 did not produce NaN"
        assert math.isinf(val_inf), "1/0 did not produce Infinity"

# --- 4. Topology Limits ---

def test_recursion_depth():
    """Ensures deep dependency chains don't overflow the stack."""
    DEPTH = 2000
    with Canvas() as model:
        curr = Var(1.0, name="Root")
        for _ in range(DEPTH):
            curr = curr + 1.0
            
        model.compute_all()
        
        expected = 1.0 + DEPTH
        assert abs(model.get_value(curr) - expected) < TestConfig.TOLERANCE