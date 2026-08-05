use std::collections::VecDeque;

use evrel_js_ir::{
    ArrayLiteralElement, BinaryOperator, BindingId, BindingKind, BlockParameterSource,
    ClassElement, ClassFieldPlacement, DeleteTarget, FunctionId, JsFunctionIr, JsModuleIr,
    ObjectLiteralEntry, ObjectLiteralKey, OperationId, OperationKind, RegionId, RegionOwner,
    TypeofTarget, UnaryOperator, ValueId,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::direct_eval::is_direct_eval_call;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EscapeNode {
    Binding(BindingId),
    Value(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionRole {
    ContainedBy(ValueId),
    Escapes,
    ObservedOnly,
}

pub(super) struct EscapeGraph<'a> {
    module: &'a JsModuleIr,
    function_id: FunctionId,
    function: &'a JsFunctionIr,
    // An edge `source -> dependency` means that escaping `source` makes
    // `dependency` reachable outside the function as well.
    escape_dependencies: FxHashMap<EscapeNode, FxHashSet<EscapeNode>>,
    escaping: FxHashSet<EscapeNode>,
    worklist: VecDeque<EscapeNode>,
}

impl<'a> EscapeGraph<'a> {
    pub(super) fn analyze(
        module: &'a JsModuleIr,
        function_id: FunctionId,
        function: &'a JsFunctionIr,
    ) -> FxHashSet<ValueId> {
        let mut graph = Self {
            module,
            function_id,
            function,
            escape_dependencies: FxHashMap::default(),
            escaping: FxHashSet::default(),
            worklist: VecDeque::new(),
        };

        graph.add_block_parameter_flow();
        graph.add_operation_flow();
        graph.add_region_flow();
        graph.propagate();

        graph
            .escaping
            .into_iter()
            .filter_map(|node| match node {
                EscapeNode::Value(value) => Some(value),
                EscapeNode::Binding(_) => None,
            })
            .collect()
    }

    fn add_block_parameter_flow(&mut self) {
        for (_, operation) in self.function.operations() {
            for successor in operation.successors() {
                let parameters = self
                    .function
                    .block(successor.target().block())
                    .expect("successor target must be a live block")
                    .parameters();
                let produced = successor.produced_argument_count();
                let arguments = successor.arguments(operation.operands());

                for (index, parameter) in parameters.iter().enumerate() {
                    if parameter.source() != BlockParameterSource::Forwarded || index < produced {
                        continue;
                    }

                    let argument = arguments
                        .get(index - produced)
                        .expect("forwarded block parameter must have an argument");
                    self.add_escape_dependency(
                        EscapeNode::Value(parameter.value()),
                        EscapeNode::Value(*argument),
                    );
                }
            }
        }
    }

    fn add_operation_flow(&mut self) {
        for (operation_id, operation) in self.function.operations() {
            let operands = operation.operation_operands();
            let results = self.operation_results(operation);
            let kind = match operation.kind() {
                OperationKind::Invoke(invoke) => invoke.operation(),
                kind => kind,
            };

            match kind {
                OperationKind::Constant(_)
                | OperationKind::RegExpLiteral(_)
                | OperationKind::ArrayLiteral(_)
                | OperationKind::ObjectLiteral(_)
                | OperationKind::LoadThis(_)
                | OperationKind::MetaProperty(_)
                | OperationKind::LoadGlobal(_)
                | OperationKind::Jump(_)
                | OperationKind::Try(_)
                | OperationKind::For(_)
                | OperationKind::RegionYield(_) => {}

                OperationKind::CreateFunction(_) => {
                    self.add_captured_bindings(operation_id);
                }

                OperationKind::CreateClass(class) => {
                    let result = results[0];
                    self.add_captured_bindings(operation_id);

                    if let Some(binding) = class.self_binding() {
                        self.add_escape_dependency(
                            EscapeNode::Binding(binding),
                            EscapeNode::Value(result),
                        );
                    }

                    if class.elements().iter().any(|element| {
                        matches!(element, ClassElement::Field(field)
                            if field.placement() == ClassFieldPlacement::Static
                                && field.initializer().is_some())
                            || matches!(element, ClassElement::StaticBlock(_))
                    }) {
                        self.seed(EscapeNode::Value(result));
                    }
                }

                OperationKind::LoadArguments(_) => {
                    let arguments = EscapeNode::Value(results[0]);
                    for (binding, data) in self.module.bindings() {
                        if data.declaring_function() == self.function_id
                            && data.kind() == BindingKind::Parameter
                        {
                            self.add_escape_dependency(arguments, EscapeNode::Binding(binding));
                        }
                    }
                }

                OperationKind::DynamicImport(_) => {
                    self.seed_values(operands);
                    self.seed(EscapeNode::Value(results[0]));
                }

                OperationKind::DestructureBinding(_)
                | OperationKind::DestructureAssignment(_)
                | OperationKind::StoreGlobal(_)
                | OperationKind::LoadSuperProperty(_)
                | OperationKind::StoreSuperProperty(_)
                | OperationKind::LoadProperty(_)
                | OperationKind::StoreProperty(_)
                | OperationKind::Update(_)
                | OperationKind::Await(_)
                | OperationKind::Yield(_)
                | OperationKind::Call(_)
                | OperationKind::SuperCall(_)
                | OperationKind::Construct(_)
                | OperationKind::TaggedTemplate(_)
                | OperationKind::TemplateLiteral(_)
                | OperationKind::JsxElement(_)
                | OperationKind::JsxFragment(_)
                | OperationKind::ForIn(_)
                | OperationKind::ForOf(_)
                | OperationKind::Return(_)
                | OperationKind::Throw(_) => self.seed_values(operands),

                OperationKind::Debugger(_) => self.seed_local_bindings(),

                OperationKind::InitializeBinding(binding) => {
                    self.add_binding_store(binding.binding(), operands[0]);
                }
                OperationKind::StoreBinding(binding) => {
                    self.add_binding_store(binding.binding(), operands[0]);
                }
                OperationKind::LoadBinding(binding) => {
                    self.add_escape_dependency(
                        EscapeNode::Value(results[0]),
                        EscapeNode::Binding(binding.binding()),
                    );
                }

                OperationKind::HasPrivateName(_) | OperationKind::IsNullish(_) => {}

                OperationKind::Typeof(operation) => {
                    debug_assert!(
                        operands.is_empty() || matches!(operation.target(), TypeofTarget::Value)
                    );
                }

                OperationKind::Delete(operation) => {
                    if matches!(operation.target(), DeleteTarget::Property(_)) {
                        self.seed_values(operands);
                    }
                }

                OperationKind::Unary(operation) => {
                    if !matches!(
                        operation.operator(),
                        UnaryOperator::LogicalNot | UnaryOperator::Void
                    ) {
                        self.seed_values(operands);
                    }
                }

                OperationKind::Binary(operation) => {
                    if !matches!(
                        operation.operator(),
                        BinaryOperator::StrictEqual | BinaryOperator::StrictNotEqual
                    ) {
                        self.seed_values(operands);
                    }
                }

                OperationKind::If(_)
                | OperationKind::While(_)
                | OperationKind::DoWhile(_)
                | OperationKind::Switch(_)
                | OperationKind::ResumeCompletion(_) => {
                    // Conditions and switch comparisons observe values without
                    // retaining them beyond the current activation.
                }

                OperationKind::EnterFinally(_) => self.seed_values(operands),

                OperationKind::Invoke(_) => unreachable!("invoke operation must be unwrapped"),
            }

            if matches!(kind, OperationKind::ObjectLiteral(_)) {
                self.add_captured_bindings(operation_id);
            }

            if matches!(kind, OperationKind::Call(_))
                && is_direct_eval_call(self.module, self.function, operation_id)
            {
                self.seed_local_bindings();
            }
        }
    }

    fn add_region_flow(&mut self) {
        let roles = self.region_roles();

        for (region_id, region) in self.function.regions() {
            let yielded = self.region_yielded_values(region_id);
            if yielded.is_empty() {
                continue;
            }

            match region.owner() {
                Some(RegionOwner::FunctionBody) => {}
                Some(RegionOwner::FunctionParameter { .. }) | None => self.seed_values(&yielded),
                Some(RegionOwner::Operation(_)) => match roles.get(&region_id).copied() {
                    Some(RegionRole::ContainedBy(container)) => {
                        for value in yielded {
                            self.add_escape_dependency(
                                EscapeNode::Value(container),
                                EscapeNode::Value(value),
                            );
                        }
                    }
                    Some(RegionRole::ObservedOnly) => {}
                    Some(RegionRole::Escapes) | None => self.seed_values(&yielded),
                },
            }
        }
    }

    fn region_roles(&self) -> FxHashMap<RegionId, RegionRole> {
        let mut roles = FxHashMap::default();

        for (_, operation) in self.function.operations() {
            let kind = match operation.kind() {
                OperationKind::Invoke(invoke) => invoke.operation(),
                kind => kind,
            };

            match kind {
                OperationKind::ArrayLiteral(array) => {
                    let container = operation.results()[0];
                    for element in array.elements() {
                        match element {
                            ArrayLiteralElement::Value { expression } => insert_region_role(
                                &mut roles,
                                *expression,
                                RegionRole::ContainedBy(container),
                            ),
                            ArrayLiteralElement::Spread { expression } => {
                                insert_region_role(&mut roles, *expression, RegionRole::Escapes)
                            }
                            ArrayLiteralElement::Elision => {}
                        }
                    }
                }

                OperationKind::ObjectLiteral(object) => {
                    let container = operation.results()[0];
                    for entry in object.entries() {
                        match entry {
                            ObjectLiteralEntry::Property { key, value } => {
                                if let ObjectLiteralKey::Computed { expression } = key {
                                    insert_region_role(
                                        &mut roles,
                                        *expression,
                                        RegionRole::Escapes,
                                    );
                                }
                                insert_region_role(
                                    &mut roles,
                                    *value,
                                    RegionRole::ContainedBy(container),
                                );
                            }
                            ObjectLiteralEntry::Prototype { expression } => insert_region_role(
                                &mut roles,
                                *expression,
                                RegionRole::ContainedBy(container),
                            ),
                            ObjectLiteralEntry::Method {
                                key: ObjectLiteralKey::Computed { expression },
                                ..
                            }
                            | ObjectLiteralEntry::Spread { expression } => {
                                insert_region_role(&mut roles, *expression, RegionRole::Escapes)
                            }
                            ObjectLiteralEntry::Method {
                                key: ObjectLiteralKey::Static(_),
                                ..
                            } => {}
                        }
                    }
                }

                OperationKind::Switch(switch) => {
                    for case in switch.cases() {
                        if let Some(region) = case.test_region() {
                            insert_region_role(&mut roles, region, RegionRole::ObservedOnly);
                        }
                    }
                }

                _ => {
                    for region in operation.regions() {
                        insert_region_role(&mut roles, region, RegionRole::Escapes);
                    }
                }
            }
        }

        roles
    }

    fn operation_results(&self, operation: &evrel_js_ir::OperationData) -> Vec<ValueId> {
        let OperationKind::Invoke(invoke) = operation.kind() else {
            return operation.results().to_vec();
        };
        let block = self
            .function
            .block(invoke.normal_target().block())
            .expect("invoke normal target must remain live");

        block
            .parameters()
            .iter()
            .take(invoke.operation().result_count())
            .map(|parameter| parameter.value())
            .collect()
    }

    fn add_binding_store(&mut self, binding: BindingId, value: ValueId) {
        self.add_escape_dependency(EscapeNode::Binding(binding), EscapeNode::Value(value));

        let data = self
            .module
            .binding(binding)
            .expect("operation must reference a live binding");
        if data.declaring_function() != self.function_id || self.binding_is_exported(binding) {
            self.seed(EscapeNode::Binding(binding));
        }
    }

    fn add_captured_bindings(&mut self, operation: OperationId) {
        let operation = self
            .function
            .operation(operation)
            .expect("capture carrier must be a live operation");
        let Some(&carrier) = operation.results().first() else {
            return;
        };

        let mut referenced_functions = Vec::new();
        operation
            .kind()
            .visit_referenced_functions(|function| referenced_functions.push(function));

        for referenced in referenced_functions {
            for binding in self.captured_bindings(referenced) {
                self.add_escape_dependency(
                    EscapeNode::Value(carrier),
                    EscapeNode::Binding(binding),
                );
            }
        }
    }

    fn captured_bindings(&self, root: FunctionId) -> FxHashSet<BindingId> {
        let descendants = self
            .module
            .functions()
            .filter_map(|(function, _)| self.is_descendant_of(function, root).then_some(function))
            .collect::<FxHashSet<_>>();
        let mut captured = FxHashSet::default();

        for function in descendants {
            let function = self
                .module
                .function(function)
                .expect("capturing function must remain live");
            for (_, operation) in function.operations() {
                operation.kind().visit_referenced_bindings(|binding| {
                    if self
                        .module
                        .binding(binding)
                        .is_some_and(|data| data.declaring_function() == self.function_id)
                    {
                        captured.insert(binding);
                    }
                });
            }
        }

        captured
    }

    fn is_descendant_of(&self, mut function: FunctionId, ancestor: FunctionId) -> bool {
        loop {
            if function == ancestor {
                return true;
            }
            let Some(parent) = self
                .module
                .function(function)
                .and_then(JsFunctionIr::parent_function)
            else {
                return false;
            };
            function = parent;
        }
    }

    fn binding_is_exported(&self, binding: BindingId) -> bool {
        self.module
            .exports()
            .iter()
            .any(|export| export.binding() == Some(binding))
    }

    fn seed_local_bindings(&mut self) {
        let bindings = self
            .module
            .bindings()
            .filter_map(|(binding, data)| {
                (data.declaring_function() == self.function_id).then_some(binding)
            })
            .collect::<Vec<_>>();
        for binding in bindings {
            self.seed(EscapeNode::Binding(binding));
        }
    }

    fn region_yielded_values(&self, region: RegionId) -> Vec<ValueId> {
        self.function
            .region_blocks(region)
            .filter_map(|(_, block)| block.terminator())
            .filter_map(|operation| self.function.operation(operation))
            .filter(|operation| matches!(operation.kind(), OperationKind::RegionYield(_)))
            .flat_map(|operation| operation.operands().iter().copied())
            .collect()
    }

    fn seed_values(&mut self, values: &[ValueId]) {
        for value in values {
            self.seed(EscapeNode::Value(*value));
        }
    }

    fn add_escape_dependency(&mut self, escaping: EscapeNode, dependency: EscapeNode) {
        self.escape_dependencies
            .entry(escaping)
            .or_default()
            .insert(dependency);
    }

    fn seed(&mut self, node: EscapeNode) {
        if self.escaping.insert(node) {
            self.worklist.push_back(node);
        }
    }

    fn propagate(&mut self) {
        while let Some(node) = self.worklist.pop_front() {
            let dependencies = self
                .escape_dependencies
                .get(&node)
                .into_iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            for dependency in dependencies {
                self.seed(dependency);
            }
        }
    }
}

fn insert_region_role(
    roles: &mut FxHashMap<RegionId, RegionRole>,
    region: RegionId,
    role: RegionRole,
) {
    assert!(
        roles.insert(region, role).is_none(),
        "an inline region must have exactly one semantic role"
    );
}
