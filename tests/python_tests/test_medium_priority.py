"""
Medium Priority Test Suite: Logic, Validation, & Edge Cases.

Focuses on:
1. Static Type Checking (Units/Temporal Types).
2. Vector Broadcasting Rules.
3. Arithmetic Edge Cases (NaNs).
4. Graph Topology Limits (Recursion depth).
"""

import pytest
import math
from prism_finance import Canvas, Var

# --- 1. Static Validation Tests (Ported from test_type_system.py) ---

def test_validation_unit_mismatch():
    """
    Verifies that the engine detects invalid unit arithmetic (e.g., USD + MWh).
    """
    with Canvas() as model:
        # Define incompatible inputs
        rev = Var(100.0, name="Revenue", unit="USD")
        vol = Var(50.0, name="Volume", unit="MWh")

        # Invalid operation
        _result = rev + vol

        # Expect validation error
        with pytest.raises(ValueError) as exc_info:
            model.validate()
        
        assert "Unit Mismatch" in str(exc_info.value)
        assert "USD" in str(exc_info.value)
        assert "MWh" in str(exc_info.value)

def test_validation_temporal_mismatch():
    """
    Verifies that the engine detects invalid temporal logic (Stock + Stock).
    Flow + Flow = Flow (OK)
    Stock + Stock = Ambiguous (Error)
    """
    with Canvas() as model:
        # Balance + Balance is usually physically meaningless 
        # (you don't add your bank balance today to your balance yesterday).
        b1 = Var(100.0, name="Opening Balance", temporal_type="Stock")
        b2 = Var(120.0, name="Closing Balance", temporal_type="Stock")

        _result = b1 + b2

        with pytest.raises(ValueError) as exc_info:
            model.validate()
        
        assert "Ambiguous: Stock +/- Stock" in str(exc_info.value)

def test_validation_successful_model():
    """Verifies that a logically consistent model passes validation."""
    with Canvas() as model:
        price = Var(10.0, name="Price", unit="USD/MWh", temporal_type="Flow")
        vol = Var(50.0, name="Volume", unit="MWh", temporal_type="Flow")
        
        # Result unit should be (USD/MWh) * MWh = USD
        revenue = price * vol
        
        # Check no error raised
        model.validate()

# --- 2. Vector Broadcasting & Shape Mismatch ---

def test_vector_shape_mismatch():
    """
    Verifies that operations between vectors of different lengths raise a clean error.
    """
    with Canvas() as model:
        v1 = Var([1.0, 2.0, 3.0], name="Vec3")
        v2 = Var([1.0, 2.0], name="Vec2")
        
        # The graph construction is valid, but computation should fail
        # because the Ledger cannot align these inputs.
        _res = v1 + v2
        
        with pytest.raises(RuntimeError) as exc_info:
            model.compute_all()
        
        # Rust error: "Structural mismatch: Input len 2 != Model len 3"
        # Fix: Checking for lowercase "mismatch" to match Rust output.
        assert "mismatch" in str(exc_info.value).lower()

def test_scalar_vector_broadcasting():
    """
    Verifies that Scalars broadcast correctly against Vectors.
    """
    with Canvas() as model:
        vec = Var([10.0, 20.0, 30.0], name="Vector")
        scalar = Var(5.0, name="Scalar")
        
        # Vector + Scalar
        res = vec + scalar
        
        model.compute_all()
        
        vals = model.get_value(res)
        assert vals == [15.0, 25.0, 35.0]

# --- 3. Arithmetic Edge Cases ---

def test_nan_propagation():
    """
    Verifies that mathematical errors (0/0) propagate as NaN rather than panicking.
    This allows users to debug models using the Trace output.
    """
    with Canvas() as model:
        num = Var(0.0, name="Zero")
        den = Var(0.0, name="ZeroDenom")
        
        # 0 / 0 = NaN
        res = num / den
        
        model.compute_all()
        
        val = model.get_value(res)
        assert math.isnan(val), "0/0 should result in NaN"

# --- 4. Graph Topology Stress ---

def test_deep_recursion_depth():
    """
    Stress tests the Topological Sort algorithm.
    Creates a deep dependency chain A -> B -> C ... -> Z.
    
    If the Rust implementation uses naive recursion, this might stack overflow.
    This test ensures we can handle at least moderate depth (2000 nodes).
    """
    DEPTH = 2000
    with Canvas() as model:
        curr = Var(1.0, name="Root")
        
        for i in range(DEPTH):
            # Chain: next = prev + 1
            curr = curr + 1.0
            
        model.compute_all()
        
        final_val = model.get_value(curr)
        expected = 1.0 + DEPTH
        
        assert abs(final_val - expected) < 1e-6