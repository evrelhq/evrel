//! JavaScript destructuring.

use crate::{AssignmentPattern, BindingPattern, RegionId};

use super::OperationEffects;

/// Determines how destructuring writes its leaf bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingWriteMode {
    /// Initializes lexical bindings such as `let` and `const`.
    Initialize,

    /// Stores into an already-instantiated `var` binding.
    Store,
}

/// Destructures one source value into a compiler-owned binding pattern.
///
/// The source value is the operation's only operand. Binding writes are
/// represented by the pattern rather than SSA operands.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructureBindingOp {
    pattern: BindingPattern,
    mode: BindingWriteMode,
}

impl DestructureBindingOp {
    /// Creates a binding-pattern destructuring operation.
    pub fn new(pattern: BindingPattern, mode: BindingWriteMode) -> Self {
        Self { pattern, mode }
    }

    /// Returns the binding pattern receiving the source value.
    pub const fn pattern(&self) -> &BindingPattern {
        &self.pattern
    }

    /// Returns how leaf bindings are written.
    pub const fn mode(&self) -> BindingWriteMode {
        self.mode
    }

    /// Returns pattern-expression regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.pattern.regions()
    }

    /// Returns the observable effects of destructuring.
    pub const fn effects(&self) -> OperationEffects {
        // Array destructuring executes the iterator protocol.
        OperationEffects::MAY_THROW
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) fn result_count(&self) -> usize {
        self.pattern.binding_ids().len()
    }
}

/// Destructures one source value into assignment destinations.
///
/// The source value is the operation's only operand. Assignment targets and
/// their deferred reference expressions are represented by the pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructureAssignmentOp {
    pattern: AssignmentPattern,
}

impl DestructureAssignmentOp {
    /// Creates a destructuring-assignment operation.
    pub const fn new(pattern: AssignmentPattern) -> Self {
        Self { pattern }
    }

    /// Returns the assignment pattern receiving the source value.
    pub const fn pattern(&self) -> &AssignmentPattern {
        &self.pattern
    }

    /// Returns pattern-expression regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.pattern.regions()
    }

    /// Returns the observable effects of destructuring assignment.
    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::MAY_THROW_AND_OBSERVABLE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AssignmentPattern, AssignmentTarget, BindingId, BindingKind, BindingPattern, ConstantOp,
        ConstantValue, JsModuleIr, LocationId, ModuleBuilder, OperationKind, PatternExpression,
        RegionId,
    };

    use super::{BindingWriteMode, DestructureAssignmentOp, DestructureBindingOp};

    #[test]
    fn defines_binding_destructuring_shape() {
        let binding = BindingId::from_index(0);
        let pattern = BindingPattern::array([Some(BindingPattern::binding(binding))], None);
        let operation = DestructureBindingOp::new(pattern, BindingWriteMode::Initialize);

        assert_eq!(operation.mode(), BindingWriteMode::Initialize);
        assert_eq!(operation.pattern().binding_ids(), [binding]);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
        assert!(operation.effects().may_throw());
    }

    #[test]
    fn allocates_one_result_for_each_binding_leaf() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let operation = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let first = module_builder.create_binding(function, "first", BindingKind::Const);
            let second = module_builder.create_binding(function, "second", BindingKind::Const);
            let mut builder = module_builder.function_builder(function);
            let source = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
            );
            let source = builder.operation_results(source)[0];
            let operation = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::DestructureBinding(DestructureBindingOp::new(
                    BindingPattern::array(
                        [
                            Some(BindingPattern::binding(first)),
                            Some(BindingPattern::binding(second)),
                        ],
                        None,
                    ),
                    BindingWriteMode::Initialize,
                )),
                [source],
            );

            operation
        };

        let operation = module
            .function(function)
            .unwrap()
            .operation(operation)
            .unwrap();

        assert_eq!(operation.results().len(), 2);
    }

    #[test]
    fn defines_assignment_destructuring_shape() {
        let object = RegionId::from_index(0);
        let key = RegionId::from_index(1);
        let pattern = AssignmentPattern::target(AssignmentTarget::ComputedProperty {
            object: PatternExpression::new(object),
            key: PatternExpression::new(key),
        });
        let operation = DestructureAssignmentOp::new(pattern.clone());

        assert_eq!(operation.pattern(), &pattern);
        assert_eq!(operation.regions(), [object, key]);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
        assert!(operation.effects().may_throw());
        assert!(operation.effects().may_have_observable_effects());
    }
}
