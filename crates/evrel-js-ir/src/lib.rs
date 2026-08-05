//! JavaScript semantic intermediate representation for Evrel.

mod arena;
mod binding;
mod block;
mod function;
mod ids;
mod module;
mod operation;
mod pattern;
mod printer;
mod private_name;
mod program;
mod region;
mod source;
mod value;

pub use binding::{BindingData, BindingKind};
pub use block::{BasicBlockData, BlockParameter, BlockParameterSource};
pub use function::{
    ExceptionHandlerData, ExceptionHandlerKind, FunctionBuilder, FunctionEditor, FunctionKind,
    FunctionMode, FunctionParameter, FunctionParameterKind, FunctionProperties, JsFunctionIr,
    LabeledStatementData,
};
pub use ids::{
    BindingId, BlockId, ExceptionHandlerId, FunctionId, LabeledStatementId, LocationId, ModuleId,
    OperationId, PrivateNameId, ProgramBindingId, ProgramFunctionId, ProgramOperationId,
    ProgramRegionId, RegionId, SourceFileId, TemplateSiteId, ValueId,
};
pub use module::{
    JsModuleIr, ModuleAttribute, ModuleBuilder, ModuleEditor, ModuleExport, ModuleExportName,
    ModuleImport,
};
pub use operation::{
    ArrayLiteralElement, ArrayLiteralOp, AwaitOp, BinaryOp, BinaryOperator, BindingWriteMode,
    BlockTarget, CallArgument, CallOp, CallReceiver, CallTarget, ClassElement, ClassElementKey,
    ClassField, ClassFieldPlacement, ClassMethod, ClassMethodKind, ClassMethodPlacement,
    ClassStaticBlock, CompletionCase, CompletionKind, ConstantOp, ConstantValue, ConstructOp,
    CreateClassOp, CreateFunctionOp, DebuggerOp, DeleteOp, DeleteTarget, DestructureAssignmentOp,
    DestructureBindingOp, DoWhileOp, DynamicImportOp, DynamicImportPhase, EnterFinallyOp, ForInOp,
    ForOfKind, ForOfOp, ForOp, HasPrivateNameOp, IfOp, InitializeBindingOp, InvokeOp, IsNullishOp,
    JsString, JsxAttribute, JsxAttributeName, JsxAttributeValue, JsxChild, JsxElementName,
    JsxElementOp, JsxFragmentOp, JsxMemberBase, JumpOp, LoadArgumentsOp, LoadBindingOp,
    LoadGlobalOp, LoadPropertyOp, LoadSuperPropertyOp, LoadThisOp, LoopKind, LoopOperation,
    MemoryEffects, MetaPropertyKind, MetaPropertyOp, ObjectLiteralEntry, ObjectLiteralKey,
    ObjectLiteralOp, ObjectMethodKind, OperationData, OperationEffects, OperationKind,
    OperationSuccessor, PropertyKey, RegExpLiteralOp, RegionYieldOp, ResumeCompletionOp, ReturnOp,
    StoreBindingOp, StoreGlobalOp, StorePropertyOp, StoreSuperPropertyOp, SuperCallOp,
    SuperPropertyKey, SwitchCase, SwitchOp, TaggedTemplateOp, TemplateLiteralOp, TemplateQuasi,
    ThrowOp, TryOp, TypeofOp, TypeofTarget, UnaryOp, UnaryOperator, UpdateOp, UpdateOperator,
    WhileOp, YieldKind, YieldOp,
};
pub use pattern::{
    AssignmentPattern, AssignmentTarget, BindingPattern, ObjectAssignmentProperty,
    ObjectBindingProperty, PatternExpression,
};
pub use printer::{print_function, print_module};
pub use private_name::PrivateNameData;
pub use program::{
    JsProgramIr, ModuleDependency, ModuleKey, ModuleRequest, ModuleRequestKind, ModuleTarget,
    ProgramEditor, ProgramModule,
};
pub use region::{RegionData, RegionOwner};
pub use source::{CompilerLocation, SourceDatabase, SourceFile, SyntheticReason, TextRange};
pub use value::{ValueData, ValueDefinition, ValueType, ValueUse};
