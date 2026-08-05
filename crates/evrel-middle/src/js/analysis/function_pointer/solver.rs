use std::collections::VecDeque;

use evrel_js_ir::{
    ArrayLiteralElement, BinaryOperator, BindingId, BindingKind, BlockParameterSource,
    ClassElement, ClassFieldPlacement, DeleteTarget, FunctionId, JsFunctionIr, JsModuleIr,
    MetaPropertyKind, ObjectLiteralEntry, ObjectLiteralKey, OperationData, OperationId,
    OperationKind, RegionId, RegionOwner, TypeofTarget, UnaryOperator, ValueId,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::direct_eval::is_direct_eval_call;
use super::points_to::{AnalysisOwner, SparseBitSet};
use super::{AbstractObject, AbstractObjectId, AbstractObjectKind, PointsToSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PointerNode {
    Binding(BindingId),
    Value(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionRole {
    ContainedBy(ValueId),
    Escapes,
    ObservedOnly,
}

pub(super) struct PointerResult {
    pub(super) owner: AnalysisOwner,
    pub(super) points_to: FxHashMap<ValueId, PointsToSet>,
    pub(super) objects: Vec<AbstractObject>,
    pub(super) escaping_objects: SparseBitSet,
}

pub(super) struct PointerSolver<'a> {
    module: &'a JsModuleIr,
    function_id: FunctionId,
    function: &'a JsFunctionIr,
    owner: AnalysisOwner,

    // Copy edges carry points-to facts forward. The same edge is traversed in
    // reverse when escape reachability is propagated.
    copy_edges: FxHashMap<PointerNode, FxHashSet<PointerNode>>,
    escape_dependencies: FxHashMap<PointerNode, FxHashSet<PointerNode>>,
    facts: FxHashMap<PointerNode, PointsToSet>,
    pending_facts: FxHashMap<PointerNode, PointsToSet>,
    fact_worklist: VecDeque<PointerNode>,
    queued_facts: FxHashSet<PointerNode>,

    escaping_nodes: FxHashSet<PointerNode>,
    escape_worklist: VecDeque<PointerNode>,

    objects: Vec<AbstractObject>,
    arguments_object: Option<AbstractObjectId>,
    import_meta_object: Option<AbstractObjectId>,
}

impl<'a> PointerSolver<'a> {
    pub(super) fn analyze(
        module: &'a JsModuleIr,
        function_id: FunctionId,
        function: &'a JsFunctionIr,
    ) -> PointerResult {
        let owner = AnalysisOwner::fresh(function_id);
        let mut solver = Self {
            module,
            function_id,
            function,
            owner,
            copy_edges: FxHashMap::default(),
            escape_dependencies: FxHashMap::default(),
            facts: FxHashMap::default(),
            pending_facts: FxHashMap::default(),
            fact_worklist: VecDeque::new(),
            queued_facts: FxHashSet::default(),
            escaping_nodes: FxHashSet::default(),
            escape_worklist: VecDeque::new(),
            objects: Vec::new(),
            arguments_object: None,
            import_meta_object: None,
        };

        solver.initialize_boundary_facts();
        solver.add_block_parameter_flow();
        solver.add_operation_flow();
        solver.add_region_flow();
        solver.solve_points_to();
        solver.propagate_escape();
        solver.finish()
    }

    fn initialize_boundary_facts(&mut self) {
        for (value, _) in self.function.values() {
            let owner = self.owner;
            self.facts
                .entry(PointerNode::Value(value))
                .or_insert_with(|| PointsToSet::bottom(owner));
        }

        for parameter in self.function.parameters() {
            self.add_fact(
                PointerNode::Value(parameter.value()),
                PointsToSet::unknown(self.owner),
            );
        }

        if let Some(binding) = self.function.self_binding() {
            self.add_fact(
                PointerNode::Binding(binding),
                PointsToSet::unknown_object(self.owner),
            );
        }

        for (region_id, _) in self.function.regions() {
            for (_, block) in self.function.region_blocks(region_id) {
                for parameter in block.parameters() {
                    if parameter.source() != BlockParameterSource::Forwarded {
                        self.add_fact(
                            PointerNode::Value(parameter.value()),
                            PointsToSet::unknown(self.owner),
                        );
                    }
                }
            }
        }

        for (binding, data) in self.module.bindings() {
            let fact = if data.declaring_function() != self.function_id
                || matches!(
                    data.kind(),
                    BindingKind::Import | BindingKind::Parameter | BindingKind::Catch
                ) {
                PointsToSet::unknown(self.owner)
            } else {
                PointsToSet::bottom(self.owner)
            };
            self.add_fact(PointerNode::Binding(binding), fact);
        }
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
                    self.add_copy(
                        PointerNode::Value(*argument),
                        PointerNode::Value(parameter.value()),
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
                | OperationKind::TemplateLiteral(_)
                | OperationKind::HasPrivateName(_)
                | OperationKind::IsNullish(_)
                | OperationKind::Typeof(_)
                | OperationKind::Delete(_)
                | OperationKind::Unary(_)
                | OperationKind::Update(_)
                | OperationKind::Binary(_) => {
                    self.add_results(&results, PointsToSet::primitive(self.owner));
                }

                OperationKind::RegExpLiteral(_)
                | OperationKind::ArrayLiteral(_)
                | OperationKind::ObjectLiteral(_)
                | OperationKind::CreateFunction(_)
                | OperationKind::CreateClass(_)
                | OperationKind::DynamicImport(_) => {
                    let [result] = results.as_slice() else {
                        unreachable!("allocation operations must have one result")
                    };
                    let object = self.allocate_object(AbstractObjectKind::Allocation(operation_id));
                    self.add_fact(PointerNode::Value(*result), PointsToSet::singleton(object));
                }

                OperationKind::LoadArguments(_) => {
                    let [result] = results.as_slice() else {
                        unreachable!("arguments loads must have one result")
                    };
                    let object = self.arguments_object();
                    self.add_fact(PointerNode::Value(*result), PointsToSet::singleton(object));

                    for (binding, data) in self.module.bindings() {
                        if data.declaring_function() == self.function_id
                            && data.kind() == BindingKind::Parameter
                        {
                            self.add_containment(
                                PointerNode::Value(*result),
                                PointerNode::Binding(binding),
                            );
                        }
                    }
                }

                OperationKind::MetaProperty(property)
                    if property.kind() == MetaPropertyKind::ImportMeta =>
                {
                    let [result] = results.as_slice() else {
                        unreachable!("meta-property loads must have one result")
                    };
                    let object = self.import_meta_object();
                    self.add_fact(PointerNode::Value(*result), PointsToSet::singleton(object));
                    // `import.meta` is already reachable outside this activation.
                    self.seed_escape(PointerNode::Value(*result));
                }

                OperationKind::Construct(_) | OperationKind::SuperCall(_) => {
                    self.add_results(&results, PointsToSet::unknown_object(self.owner));
                    self.seed_values(operands);
                }

                OperationKind::LoadThis(_)
                | OperationKind::MetaProperty(_)
                | OperationKind::LoadGlobal(_)
                | OperationKind::LoadProperty(_)
                | OperationKind::LoadSuperProperty(_)
                | OperationKind::Await(_)
                | OperationKind::Yield(_)
                | OperationKind::Call(_)
                | OperationKind::TaggedTemplate(_)
                | OperationKind::JsxElement(_)
                | OperationKind::JsxFragment(_) => {
                    self.add_results(&results, PointsToSet::unknown(self.owner));
                }

                OperationKind::LoadBinding(binding) => {
                    let [result] = results.as_slice() else {
                        unreachable!("binding loads must have one result")
                    };
                    self.add_copy(
                        PointerNode::Binding(binding.binding()),
                        PointerNode::Value(*result),
                    );
                }

                OperationKind::InitializeBinding(binding) => {
                    self.add_binding_store(binding.binding(), operands[0]);
                }
                OperationKind::StoreBinding(binding) => {
                    self.add_binding_store(binding.binding(), operands[0]);
                }

                OperationKind::Debugger(_)
                | OperationKind::DestructureBinding(_)
                | OperationKind::DestructureAssignment(_)
                | OperationKind::StoreGlobal(_)
                | OperationKind::StoreProperty(_)
                | OperationKind::StoreSuperProperty(_)
                | OperationKind::Jump(_)
                | OperationKind::If(_)
                | OperationKind::Try(_)
                | OperationKind::While(_)
                | OperationKind::DoWhile(_)
                | OperationKind::For(_)
                | OperationKind::ForIn(_)
                | OperationKind::ForOf(_)
                | OperationKind::Switch(_)
                | OperationKind::EnterFinally(_)
                | OperationKind::ResumeCompletion(_)
                | OperationKind::RegionYield(_)
                | OperationKind::Return(_)
                | OperationKind::Throw(_) => {}

                OperationKind::Invoke(_) => unreachable!("invoke operation must be unwrapped"),
            }

            self.add_operation_escape_flow(operation_id);

            if matches!(
                kind,
                OperationKind::DestructureBinding(_)
                    | OperationKind::DestructureAssignment(_)
                    | OperationKind::For(_)
                    | OperationKind::ForIn(_)
                    | OperationKind::ForOf(_)
            ) {
                let mut written_bindings = Vec::new();
                kind.visit_referenced_bindings(|binding| written_bindings.push(binding));
                for binding in written_bindings {
                    self.add_fact(
                        PointerNode::Binding(binding),
                        PointsToSet::unknown(self.owner),
                    );
                }
            }
        }
    }

    fn add_operation_escape_flow(&mut self, operation_id: OperationId) {
        let operation = self
            .function
            .operation(operation_id)
            .expect("operation must be live");
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
            | OperationKind::LoadArguments(_)
            | OperationKind::MetaProperty(_)
            | OperationKind::LoadGlobal(_)
            | OperationKind::Jump(_)
            | OperationKind::Try(_)
            | OperationKind::For(_)
            | OperationKind::RegionYield(_)
            | OperationKind::InitializeBinding(_)
            | OperationKind::StoreBinding(_)
            | OperationKind::LoadBinding(_)
            | OperationKind::HasPrivateName(_)
            | OperationKind::IsNullish(_) => {}

            OperationKind::CreateFunction(_) => {
                self.add_captured_bindings(operation_id);
            }

            OperationKind::CreateClass(class) => {
                let result = results[0];
                self.add_captured_bindings(operation_id);

                if let Some(binding) = class.self_binding() {
                    self.add_copy(PointerNode::Value(result), PointerNode::Binding(binding));
                }

                if class.elements().iter().any(|element| {
                    matches!(element, ClassElement::Field(field)
                        if field.placement() == ClassFieldPlacement::Static
                            && field.initializer().is_some())
                        || matches!(element, ClassElement::StaticBlock(_))
                }) {
                    self.seed_escape(PointerNode::Value(result));
                }
            }

            OperationKind::DynamicImport(_) => {
                self.seed_values(operands);
                self.seed_escape(PointerNode::Value(results[0]));
            }

            OperationKind::DestructureBinding(_)
            | OperationKind::DestructureAssignment(_)
            | OperationKind::StoreGlobal(_)
            | OperationKind::LoadSuperProperty(_)
            | OperationKind::StoreSuperProperty(_)
            | OperationKind::LoadProperty(_)
            | OperationKind::StoreProperty(_)
            | OperationKind::Update(_)
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

            OperationKind::Await(_) | OperationKind::Yield(_) => {
                // A suspended activation is retained by its async or generator
                // continuation. Without operation-level liveness, conservatively
                // treat every local value and binding as continuation state.
                self.seed_activation_state();
            }

            OperationKind::Debugger(_) => self.seed_local_bindings(),

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
            | OperationKind::ResumeCompletion(_) => {}

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

    fn operation_results(&self, operation: &OperationData) -> Vec<ValueId> {
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
                            self.add_containment(
                                PointerNode::Value(container),
                                PointerNode::Value(value),
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
            match operation.kind() {
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

    fn add_binding_store(&mut self, binding: BindingId, value: ValueId) {
        self.add_copy(PointerNode::Value(value), PointerNode::Binding(binding));

        let data = self
            .module
            .binding(binding)
            .expect("operation must reference a live binding");
        if data.declaring_function() != self.function_id || self.binding_is_exported(binding) {
            self.seed_escape(PointerNode::Binding(binding));
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
                self.add_containment(PointerNode::Value(carrier), PointerNode::Binding(binding));
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
            self.seed_escape(PointerNode::Binding(binding));
        }
    }

    fn seed_activation_state(&mut self) {
        let values = self
            .function
            .values()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
        self.seed_values(&values);
        self.seed_local_bindings();
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

    fn allocate_object(&mut self, kind: AbstractObjectKind) -> AbstractObjectId {
        let id = AbstractObjectId::from_index(self.owner, self.objects.len());
        self.objects.push(AbstractObject::new(id, kind));
        id
    }

    fn arguments_object(&mut self) -> AbstractObjectId {
        if let Some(object) = self.arguments_object {
            return object;
        }

        let object = self.allocate_object(AbstractObjectKind::ArgumentsObject);
        self.arguments_object = Some(object);
        object
    }

    fn import_meta_object(&mut self) -> AbstractObjectId {
        if let Some(object) = self.import_meta_object {
            return object;
        }

        let object = self.allocate_object(AbstractObjectKind::ImportMeta);
        self.import_meta_object = Some(object);
        object
    }

    fn add_results(&mut self, results: &[ValueId], fact: PointsToSet) {
        for result in results {
            self.add_fact(PointerNode::Value(*result), fact.clone());
        }
    }

    fn add_fact(&mut self, node: PointerNode, fact: PointsToSet) {
        let owner = fact.owner();
        let delta = self
            .facts
            .entry(node)
            .or_insert_with(|| PointsToSet::bottom(owner))
            .join_delta(&fact);
        if delta.is_bottom() {
            return;
        }

        self.pending_facts
            .entry(node)
            .or_insert_with(|| PointsToSet::bottom(owner))
            .join_delta(&delta);
        self.queue_fact(node);
    }

    fn add_copy(&mut self, source: PointerNode, target: PointerNode) {
        self.copy_edges.entry(source).or_default().insert(target);
        self.add_escape_dependency(target, source);
    }

    fn add_containment(&mut self, container: PointerNode, contained: PointerNode) {
        self.add_escape_dependency(container, contained);
    }

    fn add_escape_dependency(&mut self, escaping: PointerNode, dependency: PointerNode) {
        self.escape_dependencies
            .entry(escaping)
            .or_default()
            .insert(dependency);
    }

    fn seed_values(&mut self, values: &[ValueId]) {
        for value in values {
            self.seed_escape(PointerNode::Value(*value));
        }
    }

    fn seed_escape(&mut self, node: PointerNode) {
        if self.escaping_nodes.insert(node) {
            self.escape_worklist.push_back(node);
        }
    }

    fn queue_fact(&mut self, node: PointerNode) {
        if self.queued_facts.insert(node) {
            self.fact_worklist.push_back(node);
        }
    }

    fn solve_points_to(&mut self) {
        while let Some(source) = self.fact_worklist.pop_front() {
            self.queued_facts.remove(&source);
            let delta = self
                .pending_facts
                .remove(&source)
                .unwrap_or_else(|| PointsToSet::bottom(self.owner));
            let targets = self
                .copy_edges
                .get(&source)
                .into_iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();

            for target in targets {
                self.add_fact(target, delta.clone());
            }
        }
    }

    fn propagate_escape(&mut self) {
        while let Some(node) = self.escape_worklist.pop_front() {
            let dependencies = self
                .escape_dependencies
                .get(&node)
                .into_iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            for dependency in dependencies {
                self.seed_escape(dependency);
            }
        }
    }

    fn finish(self) -> PointerResult {
        let points_to = self
            .function
            .values()
            .map(|(value, _)| {
                let fact = self
                    .facts
                    .get(&PointerNode::Value(value))
                    .cloned()
                    .unwrap_or_else(|| PointsToSet::bottom(self.owner));
                (value, fact)
            })
            .collect();

        let mut escaping_objects = SparseBitSet::new();
        for points_to in self
            .escaping_nodes
            .iter()
            .filter_map(|node| self.facts.get(node))
        {
            points_to.union_objects_into(&mut escaping_objects);
        }

        PointerResult {
            owner: self.owner,
            points_to,
            objects: self.objects,
            escaping_objects,
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
