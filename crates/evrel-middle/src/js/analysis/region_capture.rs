//! SSA values captured across structured region boundaries.

use std::collections::{BTreeMap, BTreeSet};

use evrel_js_ir::{JsFunctionIr, RegionId, ValueDefinition, ValueId};

/// Immutable capture sets for every region in one function snapshot.
///
/// A region captures an SSA value when an operation in that region or one of
/// its descendants uses the value and the value is defined in an ancestor
/// region. Function parameters are inputs of the function body, so the body
/// itself does not capture them, while nested regions that use them do.
///
/// Recompute after changing regions, operations, operands, block parameters,
/// or the function signature.
#[derive(Debug, Clone)]
pub struct RegionCaptureAnalysis {
    captured_values: BTreeMap<RegionId, Box<[ValueId]>>,
}

impl RegionCaptureAnalysis {
    /// Computes captures for every region owned by `function`.
    pub fn analyze(function: &JsFunctionIr) -> Self {
        let mut captures = function
            .regions()
            .map(|(region, _)| (region, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for (_, operation) in function.operations() {
            let use_region = function
                .block_region(operation.block())
                .expect("operation block must belong to a live region");

            for &operand in operation.operands() {
                let definition_region = value_definition_region(function, operand);
                let mut boundary = use_region;

                while boundary != definition_region {
                    captures
                        .get_mut(&boundary)
                        .expect("operation region must belong to the function")
                        .insert(operand);

                    boundary = function
                        .region(boundary)
                        .expect("captured region must remain live")
                        .parent()
                        .expect("an SSA value must be defined in the use region or an ancestor");
                }
            }
        }

        Self {
            captured_values: captures
                .into_iter()
                .map(|(region, values)| (region, values.into_iter().collect()))
                .collect(),
        }
    }

    /// Returns values captured by `region` in stable value-ID order.
    pub fn captured_values(&self, region: RegionId) -> Option<&[ValueId]> {
        self.captured_values.get(&region).map(Box::as_ref)
    }
}

fn value_definition_region(function: &JsFunctionIr, value: ValueId) -> RegionId {
    match function
        .value(value)
        .expect("operation operand must reference a live value")
        .definition()
    {
        ValueDefinition::FunctionParameter { .. } => function.body_region(),
        ValueDefinition::OperationResult { operation, .. } => {
            let block = function
                .operation(*operation)
                .expect("defining operation must remain live")
                .block();
            function
                .block_region(block)
                .expect("defining operation block must belong to a live region")
        }
        ValueDefinition::BlockParameter { block, .. } => function
            .block_region(*block)
            .expect("parameter block must belong to a live region"),
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        ArrayLiteralElement, ArrayLiteralOp, BinaryOp, BinaryOperator, ConstantOp, ConstantValue,
        JsModuleIr, LocationId, ModuleBuilder, OperationKind,
    };

    use super::RegionCaptureAnalysis;

    #[test]
    fn computes_captures_across_nested_region_boundaries() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (outer, inner, outside, local) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let outside_operation = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
            );
            let outside = builder.operation_results(outside_operation)[0];

            let outer = builder.begin_region(1);
            let local_operation = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
            );
            let local = builder.operation_results(local_operation)[0];

            let inner = builder.begin_region(1);
            let comparison = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [outside, local],
            );
            let comparison = builder.operation_results(comparison)[0];
            builder.finish_region(inner, LocationId::UNKNOWN, [comparison]);

            let inner_owner = builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::ArrayLiteral(ArrayLiteralOp::new([ArrayLiteralElement::Value {
                    expression: inner,
                }])),
                [],
            );
            let outer_result = builder.operation_results(inner_owner)[0];
            builder.finish_region(outer, LocationId::UNKNOWN, [outer_result]);

            builder.append_operation(
                LocationId::UNKNOWN,
                OperationKind::ArrayLiteral(ArrayLiteralOp::new([ArrayLiteralElement::Value {
                    expression: outer,
                }])),
                [],
            );

            (outer, inner, outside, local)
        };

        let function = module.function(function_id).unwrap();
        let body = function.body_region();
        let captures = RegionCaptureAnalysis::analyze(function);

        assert_eq!(captures.captured_values(body), Some([].as_slice()));
        assert_eq!(captures.captured_values(outer), Some([outside].as_slice()));
        assert_eq!(
            captures.captured_values(inner),
            Some([outside, local].as_slice())
        );
    }
}
