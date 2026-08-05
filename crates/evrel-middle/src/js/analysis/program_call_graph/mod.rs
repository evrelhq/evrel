//! Conservative program-wide JavaScript call and function-reference topology.
//!
//! The graph models target identity and completeness, not callee behavior or
//! profitability. Calls with unresolved runtime targets remain explicit
//! incomplete sites, so an empty target list never means "calls nothing".

mod graph;
mod targets;

#[cfg(test)]
mod tests;

pub use graph::{
    CallSite, CallSiteId, CallSiteKind, CallTargetCompleteness, CallTargetSet, FunctionReference,
    FunctionReferenceSite, ProgramCallGraph,
};
