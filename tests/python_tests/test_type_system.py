import pytest
from prism_finance import Canvas

# --- 1. Test Data Definition ---
# Separating test case data from the test logic.
# Each tuple is: (id, setup_func, validation_should_pass, expected_error_substr)
# The setup_func takes a Canvas and returns the node to be validated.
# This structure makes it easy to add new, complex scenarios.

def setup_happy_path_inference(model):
    revenue = model.add_var(100, name="Revenue", unit="USD", temporal_type="Flow")
    costs = model.add_var(40, name="Costs", unit="USD", temporal_type="Flow")
    return revenue - costs

def setup_happy_path_verification_ok(model):
    revenue = model.add_var(100, name="Revenue", unit="USD", temporal_type="Flow")
    costs = model.add_var(40, name="Costs", unit="USD", temporal_type="Flow")
    profit = revenue - costs
    profit.declare_type(unit="USD", temporal_type="Flow")
    return profit

def setup_unit_mismatch_inference(model):
    revenue = model.add_var(100, name="Revenue", unit="USD")
    volume = model.add_var(50, name="Volume", unit="MWh")
    return revenue + volume

def setup_unit_mismatch_verification_fail(model):
    revenue = model.add_var(100, name="Revenue", unit="USD")
    costs = model.add_var(40, name="Costs", unit="USD")
    profit = revenue - costs
    profit.declare_type(unit="EUR") # Declare wrong unit
    return profit

def setup_temporal_mismatch_verification_fail(model):
    revenue = model.add_var(100, name="Revenue", temporal_type="Flow")
    costs = model.add_var(40, name="Costs", temporal_type="Flow")
    profit = revenue - costs
    profit.declare_type(temporal_type="Stock") # Declare wrong temporal type
    return profit

def setup_stock_plus_stock_is_ok(model):
    """As per new rules, adding two stocks is not an error."""
    ob = model.add_var(1000, name="Opening Balance", temporal_type="Stock", unit="USD")
    cb = model.add_var(1200, name="Closing Balance", temporal_type="Stock", unit="USD")
    return ob + cb

def setup_stock_plus_flow_is_ok(model):
    balance = model.add_var(1000, name="Balance", temporal_type="Stock")
    profit = model.add_var(100, name="Profit", temporal_type="Flow")
    new_balance = balance + profit
    # The inferred type should be 'Stock'. Verify this.
    new_balance.declare_type(temporal_type="Stock")
    return new_balance

def setup_untyped_parents_pass(model):
    a = model.add_var(10, name="A")
    b = model.add_var(5, name="B")
    return a + b

def setup_untyped_parents_verification_fail(model):
    a = model.add_var(10, name="A")
    b = model.add_var(5, name="B")
    c = a + b
    # Cannot verify a type if none can be inferred.
    c.declare_type(unit="USD")
    return c


TEST_CASES = [
    # --- Happy Path Tests ---
    ("happy_path_inference", setup_happy_path_inference, True, None),
    ("happy_path_verification_ok", setup_happy_path_verification_ok, True, None),
    
    # --- "Conceptual Errors" that should now PASS ---
    ("stock_plus_stock_is_ok", setup_stock_plus_stock_is_ok, True, None),
    ("stock_plus_flow_is_ok_and_verifiable", setup_stock_plus_flow_is_ok, True, None),
    
    # --- Inference Failure Tests ---
    ("unit_mismatch_inference", setup_unit_mismatch_inference, False, "Unit Mismatch: Addition/subtraction requires all units to be identical"),

    # --- Verification Failure Tests ---
    ("unit_mismatch_verification_fail", setup_unit_mismatch_verification_fail, False, "Declared unit 'EUR' does not match inferred unit 'USD'"),
    ("temporal_mismatch_verification_fail", setup_temporal_mismatch_verification_fail, False, "Declared temporal type 'Stock' does not match inferred type 'Flow'"),

    # --- Edge Cases ---
    ("untyped_parents_pass_inference", setup_untyped_parents_pass, True, None),
    ("untyped_parents_fail_verification", setup_untyped_parents_verification_fail, False, "Declared unit 'USD' does not match inferred unit 'None'"),
]

# --- 2. Test Execution Logic ---

@pytest.mark.parametrize(
    "id_str, setup_func, should_pass, error_substr",
    TEST_CASES,
    ids=[case[0] for case in TEST_CASES]
)
def test_type_system_validation(id_str, setup_func, should_pass, error_substr):
    """
    Executes a single validation test case defined in TEST_CASES.
    
    Args:
        id_str: Descriptive ID for the test case.
        setup_func: A function that builds the graph on a Canvas.
        should_pass: Boolean indicating if model.validate() should succeed.
        error_substr: A substring expected in the error message if it fails.
    """
    model = Canvas()
    _ = setup_func(model) # The function builds the graph on the model object

    if should_pass:
        try:
            model.validate()
        except ValueError as e:
            pytest.fail(f"Test case '{id_str}' failed validation unexpectedly: {e}")
    else:
        with pytest.raises(ValueError, match=error_substr) as excinfo:
            model.validate()
        # This assert confirms that an error was indeed raised.
        assert excinfo.value is not None

def test_declare_type_overwrite_issues_warning():
    """
    Tests that calling `declare_type` on a Var that already has a type
    issues a `UserWarning`.
    """
    model = Canvas()
    # Create a var with an initial type
    revenue = model.add_var(100, name="Revenue", unit="USD", temporal_type="Flow")

    # Expect two warnings: one for unit, one for temporal_type
    with pytest.warns(UserWarning) as record:
        revenue.declare_type(unit="EUR", temporal_type="Stock")

    assert len(record) == 2
    assert "Overwriting existing unit 'USD' with 'EUR'" in str(record[0].message)
    assert "Overwriting existing temporal_type 'Flow' with 'Stock'" in str(record[1].message)

def test_declare_type_on_untyped_issues_no_warning():
    """
    Tests that calling `declare_type` on a Var with no initial type
    does NOT issue a warning.
    """
    model = Canvas()
    untyped_var = model.add_var(100, name="Untyped")
    
    with pytest.warns(None) as record:
        untyped_var.declare_type(unit="USD")

    # Assert that the code block inside pytest.warns did not raise any warnings.
    # The context manager `record` will be empty.
    assert len(record) == 0