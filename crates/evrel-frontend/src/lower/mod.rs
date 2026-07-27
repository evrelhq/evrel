//! Oxc AST to Evrel IR lowering.

mod class;
mod context;
mod declaration;
mod expression;
mod function;
mod module;
mod pattern;
mod statement;

pub(crate) use context::LoweringContext;
pub(crate) use function::{
    FunctionLowerer, lower_class_element_function, lower_function_body, lower_function_parameters,
    lower_function_properties, lower_function_statements, lower_object_method_function,
    lower_ordinary_function_definition,
};
pub(crate) use module::lower_module;
