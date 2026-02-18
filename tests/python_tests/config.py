"""
Centralized configuration for the test suite.
"""

class TestConfig:
    # Numerical tolerance for floating point comparisons
    TOLERANCE = 1e-6
    
    # Performance Thresholds
    # Target: 133M nodes/sec based on mean of 30 iterations
    PERF_THRESHOLD_NODES_PER_SEC = 135_000_000 
    PERF_ITERATIONS = 30
    
    # Graph Generation constraints for Fuzzing
    FUZZ_MAX_NODES = 100
    FUZZ_MAX_VALUE = 1e6