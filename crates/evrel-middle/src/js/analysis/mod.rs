//! Read-only computations over Evrel IR.

mod binding_promotion;
mod control_flow;
mod direct_eval;
mod dominance_frontier;
mod dominator_tree;
mod function_capture;
mod function_pointer;
mod function_value;
mod function_value_dependence;
mod module_function_reachability;
mod operation_safety;
mod program_call_graph;
mod program_linkage;
mod program_reachability;
mod region_capture;

pub use binding_promotion::{FunctionBindingPromotion, ModuleBindingPromotion, PromotableBinding};
pub use control_flow::RegionControlFlowGraph;
pub use dominance_frontier::RegionDominanceFrontier;
pub use dominator_tree::RegionDominatorTree;
pub use function_capture::{BindingCapture, CaptureAccess, FunctionCaptureAnalysis};
pub use function_pointer::{
    AbstractObject, AbstractObjectId, AbstractObjectKind, AliasResult, EscapeResult,
    FunctionPointerAnalysis, PointsToSet,
};
pub use function_value::{AbstractValue, FunctionValueAnalysis, FunctionValueInputs, ValueTypeSet};
pub use function_value_dependence::FunctionValueDependenceAnalysis;
pub use module_function_reachability::ModuleFunctionReachability;
pub use operation_safety::is_safe_to_remove;
pub use program_call_graph::{
    CallSite, CallSiteId, CallSiteKind, CallTargetCompleteness, CallTargetSet, FunctionReference,
    FunctionReferenceSite, ProgramCallGraph,
};
pub use program_linkage::{ImportedBindingTarget, ProgramLinkage};
pub use program_reachability::ProgramReachability;
pub use region_capture::RegionCaptureAnalysis;
