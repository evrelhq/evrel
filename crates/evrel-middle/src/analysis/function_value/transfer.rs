//! Transfer semantics for JavaScript IR operations.

use evrel_ir::{
    BinaryOperator, ConstantValue, JsString, OperationKind, TypeofTarget, UnaryOperator,
};

use super::AbstractValue;

/// Computes the abstract value of one operation result.
///
/// Facts describe the result when the operation completes normally. Whether
/// evaluating the operation can throw or produce observable effects remains
/// represented independently by the IR operation's effect metadata.
pub(super) fn evaluate_result(
    kind: &OperationKind,
    operands: &[AbstractValue],
    _result_index: usize,
) -> AbstractValue {
    if operands.iter().any(AbstractValue::is_bottom) {
        return AbstractValue::bottom();
    }

    match kind {
        OperationKind::Constant(operation) => {
            AbstractValue::from_constant(operation.value().clone())
        }

        OperationKind::IsNullish(_) => evaluate_unary(operands, |operand| {
            Some(ConstantValue::Boolean(matches!(
                operand,
                ConstantValue::Undefined | ConstantValue::Null
            )))
        }),

        OperationKind::Unary(operation) => evaluate_unary(operands, |operand| {
            evaluate_unary_operator(operation.operator(), operand)
        }),

        OperationKind::Binary(operation) => evaluate_binary(operands, |left, right| {
            evaluate_binary_operator(operation.operator(), left, right)
        }),

        OperationKind::Typeof(operation) if matches!(operation.target(), TypeofTarget::Value) => {
            evaluate_unary(operands, |operand| {
                Some(ConstantValue::String(JsString::new(
                    constant_typeof(operand),
                    false,
                )))
            })
        }

        _ => AbstractValue::unknown(),
    }
}

/// Returns the truthiness of an exact JavaScript primitive constant.
pub(super) fn constant_truthiness(value: &ConstantValue) -> bool {
    match value {
        ConstantValue::Undefined | ConstantValue::Null => false,
        ConstantValue::Boolean(value) => *value,
        ConstantValue::Number(value) => *value != 0.0 && !value.is_nan(),
        ConstantValue::BigInt(value) => value.as_ref() != "0",
        ConstantValue::String(value) => !value.as_str().is_empty(),
    }
}

fn evaluate_unary(
    operands: &[AbstractValue],
    evaluate: impl FnOnce(&ConstantValue) -> Option<ConstantValue>,
) -> AbstractValue {
    let [operand] = operands else {
        return AbstractValue::unknown();
    };

    let Some(operand) = operand.constant() else {
        return AbstractValue::unknown();
    };

    evaluate(operand)
        .map(AbstractValue::from_constant)
        .unwrap_or_else(AbstractValue::unknown)
}

fn evaluate_binary(
    operands: &[AbstractValue],
    evaluate: impl FnOnce(&ConstantValue, &ConstantValue) -> Option<ConstantValue>,
) -> AbstractValue {
    let [left, right] = operands else {
        return AbstractValue::unknown();
    };

    let (Some(left), Some(right)) = (left.constant(), right.constant()) else {
        return AbstractValue::unknown();
    };

    evaluate(left, right)
        .map(AbstractValue::from_constant)
        .unwrap_or_else(AbstractValue::unknown)
}

fn evaluate_unary_operator(
    operator: UnaryOperator,
    operand: &ConstantValue,
) -> Option<ConstantValue> {
    match operator {
        UnaryOperator::LogicalNot => Some(ConstantValue::Boolean(!constant_truthiness(operand))),

        UnaryOperator::Plus => match operand {
            ConstantValue::Number(value) => Some(ConstantValue::Number(*value)),
            _ => None,
        },

        UnaryOperator::Negate => match operand {
            ConstantValue::Number(value) => Some(ConstantValue::Number(-value)),
            _ => None,
        },

        UnaryOperator::Void => Some(ConstantValue::Undefined),

        // Correct folding requires explicit ToInt32 semantics.
        UnaryOperator::BitwiseNot => None,
    }
}

fn evaluate_binary_operator(
    operator: BinaryOperator,
    left: &ConstantValue,
    right: &ConstantValue,
) -> Option<ConstantValue> {
    match operator {
        BinaryOperator::StrictEqual => Some(ConstantValue::Boolean(strictly_equal(left, right))),

        BinaryOperator::StrictNotEqual => {
            Some(ConstantValue::Boolean(!strictly_equal(left, right)))
        }

        operator => {
            let (ConstantValue::Number(left), ConstantValue::Number(right)) = (left, right) else {
                return None;
            };

            Some(match operator {
                BinaryOperator::Add => ConstantValue::Number(left + right),
                BinaryOperator::Subtract => ConstantValue::Number(left - right),
                BinaryOperator::Multiply => ConstantValue::Number(left * right),
                BinaryOperator::Divide => ConstantValue::Number(left / right),
                BinaryOperator::Remainder => ConstantValue::Number(left % right),

                BinaryOperator::LessThan => ConstantValue::Boolean(left < right),
                BinaryOperator::LessThanOrEqual => ConstantValue::Boolean(left <= right),
                BinaryOperator::GreaterThan => ConstantValue::Boolean(left > right),
                BinaryOperator::GreaterThanOrEqual => ConstantValue::Boolean(left >= right),

                // These require additional ECMAScript coercion, integer, BigInt,
                // property, prototype, or exponentiation semantics.
                BinaryOperator::Exponentiate
                | BinaryOperator::LooseEqual
                | BinaryOperator::LooseNotEqual
                | BinaryOperator::StrictEqual
                | BinaryOperator::StrictNotEqual
                | BinaryOperator::In
                | BinaryOperator::InstanceOf
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::UnsignedShiftRight
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::BitwiseAnd => return None,
            })
        }
    }
}

fn strictly_equal(left: &ConstantValue, right: &ConstantValue) -> bool {
    match (left, right) {
        (ConstantValue::Undefined, ConstantValue::Undefined)
        | (ConstantValue::Null, ConstantValue::Null) => true,

        (ConstantValue::Boolean(left), ConstantValue::Boolean(right)) => left == right,

        (ConstantValue::Number(left), ConstantValue::Number(right)) => {
            !left.is_nan() && !right.is_nan() && left == right
        }

        (ConstantValue::BigInt(left), ConstantValue::BigInt(right)) => left == right,
        (ConstantValue::String(left), ConstantValue::String(right)) => left == right,

        _ => false,
    }
}

fn constant_typeof(value: &ConstantValue) -> &'static str {
    match value {
        ConstantValue::Undefined => "undefined",
        ConstantValue::Boolean(_) => "boolean",
        ConstantValue::Null => "object",
        ConstantValue::Number(_) => "number",
        ConstantValue::BigInt(_) => "bigint",
        ConstantValue::String(_) => "string",
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::{BinaryOp, BinaryOperator, ConstantOp, ConstantValue, OperationKind};

    use super::{evaluate_result, strictly_equal};
    use crate::analysis::AbstractValue;

    #[test]
    fn evaluates_constant_operations() {
        let result = evaluate_result(
            &OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
            &[],
            0,
        );

        assert_eq!(result.constant(), Some(&ConstantValue::Number(1.0)));
    }

    #[test]
    fn folds_numeric_addition() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            &[
                AbstractValue::from_constant(ConstantValue::Number(20.0)),
                AbstractValue::from_constant(ConstantValue::Number(22.0)),
            ],
            0,
        );

        assert_eq!(result.constant(), Some(&ConstantValue::Number(42.0)));
    }

    #[test]
    fn does_not_fold_unsupported_coercive_addition() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            &[
                AbstractValue::from_constant(ConstantValue::String(evrel_ir::JsString::new(
                    "20", false,
                ))),
                AbstractValue::from_constant(ConstantValue::Number(22.0)),
            ],
            0,
        );

        assert_eq!(result, AbstractValue::unknown());
    }

    #[test]
    fn follows_javascript_nan_equality() {
        let nan = ConstantValue::Number(f64::NAN);

        assert!(!strictly_equal(&nan, &nan));
    }

    #[test]
    fn propagates_an_unavailable_operand() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            &[
                AbstractValue::bottom(),
                AbstractValue::from_constant(ConstantValue::Number(1.0)),
            ],
            0,
        );

        assert!(result.is_bottom());
    }
}
