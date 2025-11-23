"""
Demonstrates and tests the core computation engine, including:
1. Correctness of incremental recomputation.
2. Performance benchmarking on a large, randomly generated graph that scales
   to the host system's memory.
"""
import sys
import os
import random
import time
import platform

# Add the project root to the Python path for local execution.
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from prism_finance import Canvas, Var

try:
    import psutil
    PSUTIL_AVAILABLE = True
except ImportError:
    PSUTIL_AVAILABLE = False


def demonstrate_incremental_recomputation():
    """Verifies that incremental recomputation produces correct results."""
    print("--- 1. Demonstrating Incremental Recomputation Correctness ---")
    with Canvas() as model:
        # --- Setup a simple graph ---
        # D depends on C, and C depends on A. B is independent.
        # D = A * C = A * (A + B)
        a = Var(10.0, name="A")
        b = Var(20.0, name="B")
        c = a + b
        d = a * c

        # --- Initial full computation ---
        model.compute_all()
        val_c1 = model.get_value(c)
        val_d1 = model.get_value(d)
        print(f"Initial state: A=10, B=20")
        print(f"  - C (A+B) = {val_c1:.1f}")
        print(f"  - D (A*C) = {val_d1:.1f}")
        assert abs(val_c1 - 30.0) < 1e-9
        assert abs(val_d1 - 300.0) < 1e-9

        # --- Update one input and recompute incrementally ---
        print("\nUpdating A to 5.0 and recomputing...")
        a.set(5.0)
        model.recompute(changed_vars=[a])

        # --- Verify the new state ---
        val_c2 = model.get_value(c)
        val_d2 = model.get_value(d)
        # B's value should persist in the ledger as it was not invalidated
        val_b = model.get_value(b)
        print(f"New state: A=5, B=20")
        print(f"  - C (A+B) = {val_c2:.1f}")
        print(f"  - D (A*C) = {val_d2:.1f}")
        assert abs(val_b - 20.0) < 1e-9, "Unchanged value B should be preserved."
        assert abs(val_c2 - 25.0) < 1e-9, "C should be 5 + 20 = 25."
        assert abs(val_d2 - 125.0) < 1e-9, "D should be 5 * 25 = 125."

    print("\nIncremental recomputation test passed.\n")


def get_system_config():
    """Infers system capabilities and determines an appropriate graph size."""
    if not PSUTIL_AVAILABLE:
        print("Warning: `psutil` not found. Using default benchmark size.")
        print("         Run `pip install psutil` for adaptive benchmarking.\n")
        return {
            "ram_gb": "N/A",
            "cpu_cores": "N/A",
            "num_nodes": 250_000,
        }

    ram_gb = psutil.virtual_memory().total / (1024 ** 3)
    cpu_cores = os.cpu_count()

    # Scale the number of nodes based on available RAM to ensure the benchmark
    # is substantial but does not exhaust system memory.
    if ram_gb < 8:
        num_nodes = 200_000  # Low-spec systems
    elif ram_gb < 24:
        num_nodes = 10_000_000  # Mid-range systems (e.g., 8-16GB RAM)
    else:
        num_nodes = 2_000_000 # High-spec systems (e.g., 32GB+ RAM)
        
    return {
        "ram_gb": f"{ram_gb:.1f}",
        "cpu_cores": cpu_cores,
        "num_nodes": num_nodes,
    }


def generate_large_graph(model: Canvas, num_nodes: int, input_fraction: float, connectivity: int):
    """Generates a large random DAG within the given canvas."""
    nodes = []
    inputs = []
    
    num_inputs = int(num_nodes * input_fraction)

    # Create input nodes
    for i in range(num_inputs):
        node = Var(random.random() * 100, name=f"Input_{i}")
        nodes.append(node)
        inputs.append(node)
    
    # Create formula nodes
    ops = ['add', 'subtract', 'multiply']
    for i in range(num_inputs, num_nodes):
        # Ensure parents have a lower index to guarantee a DAG
        parent_indices = random.sample(range(len(nodes)), k=min(len(nodes), connectivity))
        parents = [nodes[j] for j in parent_indices]

        op = random.choice(ops)
        if op == 'add':
            new_node = parents[0] + parents[1]
        elif op == 'subtract':
            new_node = parents[0] - parents[1]
        else: # multiply
            new_node = parents[0] * parents[1]
        
        # Manually set internal name for clarity if needed, though default is fine
        new_node._name = f"Formula_{i}"
        nodes.append(new_node)
        
    return inputs, nodes


def benchmark_engine_performance():
    """Measures full and incremental compute times on a large graph."""
    print("--- 2. Benchmarking Engine Performance ---")
    
    # --- Infer system and configure benchmark ---
    config = get_system_config()
    NUM_NODES = config["num_nodes"]
    INPUT_FRACTION = 0.1
    CONNECTIVITY = 2
    NUM_CHANGED_INPUTS = 5

    print("System Configuration:")
    print(f"  - Platform:  {platform.system()} {platform.machine()}")
    print(f"  - RAM:       {config['ram_gb']} GB")
    print(f"  - CPU Cores: {config['cpu_cores']}")
    print(f"  - Benchmark will use {NUM_NODES:,} nodes.\n")

    model = Canvas()
    with model:
        print("Generating random graph...")
        start_gen = time.perf_counter()
        inputs, _ = generate_large_graph(model, NUM_NODES, INPUT_FRACTION, CONNECTIVITY)
        end_gen = time.perf_counter()
        print(f"Graph generation took: {end_gen - start_gen:.3f} seconds.")
    
        # --- Benchmark Full Computation ---
        print("\nBenchmarking full computation...")
        start_full = time.perf_counter()
        model.compute_all()
        end_full = time.perf_counter()
        full_duration = end_full - start_full
        nodes_per_sec_full = NUM_NODES / full_duration if full_duration > 0 else 0
        print(f"  - Total time: {full_duration:.3f} seconds")
        print(f"  - Throughput: {nodes_per_sec_full:,.0f} nodes/sec")
    
        # --- Benchmark Incremental Recomputation ---
        print(f"\nBenchmarking incremental recomputation after changing {NUM_CHANGED_INPUTS} inputs...")
        # Select some early inputs to change, ensuring a non-trivial dependency chain
        vars_to_change = inputs[:NUM_CHANGED_INPUTS]
        for var in vars_to_change:
            var.set(random.random() * 100)
    
        start_inc = time.perf_counter()
        model.recompute(changed_vars=vars_to_change)
        end_inc = time.perf_counter()
        inc_duration = end_inc - start_inc
        
        print(f"  - Total time: {inc_duration:.3f} seconds")
        
        # --- Analysis and Context ---
        if inc_duration > 0 and full_duration > 0:
            speedup = full_duration / inc_duration
            print(f"  - Speedup vs. full recompute: {speedup:.1f}x")

        print("\nBenchmark context:")
        print("  - The benchmark measures the overhead of the Python-to-Rust FFI, graph traversal, and memory access, not just raw floating-point math.")
        print("  - The 'Speedup' metric demonstrates the primary value of incremental recomputation: avoiding redundant work for small changes in large models.")
        print("  - A lower incremental time indicates that the engine correctly identified and re-evaluated only the subset of the graph affected by the change.")


if __name__ == "__main__":
    demonstrate_incremental_recomputation()
    benchmark_engine_performance()