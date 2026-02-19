"""
Performance Regression Suite.

To run: pytest tests/python_tests/test_performance.py --run-perf
"""
import pytest
import math
import statistics
import warnings
from prism_finance import _core
from .config import TestConfig

@pytest.mark.benchmark
def test_engine_throughput():
    """
    Measures engine throughput.
    
    Behavior:
    - ALWAYS PASSES (does not fail the build).
    - WARNS if throughput is below threshold.
    """
    
    iterations = TestConfig.PERF_ITERATIONS
    threshold = TestConfig.PERF_THRESHOLD_NODES_PER_SEC
    
    # Benchmark Parameters (10M nodes, 10% inputs)
    num_nodes = 10_000_000
    input_fraction = 0.10
    
    throughputs = []
    
    print(f"\nRunning {iterations} iterations of pure Rust benchmark...")
    
    for _ in range(iterations):
        # returns (gen_time, full_compute_time, incr_compute_time, num_nodes)
        _, full_time, _, count = _core.benchmark_pure_rust(num_nodes, input_fraction)
        
        if full_time > 0:
            throughputs.append(count / full_time)
    
    if not throughputs:
        warnings.warn("Benchmark failed to produce valid timing data.", UserWarning)
        return

    mean_throughput = statistics.mean(throughputs)
    margin_of_error = 1.96 * statistics.stdev(throughputs) / math.sqrt(iterations)
    
    # Formatting for display
    mean_fmt = mean_throughput / 1e6
    thresh_fmt = threshold / 1e6
    moe_fmt = margin_of_error / 1e6
    
    # Report results to stdout (visible with pytest -s)
    print(f"\n--- Benchmark Results (N={iterations}) ---")
    print(f"Throughput: {mean_fmt:.1f} ± {moe_fmt:.1f} M nodes/sec")
    print(f"Threshold:       {thresh_fmt:.1f} M nodes/sec")
    
    # Always warn
    warnings.warn(
        f"\nPERFORMANCE RESULTS (n={iterations}):\n"
        f"Current: {mean_fmt:.1f} ± {moe_fmt:.1f} M nodes/sec\n"
        f"Target:  {thresh_fmt:.1f} M nodes/sec",
        UserWarning
    )