"""
Benchmarks the Prism engine purely inside Rust to isolate core performance from
Python FFI overhead.
"""
import sys
import os
import platform

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import _core

def run_rust_benchmark():
    """
    Executes a 10M node benchmark entirely within the Rust extension.
    """
    try:
        import psutil
        ram_gb = psutil.virtual_memory().total / (1024 ** 3)
    except ImportError:
        ram_gb = 8.0 # assumption

    # User requested "equivalent 10M node benchmark".
    NUM_NODES = 10_000_000
    INPUT_FRACTION = 0.10

    print(f"--- Running Pure Rust Benchmark ---")
    print(f"Platform: {platform.system()} {platform.machine()}")
    print(f"Target:   {NUM_NODES:,} nodes (Pure Rust)")
    
    if ram_gb < 4:
        print("Warning: System RAM < 4GB. Reducing to 1M nodes to prevent OOM.")
        NUM_NODES = 1_000_000

    print("Executing...")
    
    try:
        # Returns: (gen_time, full_compute_time, incr_compute_time, num_nodes)
        gen_time, full_time, incr_time, count = _core.benchmark_pure_rust(NUM_NODES, INPUT_FRACTION)
        
        print("\n--- Results ---")
        print(f"Graph Generation Time:     {gen_time:.4f} s")
        print(f"Full Computation Time:     {full_time:.4f} s")
        print(f"Incremental Recompute:     {incr_time:.4f} s")
        
        full_throughput = count / full_time if full_time > 0 else 0
        print(f"\nThroughput (Full):         {full_throughput:,.0f} nodes/sec")
        
        if incr_time > 0:
            speedup = full_time / incr_time
            print(f"Speedup (Full vs Incr):    {speedup:.1f}x")

        print("\nAnalysis:")
        print(f"  - Full Compute: Measures the linearized VM execution over DenseLedger (SoA).")
        print(f"  - Incremental:  Measures topological invalidation + re-execution of dirty subgraph.")

    except Exception as e:
        print(f"Benchmark failed: {e}")

if __name__ == "__main__":
    run_rust_benchmark()