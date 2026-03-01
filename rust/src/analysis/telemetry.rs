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
}

impl TelemetryReport {
    pub fn analyze(program: &Program) -> Self {
        let mut op_counts = HashMap::new();
        let mut locality = LocalityStats::default();
        let mut total_distance: u64 = 0;
        let mut read_count: u64 = 0;

        let input_boundary = program.input_start_index as u32;

        for (i, &op_byte) in program.ops.iter().enumerate() {
            let current_idx = i as u32;

            // 1. Analyze Operation Distribution
            // Safe transmute because byte came from internal OpCode enum
            let op: OpCode = unsafe { std::mem::transmute(op_byte) };
            let op_name = match op {
                OpCode::Add => "Add",
                OpCode::Sub => "Subtract",
                OpCode::Mul => "Multiply",
                OpCode::Div => "Divide",
                OpCode::Prev => "Prev",
                OpCode::Identity => "Identity",
            };
            *op_counts.entry(op_name.to_string()).or_insert(0) += 1;

            // 2. Analyze Memory Locality
            // We analyze the distance between the current write head (i) 
            // and the read heads (p1, p2).
            let p1 = program.p1[i];
            let p2 = program.p2[i];

            Self::record_jump(current_idx, p1, input_boundary, &mut locality, &mut total_distance, &mut read_count);
            Self::record_jump(current_idx, p2, input_boundary, &mut locality, &mut total_distance, &mut read_count);
        }

        Self {
            total_ops: program.ops.len(),
            op_counts,
            locality,
            avg_jump_distance: if read_count > 0 { total_distance as f64 / read_count as f64 } else { 0.0 },
        }
    }

    #[inline]
    fn record_jump(
        current: u32, 
        source: u32, 
        boundary: u32, 
        stats: &mut LocalityStats, 
        total_dist: &mut u64, 
        count: &mut u64
    ) {
        if source >= boundary {
            // Source is an Input/Constant (stored at the end of the ledger).
            // These are structurally "cold" in this architecture.
            stats.constants += 1;
        } else {
            // Source is a calculated intermediate value.
            // In a topologically sorted linear program, source < current.
            let dist = current.saturating_sub(source);
            
            *total_dist += dist as u64;
            *count += 1;

            // Bins based on f64 size (8 bytes).
            // L1 ~= 32KB / 8 = 4096 slots.
            // L2 ~= 256KB / 8 = 32768 slots.
            match dist {
                0..=2 => stats.hot_l1 += 1,       // Immediate consumption
                3..=4096 => stats.warm_l1 += 1,   // Fits in L1
                4097..=32768 => stats.warm_l2 += 1, // Fits in L2
                _ => stats.cold_ram += 1,         // Main Memory fetch
            }
        }
    }
}
