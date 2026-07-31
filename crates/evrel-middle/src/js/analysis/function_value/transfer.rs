//! Transfer semantics for JavaScript IR operations.

use evrel_js_ir::{
    BinaryOperator, ConstantValue, JsString, OperationKind, TypeofTarget, UnaryOperator,
};

use super::{AbstractValue, ValueTypeSet};

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

        OperationKind::IsNullish(_) => evaluate_is_nullish(operands),

        OperationKind::Unary(operation) => evaluate_unary(operation.operator(), operands),

        OperationKind::Binary(operation) => evaluate_binary(operation.operator(), operands),

        OperationKind::Typeof(operation) => evaluate_typeof(operation.target(), operands),

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

fn evaluate_is_nullish(operands: &[AbstractValue]) -> AbstractValue {
    let [operand] = operands else {
        return AbstractValue::unknown();
    };

    if let Some(constant) = operand.constant() {
        return AbstractValue::from_constant(ConstantValue::Boolean(matches!(
            constant,
            ConstantValue::Undefined | ConstantValue::Null
        )));
    }

    let types = operand.types();

    if ValueTypeSet::NULLISH.contains(types) {
        AbstractValue::from_constant(ConstantValue::Boolean(true))
    } else if !types.intersects(ValueTypeSet::NULLISH) {
        AbstractValue::from_constant(ConstantValue::Boolean(false))
    } else {
        AbstractValue::of_types(ValueTypeSet::BOOLEAN)
    }
}

fn evaluate_typeof(target: &TypeofTarget, operands: &[AbstractValue]) -> AbstractValue {
    if matches!(target, TypeofTarget::Value) {
        let [operand] = operands else {
            return AbstractValue::unknown();
        };

        if let Some(constant) = operand.constant() {
            return AbstractValue::from_constant(ConstantValue::String(JsString::new(
                constant_typeof(constant),
                false,
            )));
        }
    }

    AbstractValue::of_types(ValueTypeSet::STRING)
}

fn evaluate_unary(operator: UnaryOperator, operands: &[AbstractValue]) -> AbstractValue {
    let [operand] = operands else {
        return AbstractValue::unknown();
    };

    if let Some(constant) = operand
        .constant()
        .and_then(|constant| evaluate_unary_operator(operator, constant))
    {
        return AbstractValue::from_constant(constant);
    }

    match operator {
        UnaryOperator::LogicalNot => AbstractValue::of_types(ValueTypeSet::BOOLEAN),

        UnaryOperator::Plus => AbstractValue::of_types(ValueTypeSet::NUMBER),

        UnaryOperator::Negate | UnaryOperator::BitwiseNot => {
            AbstractValue::of_types(ValueTypeSet::NUMBER | ValueTypeSet::BIGINT)
        }

        UnaryOperator::Void => AbstractValue::from_constant(ConstantValue::Undefined),
    }
}

fn evaluate_binary(operator: BinaryOperator, operands: &[AbstractValue]) -> AbstractValue {
    let [left, right] = operands else {
        return AbstractValue::unknown();
    };

    if let (Some(left), Some(right)) = (left.constant(), right.constant())
        && let Some(constant) = evaluate_binary_operator(operator, left, right)
    {
        return AbstractValue::from_constant(constant);
    }

    match operator {
        BinaryOperator::LooseEqual
        | BinaryOperator::LooseNotEqual
        | BinaryOperator::StrictEqual
        | BinaryOperator::StrictNotEqual
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual
        | BinaryOperator::In
        | BinaryOperator::InstanceOf => AbstractValue::of_types(ValueTypeSet::BOOLEAN),

        BinaryOperator::Add => addition_result(left, right),

        BinaryOperator::UnsignedShiftRight => AbstractValue::of_types(ValueTypeSet::NUMBER),

        BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder
        | BinaryOperator::Exponentiate
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::BitwiseAnd => numeric_result(left, right),
    }
}

fn addition_result(left: &AbstractValue, right: &AbstractValue) -> AbstractValue {
    if left.is_definitely(ValueTypeSet::STRING) || right.is_definitely(ValueTypeSet::STRING) {
        AbstractValue::of_types(ValueTypeSet::STRING)
    } else if left.is_definitely(ValueTypeSet::NUMBER) && right.is_definitely(ValueTypeSet::NUMBER)
    {
        AbstractValue::of_types(ValueTypeSet::NUMBER)
    } else if left.is_definitely(ValueTypeSet::BIGINT) && right.is_definitely(ValueTypeSet::BIGINT)
    {
        AbstractValue::of_types(ValueTypeSet::BIGINT)
    } else {
        AbstractValue::of_types(ValueTypeSet::STRING | ValueTypeSet::NUMBER | ValueTypeSet::BIGINT)
    }
}

fn numeric_result(left: &AbstractValue, right: &AbstractValue) -> AbstractValue {
    if left.is_definitely(ValueTypeSet::NUMBER) && right.is_definitely(ValueTypeSet::NUMBER) {
        AbstractValue::of_types(ValueTypeSet::NUMBER)
    } else if left.is_definitely(ValueTypeSet::BIGINT) && right.is_definitely(ValueTypeSet::BIGINT)
    {
        AbstractValue::of_types(ValueTypeSet::BIGINT)
    } else {
        AbstractValue::of_types(ValueTypeSet::NUMBER | ValueTypeSet::BIGINT)
    }
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
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, ConstantOp, ConstantValue, IsNullishOp, OperationKind, TypeofOp,
        UnaryOp, UnaryOperator,
    };

    use super::{evaluate_result, strictly_equal};
    use crate::js::analysis::{AbstractValue, ValueTypeSet};

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
    fn retains_the_result_type_of_unsupported_string_addition() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            &[
                AbstractValue::from_constant(ConstantValue::String(evrel_js_ir::JsString::new(
                    "20", false,
                ))),
                AbstractValue::from_constant(ConstantValue::Number(22.0)),
            ],
            0,
        );

        assert_eq!(result.constant(), None);
        assert_eq!(result.types(), ValueTypeSet::STRING);
    }

    #[test]
    fn retains_numeric_type_after_constant_information_is_lost() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            &[
                AbstractValue::of_types(ValueTypeSet::NUMBER),
                AbstractValue::of_types(ValueTypeSet::NUMBER),
            ],
            0,
        );

        assert_eq!(result.constant(), None);
        assert_eq!(result.types(), ValueTypeSet::NUMBER);
    }

    #[test]
    fn classifies_comparison_results_as_boolean() {
        let result = evaluate_result(
            &OperationKind::Binary(BinaryOp::new(BinaryOperator::LessThan)),
            &[AbstractValue::unknown(), AbstractValue::unknown()],
            0,
        );

        assert_eq!(result.constant(), None);
        assert_eq!(result.types(), ValueTypeSet::BOOLEAN);
    }

    #[test]
    fn classifies_typeof_results_as_strings() {
        let result = evaluate_result(
            &OperationKind::Typeof(TypeofOp::value()),
            &[AbstractValue::unknown()],
            0,
        );

        assert_eq!(result.constant(), None);
        assert_eq!(result.types(), ValueTypeSet::STRING);
    }

    #[test]
    fn infers_nullish_results_from_type_facts() {
        let nullish = evaluate_result(
            &OperationKind::IsNullish(IsNullishOp::new()),
            &[AbstractValue::of_types(ValueTypeSet::NULLISH)],
            0,
        );
        let number = evaluate_result(
            &OperationKind::IsNullish(IsNullishOp::new()),
            &[AbstractValue::of_types(ValueTypeSet::NUMBER)],
            0,
        );
        let unknown = evaluate_result(
            &OperationKind::IsNullish(IsNullishOp::new()),
            &[AbstractValue::unknown()],
            0,
        );

        assert_eq!(nullish.constant(), Some(&ConstantValue::Boolean(true)));
        assert_eq!(number.constant(), Some(&ConstantValue::Boolean(false)));
        assert_eq!(unknown.constant(), None);
        assert_eq!(unknown.types(), ValueTypeSet::BOOLEAN);
    }

    #[test]
    fn classifies_unary_results() {
        let logical_not = evaluate_result(
            &OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            &[AbstractValue::unknown()],
            0,
        );
        let plus = evaluate_result(
            &OperationKind::Unary(UnaryOp::new(UnaryOperator::Plus)),
            &[AbstractValue::unknown()],
            0,
        );
        let negate = evaluate_result(
            &OperationKind::Unary(UnaryOp::new(UnaryOperator::Negate)),
            &[AbstractValue::unknown()],
            0,
        );
        let void = evaluate_result(
            &OperationKind::Unary(UnaryOp::new(UnaryOperator::Void)),
            &[AbstractValue::unknown()],
            0,
        );

        assert_eq!(logical_not.types(), ValueTypeSet::BOOLEAN);
        assert_eq!(plus.types(), ValueTypeSet::NUMBER);
        assert_eq!(negate.types(), ValueTypeSet::NUMBER | ValueTypeSet::BIGINT);
        assert_eq!(void.constant(), Some(&ConstantValue::Undefined));
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
