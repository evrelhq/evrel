//! JavaScript expression lowering.

mod arguments;
mod array;
mod assignment;
mod await_expression;
mod binary;
mod call;
mod conditional;
mod function;
mod identifier;
mod import;
mod jsx;
mod literal;
mod logical;
mod member;
mod meta_property;
mod new;
mod object;
mod optional_chain;
mod sequence;
mod template;
mod this;
mod unary;
mod update;
mod yield_expression;

use evrel_js_ir::ValueId;
use oxc_ast::ast::Expression;
use oxc_span::GetSpan;

use crate::{FrontendError, lower::FunctionLowerer};

pub(super) use assignment::lower_assignment_target_write;

/// Lowers an expression and returns its produced SSA value.
pub(super) fn lower_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &Expression<'_>,
) -> Result<ValueId, FrontendError> {
    let span = expression.span();

    lowerer.with_span(span, |lowerer| {
        lower_expression_at_current_location(lowerer, expression)
    })
}

fn lower_expression_at_current_location(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &Expression<'_>,
) -> Result<ValueId, FrontendError> {
    match expression {
        Expression::ArrayExpression(expression) => {
            array::lower_array_expression(lowerer, expression)
        }

        Expression::Identifier(identifier) => identifier::lower_identifier(lowerer, identifier),

        Expression::StaticMemberExpression(member) => {
            member::lower_static_member_expression(lowerer, member)
        }

        Expression::ComputedMemberExpression(member) => {
            member::lower_computed_member_expression(lowerer, member)
        }

        Expression::PrivateFieldExpression(member) => {
            member::lower_private_member_expression(lowerer, member)
        }

        Expression::BooleanLiteral(literal) => Ok(literal::lower_boolean_literal(lowerer, literal)),

        Expression::BigIntLiteral(literal) => Ok(literal::lower_bigint_literal(lowerer, literal)),

        Expression::NullLiteral(literal) => Ok(literal::lower_null_literal(lowerer, literal)),

        Expression::NumericLiteral(literal) => Ok(literal::lower_numeric_literal(lowerer, literal)),

        Expression::RegExpLiteral(literal) => Ok(literal::lower_regexp_literal(lowerer, literal)),

        Expression::StringLiteral(literal) => Ok(literal::lower_string_literal(lowerer, literal)),

        Expression::TemplateLiteral(literal) => template::lower_template_literal(lowerer, literal),

        Expression::TaggedTemplateExpression(expression) => {
            template::lower_tagged_template_expression(lowerer, expression)
        }

        Expression::ObjectExpression(expression) => {
            object::lower_object_expression(lowerer, expression)
        }

        Expression::AssignmentExpression(assignment) => {
            assignment::lower_assignment_expression(lowerer, assignment)
        }

        Expression::AwaitExpression(expression) => {
            await_expression::lower_await_expression(lowerer, expression)
        }

        Expression::BinaryExpression(binary) => binary::lower_binary_expression(lowerer, binary),

        Expression::PrivateInExpression(expression) => {
            binary::lower_private_in_expression(lowerer, expression)
        }

        Expression::CallExpression(call) => call::lower_call_expression(lowerer, call),

        Expression::ClassExpression(class) => super::class::lower_class_value(lowerer, class),

        Expression::ConditionalExpression(expression) => {
            conditional::lower_conditional_expression(lowerer, expression)
        }

        Expression::LogicalExpression(expression) => {
            logical::lower_logical_expression(lowerer, expression)
        }

        Expression::MetaProperty(property) => {
            Ok(meta_property::lower_meta_property(lowerer, property))
        }

        Expression::NewExpression(expression) => new::lower_new_expression(lowerer, expression),

        Expression::ChainExpression(chain) => optional_chain::lower_optional_chain(lowerer, chain),

        Expression::ArrowFunctionExpression(function) => {
            function::lower_arrow_function_expression(lowerer, function)
        }

        Expression::FunctionExpression(function) => {
            function::lower_function_expression(lowerer, function)
        }

        Expression::ImportExpression(expression) => {
            import::lower_import_expression(lowerer, expression)
        }

        Expression::JSXElement(element) => jsx::lower_jsx_element(lowerer, element),

        Expression::JSXFragment(fragment) => jsx::lower_jsx_fragment(lowerer, fragment),

        Expression::ThisExpression(expression) => {
            Ok(this::lower_this_expression(lowerer, expression))
        }

        Expression::UnaryExpression(expression) => {
            unary::lower_unary_expression(lowerer, expression)
        }

        Expression::UpdateExpression(expression) => {
            update::lower_update_expression(lowerer, expression)
        }

        Expression::YieldExpression(expression) => {
            yield_expression::lower_yield_expression(lowerer, expression)
        }

        Expression::SequenceExpression(sequence) => {
            sequence::lower_sequence_expression(lowerer, sequence)
        }

        Expression::ParenthesizedExpression(expression) => {
            lower_expression(lowerer, &expression.expression)
        }

        Expression::TSAsExpression(expression) => lower_expression(lowerer, &expression.expression),

        Expression::TSSatisfiesExpression(expression) => {
            lower_expression(lowerer, &expression.expression)
        }

        Expression::TSTypeAssertion(expression) => {
            lower_expression(lowerer, &expression.expression)
        }

        Expression::TSNonNullExpression(expression) => {
            lower_expression(lowerer, &expression.expression)
        }

        Expression::TSInstantiationExpression(expression) => {
            lower_expression(lowerer, &expression.expression)
        }

        _ => Err(FrontendError::UnsupportedExpression),
    }
}
