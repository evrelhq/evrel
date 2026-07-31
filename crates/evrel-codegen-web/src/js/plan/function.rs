//! Planning for one Evrel function.

use std::collections::{HashMap, HashSet};

use evrel_js_ir::{
    BindingId, FunctionId, JsFunctionIr, JsModuleIr, OperationId, OperationKind, RegionId,
    ValueDefinition, ValueId,
};

use crate::{
    JsCodegenError,
    js::name::{JsNameAllocator, JsReservedNames},
};

use super::{
    DenseMap, JsControlPlan, JsEdgeKey, JsEdgeTransfer, JsExpressionRegionPlan, JsLocalAllocator,
    JsLocalId, JsNamePlan, JsOperationPlan, JsValueRepresentation, build_edge_transfers,
};

/// JavaScript representation decisions for one function.
#[derive(Debug)]
pub(crate) struct JsFunctionPlan {
    values: DenseMap<evrel_js_ir::ValueId, JsValueRepresentation>,
    operations: DenseMap<OperationId, JsOperationPlan>,
    binding_names: DenseMap<BindingId, Box<str>>,
    names: JsNamePlan,
    control: JsControlPlan,
    regions: DenseMap<RegionId, JsExpressionRegionPlan>,
    edge_transfers: HashMap<JsEdgeKey, JsEdgeTransfer>,
}

impl JsFunctionPlan {
    pub(crate) fn build(
        module: &JsModuleIr,
        function_id: FunctionId,
        function: &JsFunctionIr,
        reserved_names: &JsReservedNames,
    ) -> Result<Self, JsCodegenError> {
        let mut values = DenseMap::new();
        let mut locals = JsLocalAllocator::default();

        for (value_id, value) in function.values() {
            if let ValueDefinition::FunctionParameter { parameter_index } = *value.definition() {
                let parameter = function
                    .parameters()
                    .get(parameter_index as usize)
                    .ok_or(JsCodegenError::UnsupportedValue { value: value_id })?;

                if let Some(binding) = parameter.target().as_binding() {
                    values.insert(value_id, JsValueRepresentation::Binding(binding));
                }

                continue;
            }

            if let ValueDefinition::BlockParameter {
                block,
                parameter_index,
            } = *value.definition()
            {
                function
                    .block(block)
                    .and_then(|block| block.parameters().get(parameter_index as usize))
                    .ok_or(JsCodegenError::UnsupportedValue { value: value_id })?;

                let local = locals.allocate();
                values.insert(value_id, JsValueRepresentation::Temporary(local));
                continue;
            }

            let ValueDefinition::OperationResult {
                operation,
                result_index,
            } = *value.definition()
            else {
                continue;
            };

            let operation_data = function
                .operation(operation)
                .ok_or(JsCodegenError::UnknownOperation { operation })?;

            if matches!(operation_data.kind(), OperationKind::Update(_)) && result_index < 2 {
                let local = locals.allocate();
                values.insert(value_id, JsValueRepresentation::Temporary(local));

                continue;
            }

            if result_index != 0 {
                continue;
            }

            let representation = match operation_data.kind() {
                OperationKind::Constant(_)
                | OperationKind::LoadThis(_)
                | OperationKind::LoadArguments(_)
                | OperationKind::MetaProperty(_) => Some(JsValueRepresentation::Inline),
                OperationKind::LoadGlobal(global)
                    if global.name() == "eval" && is_direct_eval_use(function, value_id) =>
                {
                    Some(JsValueRepresentation::DirectEval)
                }
                OperationKind::JsxElement(_) | OperationKind::JsxFragment(_) => {
                    Some(JsValueRepresentation::Temporary(locals.allocate()))
                }
                OperationKind::TaggedTemplate(template)
                    if matches!(
                        template.target(),
                        evrel_js_ir::CallTarget::Value {
                            receiver: evrel_js_ir::CallReceiver::Explicit
                        }
                    ) =>
                {
                    None
                }
                OperationKind::CreateFunction(_) | OperationKind::CreateClass(_)
                    if is_direct_creation_use(function, operation, value_id) =>
                {
                    Some(JsValueRepresentation::CreationAtUse)
                }
                _ if !value.uses().is_empty() => {
                    Some(JsValueRepresentation::Temporary(locals.allocate()))
                }
                _ => None,
            };

            if let Some(representation) = representation {
                values.insert(value_id, representation);
            }
        }
        let mut operations = DenseMap::new();
        for (operation_id, operation) in function.operations() {
            let operation_plan = match operation.kind() {
                OperationKind::Constant(_)
                | OperationKind::LoadThis(_)
                | OperationKind::LoadArguments(_)
                | OperationKind::MetaProperty(_) => Some(JsOperationPlan::Omitted),
                OperationKind::CreateFunction(_) | OperationKind::CreateClass(_)
                    if operation.results().first().is_some_and(|result| {
                        values.get(*result).copied() == Some(JsValueRepresentation::CreationAtUse)
                    }) =>
                {
                    Some(JsOperationPlan::Omitted)
                }
                OperationKind::LoadGlobal(_)
                    if operation.results().first().is_some_and(|result| {
                        values.get(*result).copied() == Some(JsValueRepresentation::DirectEval)
                    }) =>
                {
                    Some(JsOperationPlan::Omitted)
                }
                OperationKind::InitializeBinding(initialize)
                    if is_hoisted_function_initialization(
                        module,
                        function,
                        operation_id,
                        initialize.binding(),
                    )? =>
                {
                    let [value] = operation.operands() else {
                        return Err(JsCodegenError::MalformedOperation {
                            operation: operation_id,
                        });
                    };
                    let Some(function) = created_function(module, function, *value)? else {
                        return Err(JsCodegenError::MalformedOperation {
                            operation: operation_id,
                        });
                    };
                    Some(JsOperationPlan::FunctionDeclaration {
                        function,
                        binding: initialize.binding(),
                    })
                }
                OperationKind::InitializeBinding(initialize)
                    if module
                        .binding(initialize.binding())
                        .is_some_and(|binding| binding.kind() == evrel_js_ir::BindingKind::Var)
                        && operation
                            .operands()
                            .first()
                            .is_some_and(|value| is_undefined(function, *value)) =>
                {
                    Some(JsOperationPlan::VarDeclaration)
                }
                _ => None,
            };
            if let Some(operation_plan) = operation_plan {
                operations.insert(operation_id, operation_plan);
            }
        }
        let control = JsControlPlan::build(function_id, function, &values, &mut locals)?;
        let mut regions = DenseMap::new();
        for (region_id, _) in function.regions() {
            if region_id != function.body_region() {
                regions.insert(
                    region_id,
                    JsExpressionRegionPlan::build(function, region_id)?,
                );
            }
        }
        let mut binding_allocators = HashMap::new();
        let mut binding_names = DenseMap::new();
        for (ordinal, (binding_id, binding)) in module.bindings().enumerate() {
            let allocator = binding_allocators
                .entry(binding.declaring_function())
                .or_insert_with(|| JsNameAllocator::new(reserved_names));
            binding_names.insert(
                binding_id,
                allocator.allocate_binding(binding.name(), ordinal),
            );
        }

        let mut edge_keys = Vec::new();
        let mut seen_edges = HashSet::new();
        control.body().visit_edges(&mut |edge| {
            if seen_edges.insert(edge) {
                edge_keys.push(edge);
            }
        });

        for region in regions.values() {
            region.visit_edges(&mut |edge| {
                if seen_edges.insert(edge) {
                    edge_keys.push(edge);
                }
            });
        }
        let edge_transfers = build_edge_transfers(function, &edge_keys, &values, &mut locals)?;
        let names = JsNamePlan::build(locals.count(), JsNameAllocator::new(reserved_names));

        let plan = Self {
            values,
            operations,
            binding_names,
            names,
            control,
            regions,
            edge_transfers,
        };

        Ok(plan)
    }

    pub(crate) const fn control(&self) -> &JsControlPlan {
        &self.control
    }

    pub(crate) fn edge_transfer(&self, edge: JsEdgeKey) -> Option<&JsEdgeTransfer> {
        self.edge_transfers.get(&edge)
    }

    pub(crate) fn region(&self, region: RegionId) -> Option<&JsExpressionRegionPlan> {
        self.regions.get(region)
    }

    pub(crate) fn operation(&self, operation: OperationId) -> Option<JsOperationPlan> {
        self.operations.get(operation).copied()
    }

    pub(crate) const fn local_count(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn value(&self, value: evrel_js_ir::ValueId) -> Option<JsValueRepresentation> {
        self.values.get(value).copied()
    }

    pub(crate) fn binding_name(&self, binding: BindingId) -> Option<&str> {
        self.binding_names.get(binding).map(Box::as_ref)
    }

    pub(crate) fn local_name(&self, local: JsLocalId) -> Option<&str> {
        self.names.local(local)
    }
}

fn created_function(
    module: &JsModuleIr,
    function: &JsFunctionIr,
    value: ValueId,
) -> Result<Option<FunctionId>, JsCodegenError> {
    let Some(value) = function.value(value) else {
        return Ok(None);
    };
    let ValueDefinition::OperationResult {
        operation,
        result_index: 0,
    } = *value.definition()
    else {
        return Ok(None);
    };
    let operation = function
        .operation(operation)
        .ok_or(JsCodegenError::UnknownOperation { operation })?;
    let OperationKind::CreateFunction(create) = operation.kind() else {
        return Ok(None);
    };
    module
        .function(create.function())
        .ok_or(JsCodegenError::UnknownFunction {
            function: create.function(),
        })?;
    Ok(Some(create.function()))
}

fn is_undefined(function: &JsFunctionIr, value: ValueId) -> bool {
    function.value(value).is_some_and(|value| {
        let ValueDefinition::OperationResult { operation, .. } = *value.definition() else {
            return false;
        };
        function.operation(operation).is_some_and(|operation| {
            matches!(
                operation.kind(),
                OperationKind::Constant(constant)
                    if matches!(constant.value(), evrel_js_ir::ConstantValue::Undefined)
            )
        })
    })
}

fn is_hoisted_function_initialization(
    module: &JsModuleIr,
    function: &JsFunctionIr,
    initialization: OperationId,
    binding: BindingId,
) -> Result<bool, JsCodegenError> {
    if !module
        .binding(binding)
        .is_some_and(|binding| binding.kind() == evrel_js_ir::BindingKind::Function)
    {
        return Ok(false);
    }
    let entry = function
        .block(function.entry_block())
        .ok_or(JsCodegenError::UnknownBlock {
            block: function.entry_block(),
        })?;
    for &operation in entry.operations() {
        let operation_data = function
            .operation(operation)
            .ok_or(JsCodegenError::UnknownOperation { operation })?;
        if operation == initialization {
            return Ok(true);
        }
        match operation_data.kind() {
            OperationKind::CreateFunction(_) => {}
            OperationKind::InitializeBinding(initialize)
                if module.binding(initialize.binding()).is_some_and(|binding| {
                    binding.kind() == evrel_js_ir::BindingKind::Function
                }) => {}
            _ => return Ok(false),
        }
    }
    Ok(false)
}

fn is_direct_creation_use(
    function: &JsFunctionIr,
    definition: OperationId,
    value: ValueId,
) -> bool {
    let Some(value_data) = function.value(value) else {
        return false;
    };
    let [use_site] = value_data.uses() else {
        return false;
    };
    let Some(user) = function.operation(use_site.operation()) else {
        return false;
    };
    let Some(definition_data) = function.operation(definition) else {
        return false;
    };
    if definition_data.block() != user.block() || !user.successors().is_empty() {
        return false;
    }

    let Some(block) = function.block(definition_data.block()) else {
        return false;
    };
    match user.kind() {
        OperationKind::InitializeBinding(_) | OperationKind::StoreBinding(_)
            if use_site.operand_index() == 0 =>
        {
            block
                .operations()
                .windows(2)
                .any(|pair| pair == [definition, use_site.operation()])
        }
        OperationKind::StoreProperty(_) => block
            .operations()
            .windows(2)
            .any(|pair| pair == [definition, use_site.operation()]),
        OperationKind::RegionYield(_) => {
            block.operations() == [definition] && block.terminator() == Some(use_site.operation())
        }
        _ => false,
    }
}

fn is_direct_eval_use(function: &JsFunctionIr, value: ValueId) -> bool {
    let Some(value) = function.value(value) else {
        return false;
    };
    let [use_site] = value.uses() else {
        return false;
    };
    if use_site.operand_index() != 0 {
        return false;
    }
    let Some(user) = function.operation(use_site.operation()) else {
        return false;
    };

    matches!(
        user.kind(),
        OperationKind::Call(call)
            if matches!(
                call.target(),
                evrel_js_ir::CallTarget::Value {
                    receiver: evrel_js_ir::CallReceiver::None
                }
            )
    )
}
