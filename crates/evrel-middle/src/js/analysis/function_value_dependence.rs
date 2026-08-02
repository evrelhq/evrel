//! Function-local SSA data dependence.

use std::collections::{BTreeMap, BTreeSet};

use evrel_js_ir::{BlockParameterSource, JsFunctionIr, OperationData, RegionId, ValueId};

/// Immutable direct data-dependence edges for one function snapshot.
///
/// Operation results depend on their explicit operands and on values yielded
/// by owned regions. Forwarded block parameters depend on their incoming SSA
/// arguments. Values produced implicitly by a control-flow edge conservatively
/// depend on all operands of that edge's terminator.
///
/// This analysis deliberately does not model control dependence, mutation of
/// bindings or objects, calls into other functions, or reactive invalidation.
/// Recompute it after changing values, operations, operands, regions, block
/// parameters, or control-flow successors.
#[derive(Debug, Clone)]
pub struct FunctionValueDependenceAnalysis {
    direct_dependencies: BTreeMap<ValueId, Box<[ValueId]>>,
}

impl FunctionValueDependenceAnalysis {
    /// Computes data dependence for every live SSA value in `function`.
    pub fn analyze(function: &JsFunctionIr) -> Self {
        let mut dependencies = function
            .values()
            .map(|(value, _)| (value, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for (_, operation) in function.operations() {
            add_operation_result_dependencies(function, operation, &mut dependencies);
            add_successor_dependencies(function, operation, &mut dependencies);
        }

        Self {
            direct_dependencies: dependencies
                .into_iter()
                .map(|(value, dependencies)| (value, dependencies.into_iter().collect()))
                .collect(),
        }
    }

    /// Returns the immediate SSA inputs of `value` in stable value-ID order.
    pub fn direct_dependencies(&self, value: ValueId) -> Option<&[ValueId]> {
        self.direct_dependencies.get(&value).map(Box::as_ref)
    }

    /// Returns all strict transitive SSA inputs of `value` in value-ID order.
    ///
    /// The returned set excludes `value` itself, including for loop-carried
    /// dependence cycles.
    pub fn transitive_dependencies(&self, value: ValueId) -> Option<Box<[ValueId]>> {
        self.direct_dependencies.get(&value)?;

        let mut visited = BTreeSet::from([value]);
        let mut pending = vec![value];

        while let Some(current) = pending.pop() {
            for &dependency in self
                .direct_dependencies
                .get(&current)
                .expect("every dependency must be a live value")
                .iter()
                .rev()
            {
                if visited.insert(dependency) {
                    pending.push(dependency);
                }
            }
        }

        visited.remove(&value);
        Some(visited.into_iter().collect())
    }

    /// Returns whether `value` has `dependency` as a strict data dependency.
    pub fn depends_on(&self, value: ValueId, dependency: ValueId) -> bool {
        if value == dependency
            || !self.direct_dependencies.contains_key(&value)
            || !self.direct_dependencies.contains_key(&dependency)
        {
            return false;
        }

        let mut visited = BTreeSet::from([value]);
        let mut pending = vec![value];

        while let Some(current) = pending.pop() {
            for &candidate in self
                .direct_dependencies
                .get(&current)
                .expect("every dependency must be a live value")
            {
                if candidate == dependency {
                    return true;
                }
                if visited.insert(candidate) {
                    pending.push(candidate);
                }
            }
        }

        false
    }
}

fn add_operation_result_dependencies(
    function: &JsFunctionIr,
    operation: &OperationData,
    dependencies: &mut BTreeMap<ValueId, BTreeSet<ValueId>>,
) {
    if operation.results().is_empty() {
        return;
    }

    let yielded = operation
        .regions()
        .into_iter()
        .flat_map(|region| region_yielded_values(function, region));
    let inputs = operation
        .operands()
        .iter()
        .copied()
        .chain(yielded)
        .collect::<BTreeSet<_>>();

    for result in operation.results() {
        dependencies
            .get_mut(result)
            .expect("operation result must be a live value")
            .extend(inputs.iter().copied());
    }
}

fn add_successor_dependencies(
    function: &JsFunctionIr,
    operation: &OperationData,
    dependencies: &mut BTreeMap<ValueId, BTreeSet<ValueId>>,
) {
    for successor in operation.successors() {
        let block = function
            .block(successor.target().block())
            .expect("successor target must be a live block");
        let produced_count = successor.produced_argument_count();
        let forwarded = successor.arguments(operation.operands());

        assert_eq!(
            block.parameters().len(),
            produced_count + forwarded.len(),
            "successor inputs must match target block parameters",
        );

        for (index, parameter) in block.parameters().iter().copied().enumerate() {
            let inputs: &[ValueId] = match parameter.source() {
                BlockParameterSource::Produced if index < produced_count => operation.operands(),
                BlockParameterSource::Forwarded if index >= produced_count => {
                    std::slice::from_ref(&forwarded[index - produced_count])
                }
                BlockParameterSource::Exception
                | BlockParameterSource::Produced
                | BlockParameterSource::Forwarded => {
                    panic!("block parameter source does not match its incoming edge")
                }
            };

            dependencies
                .get_mut(&parameter.value())
                .expect("block parameter must be a live value")
                .extend(inputs.iter().copied());
        }
    }
}

fn region_yielded_values(
    function: &JsFunctionIr,
    region: RegionId,
) -> impl Iterator<Item = ValueId> + '_ {
    function
        .region_blocks(region)
        .filter_map(|(_, block)| block.terminator())
        .filter_map(|operation| function.operation(operation))
        .filter(|operation| matches!(operation.kind(), evrel_js_ir::OperationKind::RegionYield(_)))
        .flat_map(|operation| operation.operands().iter().copied())
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        ArrayLiteralElement, ArrayLiteralOp, BinaryOp, BinaryOperator, BlockParameterSource,
        BlockTarget, ConstantOp, ConstantValue, JsModuleIr, JumpOp, LocationId, ModuleBuilder,
        OperationKind, UnaryOp, UnaryOperator, ValueType,
    };

    use super::FunctionValueDependenceAnalysis;

    #[test]
    fn follows_operands_and_structured_region_results() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (outside, local, comparison, array) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let outside = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
            );
            let outside = builder.operation_results(outside)[0];

            let expression = builder.begin_region(1);
            let local = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
            );
            let local = builder.operation_results(local)[0];
            let comparison = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [outside, local],
            );
            let comparison = builder.operation_results(comparison)[0];
            builder.finish_region(expression, LocationId::UNKNOWN, [comparison]);

            let array = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::ArrayLiteral(ArrayLiteralOp::new([ArrayLiteralElement::Value {
                    expression,
                }])),
                [],
            );
            let array = builder.operation_results(array)[0];

            (outside, local, comparison, array)
        };

        let analysis =
            FunctionValueDependenceAnalysis::analyze(module.function(function_id).unwrap());

        assert_eq!(
            analysis.direct_dependencies(comparison),
            Some([outside, local].as_slice())
        );
        assert_eq!(
            analysis.direct_dependencies(array),
            Some([comparison].as_slice())
        );
        assert_eq!(
            analysis.transitive_dependencies(array),
            Some([outside, local, comparison].into())
        );
        assert!(analysis.depends_on(array, outside));
        assert!(!analysis.depends_on(outside, array));
    }

    #[test]
    fn follows_forwarded_block_parameters_through_a_cycle() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (input, parameter, result) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let target = builder.create_block();
            let parameter = builder.append_block_parameter(
                target,
                BlockParameterSource::Forwarded,
                ValueType::JsValue,
            );

            let input = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
            );
            let input = builder.operation_results(input)[0];
            builder.terminate(
                LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 1))),
                [input],
            );

            builder.switch_to_block(target);
            let result = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
                [parameter],
            );
            let result = builder.operation_results(result)[0];
            builder.terminate(
                LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 1))),
                [result],
            );

            (input, parameter, result)
        };

        let analysis =
            FunctionValueDependenceAnalysis::analyze(module.function(function_id).unwrap());

        assert_eq!(
            analysis.direct_dependencies(parameter),
            Some([input, result].as_slice())
        );
        assert_eq!(
            analysis.transitive_dependencies(result),
            Some([parameter, input].into())
        );
        assert!(analysis.depends_on(result, input));
        assert!(!analysis.depends_on(result, result));
    }
}
