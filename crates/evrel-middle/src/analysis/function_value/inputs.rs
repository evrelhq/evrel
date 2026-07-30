//! External facts supplied to a function-value analysis.

use evrel_ir::ValueId;
use rustc_hash::FxHashMap;

use super::AbstractValue;

/// Optional value facts supplied by an enclosing analysis context.
///
/// Boundary facts constrain values entering the function or one of its
/// independently entered regions. Result facts constrain operation results
/// whose semantics were modeled outside the current function.
///
/// Result facts describe only the value produced when an operation completes
/// normally. They do not make the defining operation non-throwing, removable,
/// or free of observable effects.
#[derive(Debug, Clone, Default)]
pub struct FunctionValueInputs {
    boundary_values: FxHashMap<ValueId, AbstractValue>,
    result_values: FxHashMap<ValueId, AbstractValue>,
}

impl FunctionValueInputs {
    /// Creates an empty set of external value facts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies a fact for a function or region boundary value.
    pub fn set_boundary_value(&mut self, value: ValueId, fact: AbstractValue) {
        self.boundary_values.insert(value, fact);
    }

    /// Supplies a fact for an operation result.
    pub fn set_result_value(&mut self, value: ValueId, fact: AbstractValue) {
        self.result_values.insert(value, fact);
    }

    /// Returns the supplied boundary fact for a value.
    pub fn boundary_value(&self, value: ValueId) -> Option<&AbstractValue> {
        self.boundary_values.get(&value)
    }

    /// Returns the supplied operation-result fact for a value.
    pub fn result_value(&self, value: ValueId) -> Option<&AbstractValue> {
        self.result_values.get(&value)
    }

    /// Returns whether no external facts have been supplied.
    pub fn is_empty(&self) -> bool {
        self.boundary_values.is_empty() && self.result_values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        ConstantOp, ConstantValue, ModuleBuilder, ModuleIr, OperationKind, UnwindTarget,
    };

    use super::FunctionValueInputs;
    use crate::analysis::AbstractValue;

    #[test]
    fn stores_boundary_and_result_facts_separately() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (boundary, result) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let boundary = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let result = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );

            (
                builder.operation_results(boundary)[0],
                builder.operation_results(result)[0],
            )
        };

        let mut inputs = FunctionValueInputs::new();

        inputs.set_boundary_value(
            boundary,
            AbstractValue::from_constant(ConstantValue::Number(1.0)),
        );
        inputs.set_result_value(
            result,
            AbstractValue::from_constant(ConstantValue::Number(2.0)),
        );

        assert_eq!(
            inputs
                .boundary_value(boundary)
                .and_then(AbstractValue::constant),
            Some(&ConstantValue::Number(1.0)),
        );
        assert_eq!(
            inputs
                .result_value(result)
                .and_then(AbstractValue::constant),
            Some(&ConstantValue::Number(2.0)),
        );

        assert!(inputs.boundary_value(result).is_none());
        assert!(inputs.result_value(boundary).is_none());
        assert!(!inputs.is_empty());
    }
}
