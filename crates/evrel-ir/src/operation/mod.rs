//! Operations supported by the IR.

mod array;
mod binary;
mod binding;
mod call;
mod class;
mod constant;
mod control;
mod data;
mod debugger;
mod delete;
mod destructure;
mod effects;
mod function;
mod global;
mod jsx;
mod loops;
mod memory_effects;
mod meta;
mod module;
mod object;
mod predicate;
mod property;
mod regexp;
mod suspension;
mod switch;
mod template;
mod try_statement;
mod unary;
mod update;

pub use array::{ArrayLiteralElement, ArrayLiteralOp};
pub use binary::{BinaryOp, BinaryOperator};
pub use binding::{InitializeBindingOp, LoadBindingOp, StoreBindingOp};
pub use call::{CallArgument, CallOp, CallReceiver, CallTarget, ConstructOp, SuperCallOp};
pub use class::{
    ClassElement, ClassElementKey, ClassField, ClassFieldPlacement, ClassMethod, ClassMethodKind,
    ClassMethodPlacement, ClassStaticBlock, CreateClassOp,
};
pub use constant::{ConstantOp, ConstantValue, JsString};
pub use control::{
    BlockTarget, IfOp, JumpOp, OperationSuccessor, RegionYieldOp, ReturnOp, ThrowOp,
};
pub use data::{OperationData, OperationKind};
pub use debugger::DebuggerOp;
pub use delete::{DeleteOp, DeleteTarget};
pub use destructure::{BindingWriteMode, DestructureAssignmentOp, DestructureBindingOp};
pub use effects::OperationEffects;
pub use function::{CreateFunctionOp, LoadArgumentsOp, LoadThisOp};
pub use global::{LoadGlobalOp, StoreGlobalOp};
pub use jsx::{
    JsxAttribute, JsxAttributeName, JsxAttributeValue, JsxChild, JsxElementName, JsxElementOp,
    JsxFragmentOp, JsxMemberBase,
};
pub use loops::{DoWhileOp, ForInOp, ForOfKind, ForOfOp, ForOp, LoopKind, LoopOperation, WhileOp};
pub use memory_effects::MemoryEffects;
pub use meta::{MetaPropertyKind, MetaPropertyOp};
pub use module::{DynamicImportOp, DynamicImportPhase};
pub use object::{ObjectLiteralEntry, ObjectLiteralKey, ObjectLiteralOp, ObjectMethodKind};
pub use predicate::{HasPrivateNameOp, IsNullishOp};
pub use property::{
    LoadPropertyOp, LoadSuperPropertyOp, PropertyKey, StorePropertyOp, StoreSuperPropertyOp,
    SuperPropertyKey,
};
pub use regexp::RegExpLiteralOp;
pub use suspension::{AwaitOp, YieldKind, YieldOp};
pub use switch::{SwitchCase, SwitchOp};
pub use template::{TaggedTemplateOp, TemplateLiteralOp, TemplateQuasi};
pub use try_statement::TryOp;
pub use unary::{TypeofOp, TypeofTarget, UnaryOp, UnaryOperator};
pub use update::{UpdateOp, UpdateOperator};
