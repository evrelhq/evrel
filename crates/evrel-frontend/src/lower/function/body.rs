//! JavaScript function-body lowering.

use evrel_js_ir::FunctionProperties;
use evrel_js_ir::{ConstantOp, ConstantValue, OperationKind, ReturnOp};
use oxc_ast::ast::{FunctionBody, Statement};

use crate::{FrontendError, lower::FunctionLowerer};

use super::super::statement::lower_statement_list;

/// Lowers a block-bodied JavaScript function.
pub(crate) fn lower_function_body(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    body: &FunctionBody<'_>,
) -> Result<(), FrontendError> {
    lower_function_statements(lowerer, &body.statements)
}

pub(crate) fn lower_function_properties(body: &FunctionBody<'_>) -> FunctionProperties {
    if body
        .directives
        .iter()
        .any(|directive| directive.directive == "use strict")
    {
        FunctionProperties::default().with_use_strict_directive()
    } else {
        FunctionProperties::default()
    }
}

/// Lowers a function-like statement body and adds normal completion.
pub(crate) fn lower_function_statements(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statements: &[Statement<'_>],
) -> Result<(), FrontendError> {
    lower_statement_list(lowerer, statements)?;
    ensure_implicit_return(lowerer);

    Ok(())
}

/// Completes the current function with an implicit `undefined` return.
fn ensure_implicit_return(lowerer: &mut FunctionLowerer<'_, '_, '_>) {
    if lowerer.current_block_is_terminated() {
        return;
    }

    let undefined = lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
        [],
    );

    lowerer.terminate(OperationKind::Return(ReturnOp::new()), [undefined]);
}
