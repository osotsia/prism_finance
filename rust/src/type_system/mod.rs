//! The static analysis engine for the computation graph.
//!
//! This module provides the `TypeChecker`, which acts as a "Guardian" for the
//! model. It runs a series of checks against the graph's structure and metadata
//! *before* any computation is performed, catching entire classes of common
//! financial modeling errors.

// Publicly export the primary components for use by other modules.
pub use self::checker::TypeChecker;

// --- MODULE DECLARATIONS ---
mod error;
mod checker;
mod rules {
    pub mod temporal;
    pub mod units;
    // Causality is implicitly handled by the DAG's cycle check for now.
    // This file could be added later if functions like `lead()` are introduced.
    // pub mod causality;
}