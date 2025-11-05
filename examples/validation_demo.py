"""
Demonstrates the static validation engine catching common modeling errors.
"""
import sys
import os
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas

def demonstrate_unit_mismatch():
    print("--- Demonstrating Unit Mismatch Validation ---")
    model = Canvas()
    
    revenue = model.add_var(100.0, name="Revenue", unit="USD")
    volume = model.add_var(50.0, name="Volume", unit="MWh")

    # This operation is logically incorrect
    result = revenue + volume

    try:
        model.validate()
        print("Validation unexpectedly passed.")
    except ValueError as e:
        print(f"Successfully caught expected error:\n  {e}\n")

def demonstrate_temporal_mismatch():
    print("--- Demonstrating Temporal Mismatch Validation ---")
    model = Canvas()

    opening_balance = model.add_var(1000.0, name="Opening Balance", temporal_type="Stock")
    closing_balance = model.add_var(1200.0, name="Closing Balance", temporal_type="Stock")

    # This operation is nonsensical in accounting
    result = opening_balance + closing_balance

    try:
        model.validate()
        print("Validation unexpectedly passed.")
    except ValueError as e:
        print(f"Successfully caught expected error:\n  {e}\n")

def demonstrate_valid_model():
    print("--- Demonstrating a Valid Model ---")
    model = Canvas()

    revenue = model.add_var(100.0, name="Revenue", unit="USD", temporal_type="Flow")
    costs = model.add_var(40.0, name="Costs", unit="USD", temporal_type="Flow")

    profit = revenue + costs
    
    try:
        model.validate()
        print("Successfully validated the model. No errors found.\n")
    except ValueError as e:
        print(f"Caught unexpected error: {e}")

if __name__ == "__main__":
    demonstrate_unit_mismatch()
    demonstrate_temporal_mismatch()
    demonstrate_valid_model()