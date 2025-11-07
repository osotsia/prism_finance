import pytest
import warnings
from prism_finance import Canvas, Var

# --- 1. Test Fixture: Provides Reusable Data ---

@pytest.fixture
def model_with_vars() -> tuple[Canvas, dict[str, Var]]:
    """Provides a Canvas and a dictionary of common Vars for tests."""
    model = Canvas()
    v = {
        "revenue": model.add_var(100, name="Revenue", unit="USD", temporal_type="Flow"),
        "costs": model.add_var(40, name="Costs", unit="USD", temporal_type="Flow"),
        "volume": model.add_var(50, name="Volume", unit="MWh", temporal_type="Flow"),
        "opening_balance": model.add_var(1000, name="OB", unit="USD", temporal_type="Stock"),
        "closing_balance": model.add_var(1200, name="CB", unit="USD", temporal_type="Stock"),
        "untyped_a": model.add_var(10, name="A"),
        "untyped_b": model.add_var(5, name="B"),
    }
    return model, v


# --- 2. Test Case Data Definition ---
# Structure: (id, setup_lambda, should_pass, expected_error_substring)

VALIDATION_TEST_CASES = [
    # "Happy Path": Standard, expected inputs that should work perfectly.
    ("inference_ok", lambda v: v["revenue"] - v["costs"], True, None),
    ("verification_ok", lambda v: (v["revenue"] - v["costs"]).declare_type(unit="USD", temporal_type="Flow"), True, None),

    # Domain-Specific Edge Cases: Operations that are conceptually valid in finance.
    ("stock_plus_stock_is_valid", lambda v: v["opening_balance"] + v["closing_balance"], True, None),
    ("stock_plus_flow_is_valid", lambda v: v["opening_balance"] + v["revenue"], True, None),
    ("stock_plus_flow_infers_stock", lambda v: (v["opening_balance"] + v["revenue"]).declare_type(temporal_type="Stock"), True, None),

    # Type Errors (Inference Failures): Inputs of an incorrect type that should raise an error.
    ("inference_fail_unit_mismatch", lambda v: v["revenue"] + v["volume"], False, "Unit Mismatch"),
    
    # Type Errors (Verification Failures): Declared type mismatches inferred type.
    ("verification_fail_unit_mismatch", lambda v: (v["revenue"] - v["costs"]).declare_type(unit="EUR"), False, "Declared unit 'EUR' does not match inferred unit 'USD'"),
    ("verification_fail_temporal_mismatch", lambda v: (v["revenue"] - v["costs"]).declare_type(temporal_type="Stock"), False, "Declared temporal type 'Stock' does not match inferred type 'Flow'"),
    
    # Empty/Null Inputs (Untyped Vars): Operations on untyped inputs.
    ("untyped_parents_pass_inference", lambda v: v["untyped_a"] + v["untyped_b"], True, None),
    ("untyped_parents_fail_verification", lambda v: (v["untyped_a"] + v["untyped_b"]).declare_type(unit="USD"), False, "Declared unit 'USD' does not match inferred unit 'None'"),
]


# --- 3. Test Execution Logic ---

@pytest.mark.parametrize(
    "id_str, setup_lambda, should_pass, expected_error",
    VALIDATION_TEST_CASES,
    ids=[case[0] for case in VALIDATION_TEST_CASES]
)
def test_validation_scenarios(model_with_vars, id_str, setup_lambda, should_pass, expected_error):
    """
    Executes a single validation test case defined in VALIDATION_TEST_CASES.
    The test logic is separated from the data and setup.
    """
    model, v = model_with_vars
    setup_lambda(v)  # Builds the formula on the graph

    if should_pass:
        try:
            model.validate()
        except ValueError as e:
            pytest.fail(f"Test '{id_str}' failed validation unexpectedly: {e}")
    else:
        with pytest.raises(ValueError, match=expected_error):
            model.validate()


def test_declare_type_overwrite_issues_warning(model_with_vars):
    """
    Tests that calling `declare_type` on a Var that already has a type issues a `UserWarning`.
    """
    _, v = model_with_vars
    revenue = v["revenue"] # This Var already has unit="USD" and temporal_type="Flow"

    with pytest.warns(UserWarning, match="Overwriting existing unit 'USD' with 'EUR'"):
        revenue.declare_type(unit="EUR")
    
    with pytest.warns(UserWarning, match="Overwriting existing temporal_type 'Flow' with 'Stock'"):
        revenue.declare_type(temporal_type="Stock")


def test_declare_type_on_untyped_issues_no_warning(model_with_vars):
    """
    Tests that calling `declare_type` on a Var with no initial type does NOT issue a warning.
    """
    _, v = model_with_vars
    untyped_var = v["untyped_a"]
    
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")  # Ensure all warnings are captured
        untyped_var.declare_type(unit="USD", temporal_type="Flow")
        
        assert len(w) == 0, f"Expected no warnings, but got {len(w)}: {[str(warn.message) for warn in w]}"