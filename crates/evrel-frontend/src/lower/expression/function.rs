//! JavaScript function-expression lowering.

use evrel_ir::{CreateFunctionOp, FunctionKind, FunctionMode, OperationKind, ReturnOp, ValueId};
use oxc_ast::ast::{ArrowFunctionExpression, Function};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        declaration::{declare_scope_bindings, instantiate_function_scope},
        lower_function_body, lower_function_parameters, lower_function_properties,
        lower_ordinary_function_definition,
    },
};

use super::lower_expression;

/// Lowers an arrow function.
pub(super) fn lower_arrow_function_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ArrowFunctionExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let mode = if expression.r#async {
        FunctionMode::Async
    } else {
        FunctionMode::Normal
    };
    let (function, result) = lowerer.build_nested_function_with_properties(
        FunctionKind::Arrow,
        mode,
        lower_function_properties(&expression.body),
        |nested| {
            let scope = expression
                .scope_id
                .get()
                .expect("semantic analysis must assign the function scope");

            lower_function_parameters(nested, &expression.params)?;
            declare_scope_bindings(nested, scope)?;
            instantiate_function_scope(nested, scope, &expression.body.statements)?;
            lower_arrow_function_body(nested, expression)
        },
    );

    result?;

    Ok(lowerer.emit_value(
        OperationKind::CreateFunction(CreateFunctionOp::new(function)),
        [],
    ))
}

/// Lowers an ordinary function expression.
pub(super) fn lower_function_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &Function<'_>,
) -> Result<ValueId, FrontendError> {
    let function = lower_ordinary_function_definition(lowerer, expression)?;

    Ok(lowerer.emit_value(
        OperationKind::CreateFunction(CreateFunctionOp::new(function)),
        [],
    ))
}

fn lower_arrow_function_body(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ArrowFunctionExpression<'_>,
) -> Result<(), FrontendError> {
    if expression.expression {
        let body = expression
            .get_expression()
            .expect("expression-bodied arrow must contain an expression");
        let value = lower_expression(lowerer, body)?;

        lowerer.terminate(OperationKind::Return(ReturnOp::new()), [value]);

        return Ok(());
    }

    lower_function_body(lowerer, &expression.body)
}
