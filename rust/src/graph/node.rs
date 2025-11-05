//! Defines the `Node` and its associated types, representing a single
//! variable or calculation step in the financial model.

use petgraph::graph::NodeIndex;

/// A unique, stable identifier for a node within the graph.
///
/// This is a type alias for `petgraph::graph::NodeIndex` to abstract the
/// underlying graph implementation.
pub type NodeId = NodeIndex;

/// Represents the temporal nature of a financial variable.
///
/// This is a core component of the static analysis type system, used to
/// prevent logical errors like adding a stock (a point-in-time value) to
/// another stock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TemporalType {
    /// A value measured at a specific point in time (e.g., a balance sheet item like 'Debt Balance').
    Stock,
    /// A value measured over a period of time (e.g., an income statement item like 'Revenue').
    Flow,
}

/// Represents the physical or monetary unit of a variable.
///
/// Used by the static analysis engine to perform dimensional analysis and
/// prevent errors like multiplying 'USD/kW' by 'USD/MWh'.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unit(pub String);

/// Contains metadata for a node, used for auditing, display, and static analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeMetadata {
    /// A human-readable name for the variable (e.g., "Total Project Cost").
    pub name: String,
    /// The temporal classification of the variable.
    pub temporal_type: Option<TemporalType>,
    /// The unit of measurement for the variable.
    pub unit: Option<Unit>,
}

/// Defines the specific calculation performed by a `Formula` node.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    /// Represents a time-series lag operation (e.g., `.prev()`).
    ///
    /// The `default_node` provides the value for initial periods where
    /// a lagged value is not yet available.
    PreviousValue { lag: u32, default_node: NodeId },
}

/// The primary enum representing a node in the computation graph.
///
/// A node is the "skeleton" of the model. It defines the logic and relationships,
/// but does not hold the computed values themselves (which are managed by the `computation::Ledger`).
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// An input variable with a fixed time-series value.
    Constant {
        value: Vec<f64>,
        meta: NodeMetadata,
    },
    /// A calculated variable, derived from one or more parent nodes.
    Formula {
        op: Operation,
        // The list of parent nodes this formula depends on. The order is significant
        // for non-commutative operations like subtraction and division.
        parents: Vec<NodeId>,
        meta: NodeMetadata,
    },
    /// A placeholder for a variable whose value is determined by the solver.
    SolverVariable { meta: NodeMetadata },
}