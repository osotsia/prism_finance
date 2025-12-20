#!/bin/bash

# Configuration
ITERATIONS=40
BENCH_SCRIPT="examples/benchmark.py"
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
    RESULT=$(python "$BENCH_SCRIPT" 2>/dev/null | grep "Throughput (Full):" | awk '{print $3}' | tr -d ',')
    
    if [[ -n "$RESULT" ]]; then
        echo "$RESULT" >> "$TEMP_FILE"
        # Display progress with thousands separator
        printf "Iteration %02d: %'d nodes/sec\n" "$i" "$RESULT"
    else
        echo "Iteration $i: Failed to extract throughput."
    fi
done

# Calculate statistics
if [[ -s "$TEMP_FILE" ]]; then
    # Use printf within awk to prevent scientific notation (e.g., 1.25e+08)
    MEAN=$(awk '{ sum += $1; count++ } END { if (count > 0) printf "%.0f", sum / count }' "$TEMP_FILE")
    COUNT=$(wc -l < "$TEMP_FILE")
    
    echo "--------------------------------------------------"
    echo "Benchmark Summary"
    echo "--------------------------------------------------"
    echo "Successful runs: $COUNT / $ITERATIONS"
    printf "Mean Throughput: %'d nodes/sec\n" "$MEAN"
else
    echo "No valid results collected."
fi

# Cleanup
rm "$TEMP_FILE"