//! Defines the `Edge` type, representing a dependency between two nodes.

/// Describes the semantic type of a dependency in the graph.
///
/// This information is used by the validation engine to understand the model's
/// logic, for instance, to distinguish a standard calculation from a temporal shift.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Edge {
    /// A standard arithmetic dependency.
    /// Example: `C = A + B`. `A` and `B` are arithmetic parents of `C`.
    Arithmetic,
    /// A dependency on a previous time-step of another node.
    /// Example: `Debt[t] = Debt[t-1] + ...`. The link from `Debt` to itself is a temporal edge.
    Temporal,
    /// A dependency used to supply a default value for an operation.
    /// Example: `Revenue.prev(default=InitialRevenue)`. The link from `InitialRevenue` to the
    /// `Revenue` formula node is a `DefaultValue` edge.
    DefaultValue,
}