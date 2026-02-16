use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct NodeId(pub u32);

impl NodeId {
    #[inline(always)]
    pub fn index(&self) -> usize { self.0 as usize }
    pub fn new(idx: usize) -> Self { Self(idx as u32) }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemporalType {
    Stock,
    Flow,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unit(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub name: String,
    pub temporal_type: Option<TemporalType>,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    PreviousValue { lag: u32, default_node: NodeId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Scalar(f64),
    TimeSeries(u32), // Index into constants_data
    Formula(Operation),
    SolverVariable,
}