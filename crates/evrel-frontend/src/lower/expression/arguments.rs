//! Shared JavaScript argument-list lowering.

use evrel_js_ir::CallArgument;
use oxc_ast::ast::Argument;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers invocation arguments into source-ordered expression regions.
pub(super) fn lower_arguments(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    arguments: &[Argument<'_>],
) -> Result<Box<[CallArgument]>, FrontendError> {
    let mut lowered = Vec::with_capacity(arguments.len());

    for argument in arguments {
        let argument = match argument {
            Argument::SpreadElement(spread) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &spread.argument)
                })?;

                CallArgument::Spread { expression }
            }

            argument => {
                let expression = argument
                    .as_expression()
                    .ok_or(FrontendError::UnsupportedExpression)?;
                let expression = lowerer
                    .build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

                CallArgument::Value { expression }
            }
        };

        lowered.push(argument);
    }

    Ok(lowered.into_boxed_slice())
}
