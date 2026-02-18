"""
Low Priority Test Suite: Formatting, UX, & Introspection.

Focuses on:
1. Trace Output Functionality.
"""

import pytest
from prism_finance import Canvas, Var

def test_trace_output_smoke_test(capsys):
    """
    Smoke test for the `.trace()` method.
    Ensures it runs without error and prints something resembling an audit log.
    """
    with Canvas() as model:
        rev = Var(100.0, name="Revenue")
        cost = Var(60.0, name="Costs")
        profit = rev - cost
        profit.name = "Profit"
        
        model.compute_all()
        
        # Run trace (prints to stdout)
        model.trace(profit)
        
        # Capture stdout
        captured = capsys.readouterr()
        output = captured.out
        
        # Check for key structural elements of the trace
        assert "AUDIT TRACE" in output
        assert "Profit" in output
        assert "Revenue" in output
        assert "Costs" in output
        assert "100.000" in output # Value check
        assert "40.000" in output  # Result check

def test_orphaned_nodes_allowed():
    """
    Verifies that creating unused nodes does not crash the engine.
    (Regression for 'Dead Code' handling).
    """
    with Canvas() as model:
        used = Var(10.0, name="Used")
        unused = Var(99.0, name="Unused")
        
        res = used * 2.0
        
        model.compute_all()
        
        assert model.get_value(res) == 20.0
        # Check we can still retrieve the unused value (it sits in the ledger)
        assert model.get_value(unused) == 99.0