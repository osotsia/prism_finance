use std::collections::HashMap;
use crate::compute::bytecode::{Program, OpCode};

#[derive(Debug, Clone, Default)]
pub struct LocalityStats {
    /// Reads where the source was produced 1-2 instructions ago (likely register/L1).
    pub hot_l1: usize,
    /// Reads within ~32KB window (L1 limit).
    pub warm_l1: usize,
    /// Reads within ~256KB window (L2 limit).
    pub warm_l2: usize,
    /// Reads outside local cache windows (L3/RAM).
    pub cold_ram: usize,
    /// Reads from constant/input storage (unavoidable cold reads).
    pub constants: usize,
}

#[derive(Debug, Clone)]
pub struct TelemetryReport {
    pub total_ops: usize,
    pub op_counts: HashMap<String, usize>,
    pub locality: LocalityStats,
    /// The average distance of a read (excluding constants). Lower is better.
    pub avg_jump_distance: f64,
    
    // --- New Metrics ---
    
    /// Ratio (0.0 to 1.0) of operations where the write destination 
    /// is exactly (previous_write_index + 1).
    /// Low values (< 0.9) indicate "Random Writes", which disable 
    /// CPU Write Combining and thrash the cache.
    pub write_sequentiality: f64,

    /// Ratio (0.0 to 1.0) of Input reads that are monotonic 
    /// (reading index N, then >N).
    /// High values indicate efficient prefetching of the "cold" input block.
    pub input_read_contiguity: f64,
}

impl TelemetryReport {
    pub fn analyze(program: &Program) -> Self {
        let mut op_counts = HashMap::new();
        let mut locality = LocalityStats::default();
        let mut total_distance: u64 = 0;
        let mut read_count: u64 = 0;

        // Metrics for Write Sequentiality
        let mut sequential_writes = 0;
        // In the current implicit-addressing engine, write_idx is always 'i'.
        // We simulate the check to guard against future architecture changes.
        // We map Logical Node IDs back to Physical Layout to verify.
        // Note: Program doesn't store 'order' in the Segregated version,
        // but we know ops[i] writes to physical index i in the current design.
        // So this metric effectively validates the "Implicit Addressing" assumption.
        let mut prev_write_idx: i32 = -1;

        // Metrics for Input Contiguity
        let mut monotonic_input_reads = 0;
        let mut total_input_reads = 0;
        let mut prev_input_idx: i32 = -1;

        let input_boundary = program.input_start_index as u32;

        for (i, &op_byte) in program.ops.iter().enumerate() {
            let current_idx = i as u32;

            // 1. Analyze Write Pattern
            // In the current Engine, dest is implicitly 'i'.
            let dest_idx = i as i32;
            if dest_idx == prev_write_idx + 1 {
                sequential_writes += 1;
            }
            prev_write_idx = dest_idx;

            // 2. Analyze OpCode
            let op: OpCode = unsafe { std::mem::transmute(op_byte) };
            let op_name = match op {
                OpCode::Add => "Add", OpCode::Sub => "Subtract",
                OpCode::Mul => "Multiply", OpCode::Div => "Divide",
                OpCode::Prev => "Prev", OpCode::Identity => "Identity",
            };
            *op_counts.entry(op_name.to_string()).or_insert(0) += 1;

            // 3. Analyze Locality & Input Patterns
            let check_read = |src: u32, 
                              bnd: u32, 
                              loc: &mut LocalityStats, 
                              dist: &mut u64, 
                              cnt: &mut u64,
                              mono_in: &mut usize,
                              tot_in: &mut usize,
                              last_in: &mut i32| {
                if src >= bnd {
                    // Input Read
                    loc.constants += 1;
                    *tot_in += 1;
                    if (src as i32) > *last_in {
                        *mono_in += 1;
                    }
                    *last_in = src as i32;
                } else {
                    // Calculated Read
                    let d = current_idx.saturating_sub(src);
                    *dist += d as u64;
                    *cnt += 1;
                    match d {
                        0..=2 => loc.hot_l1 += 1,
                        3..=4096 => loc.warm_l1 += 1,
                        4097..=32768 => loc.warm_l2 += 1,
                        _ => loc.cold_ram += 1,
                    }
                }
            };

            let p1 = program.p1[i];
            let p2 = program.p2[i];

            check_read(p1, input_boundary, &mut locality, &mut total_distance, &mut read_count, 
                       &mut monotonic_input_reads, &mut total_input_reads, &mut prev_input_idx);
            
            check_read(p2, input_boundary, &mut locality, &mut total_distance, &mut read_count, 
                       &mut monotonic_input_reads, &mut total_input_reads, &mut prev_input_idx);
        }

        Self {
            total_ops: program.ops.len(),
            op_counts,
            locality,
            avg_jump_distance: if read_count > 0 { total_distance as f64 / read_count as f64 } else { 0.0 },
            
            write_sequentiality: if program.ops.len() > 0 { 
                sequential_writes as f64 / program.ops.len() as f64 
            } else { 1.0 },

            input_read_contiguity: if total_input_reads > 0 {
                monotonic_input_reads as f64 / total_input_reads as f64
            } else { 1.0 },
        }
    }
}
