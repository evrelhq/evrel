//! Function IR storage and construction.

mod builder;
mod editor;
mod exception_handler;
mod function;
mod kind;
mod labeled_statement;
mod mode;
mod parameter;
mod properties;
mod unwind;

pub use builder::FunctionBuilder;
pub use editor::FunctionEditor;
pub use exception_handler::{ExceptionHandlerData, ExceptionHandlerKind};
pub use function::FunctionIr;
pub use kind::FunctionKind;
pub use labeled_statement::LabeledStatementData;
pub use mode::FunctionMode;
pub use parameter::{FunctionParameter, FunctionParameterKind};
pub use properties::FunctionProperties;
pub use unwind::UnwindTarget;
