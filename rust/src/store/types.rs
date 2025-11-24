use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NodeId(pub u32);

impl NodeId {
    #[inline(always)]
    pub fn index(&self) -> usize { self.0 as usize }
    pub fn new(idx: usize) -> Self { Self(idx as u32) }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TemporalType {
    Stock,
    Flow,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Unit(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeMetadata {
    pub name: String,
    pub temporal_type: Option<TemporalType>,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    PreviousValue { lag: u32, default_node: NodeId },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Scalar(f64),
    TimeSeries(u32), // Index into constants_data
    Formula(Operation),
    SolverVariable,
}