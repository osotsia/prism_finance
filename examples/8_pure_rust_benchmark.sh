#!/bin/bash

# Configuration
ITERATIONS=150
BENCH_SCRIPT="examples/single_benchmark.py"
TEMP_FILE="bench_results.tmp"

# Ensure clean start
> "$TEMP_FILE"

# Validation
if [[ ! -f "$BENCH_SCRIPT" ]]; then
    echo "Error: $BENCH_SCRIPT not found in current directory."
    exit 1
fi

echo "Starting $ITERATIONS iterations of $BENCH_SCRIPT..."

# Set locale to ensure thousands separator works in printf
export LC_NUMERIC=en_US.UTF-8

for ((i=1; i<=ITERATIONS; i++)); do
    # Extract numeric throughput value and strip commas
    # Example output line: "Throughput (Full):         145,200 nodes/sec"
    RESULT=$(python "$BENCH_SCRIPT" 2>/dev/null | grep "Throughput (Full):" | awk '{print $3}' | tr -d ',')
    
    if [[ -n "$RESULT" ]]; then
        echo "$RESULT" >> "$TEMP_FILE"
        # Display progress with thousands separator
        printf "Iteration %02d: %'d nodes/sec\n" "$i" "$RESULT"
    else
        echo "Iteration $i: Failed to extract throughput."
    fi
done

# Calculate statistics using Python for floating point precision
if [[ -s "$TEMP_FILE" ]]; then
    echo "--------------------------------------------------"
    echo "Benchmark Summary (N=$ITERATIONS)"
    echo "--------------------------------------------------"
    
    python3 -c "
import math

# Load data
with open('$TEMP_FILE', 'r') as f:
    data = [float(line.strip()) for line in f if line.strip()]

n = len(data)
if n > 1:
    mean = sum(data) / n
    
    # Calculate Sample Standard Deviation
    variance = sum((x - mean) ** 2 for x in data) / (n - 1)
    std_dev = math.sqrt(variance)
    
    # Calculate Standard Error (SE)
    std_error = std_dev / math.sqrt(n)
    
    # Calculate 95% Confidence Interval (Z=1.96)
    margin_of_error = 1.96 * std_error
    
    # Scale to Millions
    mean_m = mean / 1_000_000.0
    moe_m = margin_of_error / 1_000_000.0
    
    print(f'Mean Throughput: {mean_m:.1f} ± {moe_m:.1f} million nodes/sec (95% CI)')
    # print(f'Standard Error:  {std_error:,.0f} nodes/sec')
    # print(f'Min / Max:       {min(data):,.0f} / {max(data):,.0f}')
else:
    print('Insufficient data for statistics.')
"
else
    echo "No valid results collected."
fi

# Cleanup
rm "$TEMP_FILE"