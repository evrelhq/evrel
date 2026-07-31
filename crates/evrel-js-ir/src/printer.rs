//! Canonical textual formatting for Evrel IR.

use crate::{
    ArrayLiteralElement, AssignmentPattern, AssignmentTarget, BasicBlockData, BinaryOperator,
    BindingData, BindingId, BindingKind, BindingPattern, BindingWriteMode, BlockId, BlockParameter,
    BlockParameterSource, BlockTarget, CallArgument, CallReceiver, CallTarget, ClassElement,
    ClassElementKey, ClassFieldPlacement, ClassMethodKind, ClassMethodPlacement, ConstantValue,
    DeleteTarget, DynamicImportPhase, ExceptionHandlerData, ExceptionHandlerId,
    ExceptionHandlerKind, FunctionId, FunctionKind, FunctionMode, FunctionParameter,
    FunctionParameterKind, JsFunctionIr, JsModuleIr, JsxAttribute, JsxAttributeName,
    JsxAttributeValue, JsxChild, JsxElementName, JsxMemberBase, LabeledStatementData,
    LabeledStatementId, MetaPropertyKind, ObjectAssignmentProperty, ObjectBindingProperty,
    ObjectLiteralEntry, ObjectLiteralKey, ObjectMethodKind, OperationData, OperationKind,
    PrivateNameData, PrivateNameId, PropertyKey, RegionId, RegionOwner, SuperPropertyKey,
    TypeofTarget, UnaryOperator, UnwindTarget, UpdateOperator, ValueId, YieldKind,
};

/// Formats a module using Evrel's canonical textual IR representation.
pub fn print_module(module: &JsModuleIr) -> String {
    ModulePrinter::new(module).print()
}

/// Formats a function using Evrel's canonical textual IR representation.
pub fn print_function(function: &JsFunctionIr) -> String {
    FunctionPrinter::new(function).print()
}

struct ModulePrinter<'ir> {
    module: &'ir JsModuleIr,
}

impl<'ir> ModulePrinter<'ir> {
    fn new(module: &'ir JsModuleIr) -> Self {
        Self { module }
    }

    fn print(self) -> String {
        let mut sections = Vec::new();
        let imports = self
            .module
            .imports()
            .iter()
            .map(|import| {
                let declaration = match import {
                    crate::ModuleImport::Bare { source, .. } => format!("import {source:?}"),
                    crate::ModuleImport::Default {
                        source, binding, ..
                    } => {
                        format!("import default @{} from {source:?}", binding.index())
                    }
                    crate::ModuleImport::Namespace {
                        source, binding, ..
                    } => {
                        format!("import namespace @{} from {source:?}", binding.index())
                    }
                    crate::ModuleImport::Named {
                        source,
                        imported,
                        binding,
                        ..
                    } => {
                        let imported = print_module_export_name(imported);

                        format!(
                            "import {{ {imported} as @{} }} from {source:?}",
                            binding.index()
                        )
                    }
                };
                let attributes = print_module_attributes(import.attributes());

                format!("  {declaration}{attributes}")
            })
            .collect::<Vec<_>>();
        let exports = self
            .module
            .exports()
            .iter()
            .map(|export| match export {
                crate::ModuleExport::Empty {
                    source, attributes, ..
                } => {
                    let attributes = print_module_attributes(attributes);

                    format!("  export {{}} from {source:?}{attributes}")
                }

                crate::ModuleExport::Local {
                    exported, binding, ..
                } => {
                    format!(
                        "  export @{} as {}",
                        binding.index(),
                        print_module_export_name(exported),
                    )
                }

                crate::ModuleExport::Indirect {
                    source,
                    attributes,
                    imported,
                    exported,
                    ..
                } => {
                    let imported = print_module_export_name(imported);
                    let exported = print_module_export_name(exported);
                    let attributes = print_module_attributes(attributes);

                    format!("  export {{ {imported} as {exported} }} from {source:?}{attributes}")
                }

                crate::ModuleExport::Namespace {
                    source,
                    attributes,
                    exported,
                    ..
                } => {
                    let exported = print_module_export_name(exported);
                    let attributes = print_module_attributes(attributes);

                    format!("  export * as {exported} from {source:?}{attributes}")
                }

                crate::ModuleExport::Star {
                    source, attributes, ..
                } => {
                    let attributes = print_module_attributes(attributes);

                    format!("  export * from {source:?}{attributes}")
                }
            })
            .collect::<Vec<_>>();
        let bindings = self
            .module
            .bindings()
            .map(|(id, binding)| self.print_binding(id, binding))
            .collect::<Vec<_>>();
        let private_names = self
            .module
            .private_names()
            .map(|(id, private_name)| self.print_private_name(id, private_name))
            .collect::<Vec<_>>();

        if !imports.is_empty() {
            sections.push(imports.join("\n"));
        }

        if !exports.is_empty() {
            sections.push(exports.join("\n"));
        }

        if !bindings.is_empty() {
            sections.push(bindings.join("\n"));
        }

        if !private_names.is_empty() {
            sections.push(private_names.join("\n"));
        }

        for (id, function) in self.module.functions() {
            sections.push(self.print_module_function(id, function));
        }

        format!("module {{\n{}\n}}", sections.join("\n\n"))
    }

    fn print_binding(&self, id: BindingId, binding: &BindingData) -> String {
        let kind = match binding.kind() {
            BindingKind::Const => "const",
            BindingKind::Let => "let",
            BindingKind::Class => "class",
            BindingKind::Var => "var",
            BindingKind::Function => "function",
            BindingKind::Import => "import",
            BindingKind::Parameter => "parameter",
            BindingKind::Catch => "catch",
        };

        format!("  binding @{} {kind} {:?}", id.index(), binding.name())
    }

    fn print_private_name(&self, id: PrivateNameId, private_name: &PrivateNameData) -> String {
        format!("  private_name @{} {:?}", id.index(), private_name.name(),)
    }

    fn print_module_function(&self, id: FunctionId, function: &JsFunctionIr) -> String {
        let kind = match function.kind() {
            FunctionKind::Module => "entry",
            FunctionKind::Ordinary => "ordinary",
            FunctionKind::Arrow => "arrow",
            FunctionKind::ObjectMethod => "object_method",
            FunctionKind::ClassConstructor => "class_constructor",
            FunctionKind::ClassMethod => "class_method",
            FunctionKind::ClassFieldInitializer => "class_field_initializer",
            FunctionKind::ClassStaticBlock => "class_static_block",
        };
        let mode = match function.mode() {
            FunctionMode::Normal => "",
            FunctionMode::Async => " async",
            FunctionMode::Generator => " generator",
            FunctionMode::AsyncGenerator => " async_generator",
        };
        let body = FunctionPrinter::new(function)
            .print()
            .lines()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n");

        format!("  function @{} {kind}{mode} {{\n{body}\n  }}", id.index())
    }
}

struct FunctionPrinter<'ir> {
    function: &'ir JsFunctionIr,
    lines: Vec<String>,
}

impl<'ir> FunctionPrinter<'ir> {
    fn new(function: &'ir JsFunctionIr) -> Self {
        Self {
            function,
            lines: Vec::new(),
        }
    }

    fn print(mut self) -> String {
        let has_boundary_metadata =
            self.function.self_binding().is_some() || !self.function.parameters().is_empty();

        if let Some(binding) = self.function.self_binding() {
            self.lines
                .push(format!("self_binding @{}", binding.index()));
        }

        for parameter in self.function.parameters() {
            self.lines.push(print_function_parameter(parameter));
        }

        if has_boundary_metadata {
            self.lines.push(String::new());
        }

        let handlers = self
            .function
            .exception_handlers()
            .map(|(id, data)| self.print_exception_handler(id, data))
            .collect::<Vec<_>>();

        if !handlers.is_empty() {
            self.lines.extend(handlers);
            self.lines.push(String::new());
        }

        let labeled_statements = self
            .function
            .labeled_statements()
            .map(|(id, data)| self.print_labeled_statement(id, data))
            .collect::<Vec<_>>();

        if !labeled_statements.is_empty() {
            self.lines.extend(labeled_statements);
            self.lines.push(String::new());
        }

        let regions = self
            .function
            .regions()
            .map(|(id, _)| id)
            .filter(|id| *id != self.function.body_region())
            .collect::<Vec<_>>();

        for region in regions {
            self.print_region(region);
            self.lines.push(String::new());
        }

        for (index, (block_id, block)) in self.function.blocks().enumerate() {
            if index > 0 {
                self.lines.push(String::new());
            }

            self.print_block(block_id, block);
        }

        self.lines.join("\n")
    }

    fn print_region(&mut self, id: RegionId) {
        let data = self
            .function
            .region(id)
            .expect("region list must reference a live region");
        let mut fields = vec![format!("results: {}", data.result_count())];

        if let Some(parent) = data.parent() {
            fields.push(format!("parent: region @{}", parent.index()));
        }

        if let Some(owner) = data.owner() {
            fields.push(match owner {
                RegionOwner::FunctionBody => "owner: function".to_owned(),
                RegionOwner::Operation(operation) => {
                    format!("owner: op @{}", operation.index())
                }
                RegionOwner::FunctionParameter { parameter_index } => {
                    format!("owner: param {parameter_index}")
                }
            });
        }

        self.lines
            .push(format!("region @{} {}", id.index(), fields.join(", ")));

        let blocks = data.blocks().to_vec();
        for block_id in blocks {
            let block = self
                .function
                .block(block_id)
                .expect("region must reference a live block");
            self.print_block(block_id, block);
        }
    }

    fn print_exception_handler(
        &self,
        id: ExceptionHandlerId,
        data: &ExceptionHandlerData,
    ) -> String {
        let kind = match data.kind() {
            ExceptionHandlerKind::Catch => "catch",
            ExceptionHandlerKind::Finally => "finally",
        };
        let mut fields = Vec::new();

        if let Some(parent) = data.parent() {
            fields.push(format!("parent: @{}", parent.index()));
        }

        fields.push(format!("entry: bb{}", data.entry_block().index()));

        format!("handler @{} {kind} {}", id.index(), fields.join(", "))
    }

    fn print_labeled_statement(
        &self,
        id: LabeledStatementId,
        data: &LabeledStatementData,
    ) -> String {
        format!(
            "labeled @{} labels: {}, body: bb{}, completion: bb{}",
            id.index(),
            print_labels(data.labels()),
            data.body_block().index(),
            data.completion_block().index(),
        )
    }

    fn print_block(&mut self, block_id: BlockId, block: &BasicBlockData) {
        let parameters = block
            .parameters()
            .iter()
            .map(print_block_parameter)
            .collect::<Vec<_>>()
            .join(", ");
        if parameters.is_empty() {
            self.lines.push(format!("bb{}:", block_id.index()));
        } else {
            self.lines
                .push(format!("bb{}({parameters}):", block_id.index()));
        }

        for operation_id in block.operations() {
            let operation = self
                .function
                .operation(*operation_id)
                .expect("block must reference a live operation");

            self.lines
                .push(format!("  {}", self.print_operation(operation)));
        }

        if let Some(terminator) = block.terminator() {
            let operation = self
                .function
                .operation(terminator)
                .expect("block must reference a live terminator");

            self.lines
                .push(format!("  {}", self.print_operation(operation)));
        }
    }

    fn print_operation(&self, operation: &OperationData) -> String {
        let results = operation
            .results()
            .iter()
            .copied()
            .map(print_value)
            .collect::<Vec<_>>()
            .join(", ");
        let result_prefix = if results.is_empty() {
            String::new()
        } else {
            format!("{results} = ")
        };

        let body = match operation.kind() {
            OperationKind::Constant(operation) => match operation.value() {
                ConstantValue::Undefined => "constant undefined".to_owned(),
                ConstantValue::Boolean(value) => format!("constant {value}"),
                ConstantValue::Null => "constant null".to_owned(),
                ConstantValue::Number(value) => format!("constant {value}"),
                ConstantValue::BigInt(value) => format!("constant {value}n"),
                ConstantValue::String(value) => format!("constant {:?}", value.as_str()),
            },

            OperationKind::RegExpLiteral(regexp) => {
                format!("regexp /{}/{}", regexp.pattern(), regexp.flags())
            }

            OperationKind::TemplateLiteral(template) => {
                let mut components = Vec::with_capacity(template.quasis().len() * 2 - 1);

                for (index, quasi) in template.quasis().iter().enumerate() {
                    components.push(format!(
                        "{:?}",
                        quasi
                            .cooked()
                            .expect("untagged template quasis must be cooked")
                            .as_str(),
                    ));

                    if let Some(substitution) = template.substitutions().get(index) {
                        components.push(format!("region @{}", substitution.index()));
                    }
                }

                format!("template [{}]", components.join(", "))
            }

            OperationKind::TaggedTemplate(template) => {
                let operands = operation.operands();
                let (target, receiver) = print_call_target(template.target(), operands);
                let target = match receiver {
                    Some(receiver) => {
                        format!("{target}, receiver: {}", print_value(receiver))
                    }
                    None => target,
                };
                let quasis = template
                    .quasis()
                    .iter()
                    .map(|quasi| {
                        let cooked = quasi
                            .cooked()
                            .map(|value| format!("{:?}", value.as_str()))
                            .unwrap_or_else(|| "undefined".to_owned());

                        format!("{{raw: {:?}, cooked: {cooked}}}", quasi.raw())
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let substitutions = template
                    .substitutions()
                    .iter()
                    .map(|region| format!("region @{}", region.index()))
                    .collect::<Vec<_>>()
                    .join(", ");

                format!(
                    "tagged_template site @{}, target: {target}, quasis: [{quasis}], substitutions: [{substitutions}]",
                    template.site().index(),
                )
            }

            OperationKind::ArrayLiteral(array) => {
                let elements = array
                    .elements()
                    .iter()
                    .map(|element| match element {
                        ArrayLiteralElement::Value { expression } => {
                            format!("region @{}", expression.index())
                        }
                        ArrayLiteralElement::Spread { expression } => {
                            format!("...region @{}", expression.index())
                        }
                        ArrayLiteralElement::Elision => "_".to_owned(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("array_literal [{elements}]")
            }

            OperationKind::ObjectLiteral(object) => {
                let entries = object
                    .entries()
                    .iter()
                    .map(|entry| match entry {
                        ObjectLiteralEntry::Property {
                            key: ObjectLiteralKey::Static(name),
                            value,
                        } => format!("{name:?}: region @{}", value.index()),
                        ObjectLiteralEntry::Property {
                            key: ObjectLiteralKey::Computed { expression },
                            value,
                        } => format!(
                            "[region @{}]: region @{}",
                            expression.index(),
                            value.index()
                        ),
                        ObjectLiteralEntry::Method {
                            kind,
                            key,
                            function,
                        } => {
                            let kind = match kind {
                                ObjectMethodKind::Method => "method",
                                ObjectMethodKind::Getter => "get",
                                ObjectMethodKind::Setter => "set",
                            };
                            let key = match key {
                                ObjectLiteralKey::Static(name) => format!("{name:?}"),
                                ObjectLiteralKey::Computed { expression } => {
                                    format!("[region @{}]", expression.index())
                                }
                            };

                            format!("{kind} {key}: function @{}", function.index())
                        }
                        ObjectLiteralEntry::Spread { expression } => {
                            format!("...region @{}", expression.index())
                        }
                        ObjectLiteralEntry::Prototype { expression } => {
                            format!("__proto__: region @{}", expression.index())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("object_literal {{{entries}}}")
            }

            OperationKind::JsxElement(element) => {
                let name = print_jsx_element_name(element.name(), operation.operands());
                let attributes = element
                    .attributes()
                    .iter()
                    .map(print_jsx_attribute)
                    .collect::<Vec<_>>()
                    .join(", ");
                let children = element
                    .children()
                    .iter()
                    .map(print_jsx_child)
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("jsx_element {name}, attributes: [{attributes}], children: [{children}]")
            }

            OperationKind::JsxFragment(fragment) => {
                let children = fragment
                    .children()
                    .iter()
                    .map(print_jsx_child)
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("jsx_fragment children: [{children}]")
            }

            OperationKind::CreateFunction(operation) => {
                format!("create_function @{}", operation.function().index())
            }

            OperationKind::CreateClass(class) => {
                let self_binding = class
                    .self_binding()
                    .map(|binding| format!("@{}", binding.index()))
                    .unwrap_or_else(|| "none".to_owned());
                let super_class = class
                    .super_class()
                    .map(|region| format!("region @{}", region.index()))
                    .unwrap_or_else(|| "none".to_owned());
                let elements = class
                    .elements()
                    .iter()
                    .map(print_class_element)
                    .collect::<Vec<_>>()
                    .join(", ");

                format!(
                    "create_class self: {self_binding}, super: {super_class}, elements: [{elements}]"
                )
            }

            OperationKind::LoadThis(_) => "load_this".to_owned(),

            OperationKind::LoadArguments(_) => "load_arguments".to_owned(),

            OperationKind::MetaProperty(property) => match property.kind() {
                MetaPropertyKind::ImportMeta => "import.meta".to_owned(),
                MetaPropertyKind::NewTarget => "new.target".to_owned(),
            },

            OperationKind::DynamicImport(import) => {
                let phase = match import.phase() {
                    DynamicImportPhase::Evaluation => "dynamic_import",
                    DynamicImportPhase::Source => "dynamic_import.source",
                    DynamicImportPhase::Defer => "dynamic_import.defer",
                };

                match operation.operands() {
                    [source] if !import.has_options() => {
                        format!("{phase} {}", print_value(*source))
                    }
                    [source, options] if import.has_options() => format!(
                        "{phase} {}, options: {}",
                        print_value(*source),
                        print_value(*options),
                    ),
                    _ => unreachable!("dynamic import operand layout must match its options"),
                }
            }

            OperationKind::Debugger(_) => "debugger".to_owned(),

            OperationKind::InitializeBinding(binding) => {
                let [value] = operation.operands() else {
                    unreachable!("binding initialization has one operand");
                };

                format!(
                    "initialize_binding @{}, {}",
                    binding.binding().index(),
                    print_value(*value),
                )
            }

            OperationKind::DestructureBinding(destructure) => {
                let [source] = operation.operands() else {
                    unreachable!("binding destructuring has one source operand");
                };
                let mode = match destructure.mode() {
                    BindingWriteMode::Initialize => "initialize",
                    BindingWriteMode::Store => "store",
                };

                format!(
                    "destructure_binding.{mode} {}, {}",
                    print_binding_pattern(destructure.pattern()),
                    print_value(*source),
                )
            }

            OperationKind::DestructureAssignment(destructure) => {
                let [source] = operation.operands() else {
                    unreachable!("assignment destructuring has one source operand");
                };

                format!(
                    "destructure_assignment {}, {}",
                    print_assignment_pattern(destructure.pattern()),
                    print_value(*source),
                )
            }

            OperationKind::LoadBinding(binding) => {
                format!("load_binding @{}", binding.binding().index())
            }

            OperationKind::StoreBinding(binding) => {
                let [value] = operation.operands() else {
                    unreachable!("binding stores have one operand");
                };

                format!(
                    "store_binding @{}, {}",
                    binding.binding().index(),
                    print_value(*value),
                )
            }

            OperationKind::LoadGlobal(operation) => {
                format!("load_global {:?}", operation.name())
            }

            OperationKind::StoreGlobal(global) => {
                let [value] = operation.operands() else {
                    unreachable!("global stores have one operand");
                };

                format!("store_global {:?}, {}", global.name(), print_value(*value),)
            }

            OperationKind::LoadProperty(property) => match property.key() {
                PropertyKey::Static(name) => {
                    let [object] = operation.operands() else {
                        unreachable!("static property reads have one operand");
                    };

                    format!("load_property {}, {:?}", print_value(*object), name)
                }

                PropertyKey::Computed => {
                    let [object, key] = operation.operands() else {
                        unreachable!("computed property reads have two operands");
                    };

                    format!(
                        "load_property {}, {}",
                        print_value(*object),
                        print_value(*key),
                    )
                }

                PropertyKey::Private(private_name) => {
                    let [object] = operation.operands() else {
                        unreachable!("private property reads have one operand");
                    };

                    format!(
                        "load_property {}, private @{}",
                        print_value(*object),
                        private_name.index(),
                    )
                }
            },

            OperationKind::LoadSuperProperty(property) => match property.key() {
                SuperPropertyKey::Static(name) => {
                    format!("load_super_property {name:?}")
                }

                SuperPropertyKey::Computed => {
                    let [key] = operation.operands() else {
                        unreachable!("computed super property reads have one operand");
                    };

                    format!("load_super_property {}", print_value(*key))
                }
            },

            OperationKind::StoreProperty(property) => match property.key() {
                PropertyKey::Static(name) => {
                    let [object, value] = operation.operands() else {
                        unreachable!("static property stores have two operands");
                    };

                    format!(
                        "store_property {}, {name:?}, {}",
                        print_value(*object),
                        print_value(*value),
                    )
                }
                PropertyKey::Computed => {
                    let [object, key, value] = operation.operands() else {
                        unreachable!("computed property stores have three operands");
                    };

                    format!(
                        "store_property {}, {}, {}",
                        print_value(*object),
                        print_value(*key),
                        print_value(*value),
                    )
                }

                PropertyKey::Private(private_name) => {
                    let [object, value] = operation.operands() else {
                        unreachable!("private property stores have two operands");
                    };

                    format!(
                        "store_property {}, private @{}, {}",
                        print_value(*object),
                        private_name.index(),
                        print_value(*value),
                    )
                }
            },

            OperationKind::StoreSuperProperty(property) => match property.key() {
                SuperPropertyKey::Static(name) => {
                    let [value] = operation.operands() else {
                        unreachable!("static super stores have one operand");
                    };

                    format!("store_super_property {name:?}, {}", print_value(*value))
                }

                SuperPropertyKey::Computed => {
                    let [key, value] = operation.operands() else {
                        unreachable!("computed super stores have two operands");
                    };

                    format!(
                        "store_super_property {}, {}",
                        print_value(*key),
                        print_value(*value),
                    )
                }
            },

            OperationKind::HasPrivateName(predicate) => {
                let [object] = operation.operands() else {
                    unreachable!("private-name checks have one operand");
                };

                format!(
                    "has_private_name {}, private @{}",
                    print_value(*object),
                    predicate.private_name().index(),
                )
            }

            OperationKind::IsNullish(_) => {
                let [value] = operation.operands() else {
                    unreachable!("nullish predicates have one operand");
                };

                format!("is_nullish {}", print_value(*value))
            }

            OperationKind::Typeof(type_of) => match type_of.target() {
                TypeofTarget::Value => {
                    let [value] = operation.operands() else {
                        unreachable!("value typeof operations have one operand");
                    };

                    format!("typeof {}", print_value(*value))
                }
                TypeofTarget::Global(name) => {
                    assert!(
                        operation.operands().is_empty(),
                        "global typeof operations have no operands",
                    );

                    format!("typeof_global {name:?}")
                }
            },

            OperationKind::Delete(delete) => match delete.target() {
                DeleteTarget::Value => {
                    let [value] = operation.operands() else {
                        unreachable!("value delete operations have one operand");
                    };

                    format!("delete_value {}", print_value(*value))
                }
                DeleteTarget::Property(PropertyKey::Static(name)) => {
                    let [object] = operation.operands() else {
                        unreachable!("static property deletes have one operand");
                    };

                    format!("delete_property {}, {name:?}", print_value(*object))
                }
                DeleteTarget::Property(PropertyKey::Computed) => {
                    let [object, key] = operation.operands() else {
                        unreachable!("computed property deletes have two operands");
                    };

                    format!(
                        "delete_property {}, {}",
                        print_value(*object),
                        print_value(*key),
                    )
                }
                DeleteTarget::Property(PropertyKey::Private(_)) => {
                    unreachable!("private properties cannot be deleted")
                }
            },

            OperationKind::Unary(unary) => {
                let operator = match unary.operator() {
                    UnaryOperator::Plus => "plus",
                    UnaryOperator::Negate => "negate",
                    UnaryOperator::BitwiseNot => "bitwise_not",
                    UnaryOperator::LogicalNot => "not",
                    UnaryOperator::Void => "void",
                };
                let [value] = operation.operands() else {
                    unreachable!("unary operations have one operand");
                };

                format!("unary.{operator} {}", print_value(*value))
            }

            OperationKind::Update(update) => {
                let operator = match update.operator() {
                    UpdateOperator::Increment => "increment",
                    UpdateOperator::Decrement => "decrement",
                };
                let [value] = operation.operands() else {
                    unreachable!("update operations have one operand");
                };

                format!("update.{operator} {}", print_value(*value))
            }

            OperationKind::Binary(binary) => {
                let operator = match binary.operator() {
                    BinaryOperator::Add => "add",
                    BinaryOperator::Subtract => "subtract",
                    BinaryOperator::Multiply => "multiply",
                    BinaryOperator::Divide => "divide",
                    BinaryOperator::Remainder => "remainder",
                    BinaryOperator::Exponentiate => "exponentiate",
                    BinaryOperator::LooseEqual => "loose_equal",
                    BinaryOperator::LooseNotEqual => "loose_not_equal",
                    BinaryOperator::StrictEqual => "strict_equal",
                    BinaryOperator::StrictNotEqual => "strict_not_equal",
                    BinaryOperator::LessThan => "less_than",
                    BinaryOperator::LessThanOrEqual => "less_than_or_equal",
                    BinaryOperator::GreaterThan => "greater_than",
                    BinaryOperator::GreaterThanOrEqual => "greater_than_or_equal",
                    BinaryOperator::In => "in",
                    BinaryOperator::InstanceOf => "instance_of",
                    BinaryOperator::ShiftLeft => "shift_left",
                    BinaryOperator::ShiftRight => "shift_right",
                    BinaryOperator::UnsignedShiftRight => "unsigned_shift_right",
                    BinaryOperator::BitwiseOr => "bitwise_or",
                    BinaryOperator::BitwiseXor => "bitwise_xor",
                    BinaryOperator::BitwiseAnd => "bitwise_and",
                };
                let operands = operation
                    .operands()
                    .iter()
                    .copied()
                    .map(print_value)
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("binary.{operator} {operands}")
            }

            OperationKind::Await(_) => {
                let [value] = operation.operands() else {
                    unreachable!("await operations have one operand");
                };

                format!("await {}", print_value(*value))
            }

            OperationKind::Yield(yield_operation) => {
                let [value] = operation.operands() else {
                    unreachable!("yield operations have one operand");
                };
                let operator = match yield_operation.kind() {
                    YieldKind::Value => "yield",
                    YieldKind::Delegate => "yield*",
                };

                format!("{operator} {}", print_value(*value))
            }

            OperationKind::Call(call) => {
                let operands = operation.operands();
                let (target, receiver) = print_call_target(call.target(), operands);
                let receiver = receiver
                    .map(|receiver| format!("receiver: {}, ", print_value(receiver)))
                    .unwrap_or_default();
                let arguments = print_call_arguments(call.arguments());

                format!("call {target}, {receiver}args: [{arguments}]")
            }

            OperationKind::SuperCall(call) => {
                let arguments = print_call_arguments(call.arguments());

                format!("super_call args: [{arguments}]")
            }

            OperationKind::Construct(construct) => {
                let [constructor] = operation.operands() else {
                    unreachable!("construct operations have a constructor operand");
                };
                let arguments = print_call_arguments(construct.arguments());

                format!(
                    "construct {}, args: [{arguments}]",
                    print_value(*constructor),
                )
            }

            OperationKind::Jump(jump) => {
                format!(
                    "jump {}",
                    print_block_target(jump.target(), operation.operands())
                )
            }

            OperationKind::If(if_operation) => {
                let (condition, successor_arguments) = operation
                    .operands()
                    .split_first()
                    .expect("if operations must have a condition operand");
                let then_argument_count = if_operation.then_target().argument_count();
                let (then_arguments, else_arguments) =
                    successor_arguments.split_at(then_argument_count);

                format!(
                    "if {}, then: {}, else: {}, completion: bb{}",
                    print_value(*condition),
                    print_block_target(if_operation.then_target(), then_arguments),
                    print_block_target(if_operation.else_target(), else_arguments),
                    if_operation.completion_block().index(),
                )
            }

            OperationKind::Try(try_operation) => {
                let catch = try_operation
                    .catch_block()
                    .map(|block| format!("bb{}", block.index()))
                    .unwrap_or_else(|| "none".into());
                let finally = try_operation
                    .finally_block()
                    .map(|block| format!("bb{}", block.index()))
                    .unwrap_or_else(|| "none".into());

                format!(
                    "try body: {}, catch: {catch}, finally: {finally}, completion: bb{}",
                    print_block_target(try_operation.try_target(), operation.operands()),
                    try_operation.completion_block().index(),
                )
            }

            OperationKind::While(while_operation) => {
                let successors = while_operation.successors();
                let [body, exit] = successors.as_slice() else {
                    unreachable!("while operations must have two successors");
                };
                let condition = operation
                    .operands()
                    .first()
                    .copied()
                    .expect("while operations must have a condition operand");
                let labels = if while_operation.labels().is_empty() {
                    String::new()
                } else {
                    format!(", labels: {}", print_labels(while_operation.labels()))
                };

                format!(
                    "while {}, body: {}, exit: {}{labels}",
                    print_value(condition),
                    print_block_target(body.target(), body.arguments(operation.operands())),
                    print_block_target(exit.target(), exit.arguments(operation.operands())),
                )
            }

            OperationKind::DoWhile(do_while_operation) => {
                let successors = do_while_operation.successors();
                let [body, exit] = successors.as_slice() else {
                    unreachable!("do-while operations must have two successors");
                };
                let condition = operation
                    .operands()
                    .first()
                    .copied()
                    .expect("do-while operations must have a condition operand");
                let labels = if do_while_operation.labels().is_empty() {
                    String::new()
                } else {
                    format!(", labels: {}", print_labels(do_while_operation.labels()))
                };

                format!(
                    "do_while {}, body: {}, exit: {}{labels}",
                    print_value(condition),
                    print_block_target(body.target(), body.arguments(operation.operands())),
                    print_block_target(exit.target(), exit.arguments(operation.operands())),
                )
            }

            OperationKind::For(for_operation) => {
                let successors = for_operation.successors();
                let [test] = successors.as_slice() else {
                    unreachable!("for operations must have one executable successor");
                };
                let mut fields = vec![
                    format!(
                        "initializer: bb{}",
                        for_operation.initializer_block().index()
                    ),
                    format!(
                        "test: {}",
                        print_block_target(test.target(), test.arguments(operation.operands()))
                    ),
                    format!("body: bb{}", for_operation.body_block().index()),
                    format!("update: bb{}", for_operation.update_block().index()),
                    format!("exit: bb{}", for_operation.exit_block().index()),
                ];

                if !for_operation.per_iteration_bindings().is_empty() {
                    let bindings = for_operation
                        .per_iteration_bindings()
                        .iter()
                        .map(|binding| format!("@{}", binding.index()))
                        .collect::<Vec<_>>()
                        .join(", ");

                    fields.push(format!("per_iteration: [{bindings}]"));
                }

                if !for_operation.labels().is_empty() {
                    fields.push(format!("labels: {}", print_labels(for_operation.labels())));
                }

                format!("for {}", fields.join(", "))
            }

            OperationKind::ForIn(for_in) => {
                let object = operation
                    .operands()
                    .first()
                    .copied()
                    .expect("for-in operations must have an object operand");
                let successors = for_in.successors();
                let [body, exit] = successors.as_slice() else {
                    unreachable!("for-in operations must have two successors");
                };

                let mut text = format!(
                    "for_in {}, body: {} [produces: 1], exit: {}",
                    print_value(object),
                    print_block_target(body.target(), body.arguments(operation.operands())),
                    print_block_target(exit.target(), exit.arguments(operation.operands())),
                );
                append_loop_fields(&mut text, for_in.per_iteration_bindings(), for_in.labels());
                text
            }

            OperationKind::ForOf(for_of) => {
                let iterable = operation
                    .operands()
                    .first()
                    .copied()
                    .expect("for-of operations must have an iterable operand");
                let successors = for_of.successors();
                let [body, exit] = successors.as_slice() else {
                    unreachable!("for-of operations must have two successors");
                };
                let name = if for_of.kind().is_async() {
                    "for_await_of"
                } else {
                    "for_of"
                };

                let mut text = format!(
                    "{name} {}, body: {} [produces: 1], exit: {}",
                    print_value(iterable),
                    print_block_target(body.target(), body.arguments(operation.operands())),
                    print_block_target(exit.target(), exit.arguments(operation.operands())),
                );
                append_loop_fields(&mut text, for_of.per_iteration_bindings(), for_of.labels());
                text
            }

            OperationKind::Switch(switch) => {
                let discriminant = operation
                    .operands()
                    .first()
                    .copied()
                    .expect("switch operations must have a discriminant operand");
                let successors = switch.successors();
                let cases = switch
                    .cases()
                    .iter()
                    .zip(&successors)
                    .map(|(case, successor)| {
                        let selector = case
                            .test_region()
                            .map(|region| format!("case region @{}", region.index()))
                            .unwrap_or_else(|| "default".to_owned());

                        format!(
                            "{selector}: {}",
                            print_block_target(
                                successor.target(),
                                successor.arguments(operation.operands()),
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut fields = vec![
                    format!("cases: [{cases}]"),
                    format!("completion: bb{}", switch.completion_block().index()),
                ];

                if switch.no_match_target().is_some() {
                    let successor = successors
                        .get(switch.cases().len())
                        .expect("switch without default must have a no-match successor");

                    fields.insert(
                        1,
                        format!(
                            "no_match: {}",
                            print_block_target(
                                successor.target(),
                                successor.arguments(operation.operands()),
                            ),
                        ),
                    );
                }

                if !switch.labels().is_empty() {
                    fields.push(format!("labels: {}", print_labels(switch.labels())));
                }

                format!(
                    "switch {}, {}",
                    print_value(discriminant),
                    fields.join(", ")
                )
            }

            OperationKind::RegionYield(_) => {
                let values = operation
                    .operands()
                    .iter()
                    .copied()
                    .map(print_value)
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("region_yield {values}")
            }

            OperationKind::Return(_) => {
                let [value] = operation.operands() else {
                    unreachable!("return operations have one operand");
                };

                format!("return {}", print_value(*value))
            }

            OperationKind::Throw(_) => {
                let [value] = operation.operands() else {
                    unreachable!("throw operations have one operand");
                };

                format!("throw {}", print_value(*value))
            }
        };

        let unwind = match operation.unwind_target() {
            UnwindTarget::Propagate => String::new(),
            UnwindTarget::Handler(handler) => format!(" [unwind: @{}]", handler.index()),
        };

        format!("{result_prefix}{body}{unwind}")
    }
}

fn print_jsx_element_name(name: &JsxElementName, operands: &[ValueId]) -> String {
    match name {
        JsxElementName::Intrinsic(name) => format!("intrinsic {name:?}"),

        JsxElementName::Reference => {
            format!("reference {}", print_jsx_reference_operand(operands))
        }

        JsxElementName::Member { base, properties } => {
            let mut name = match base {
                JsxMemberBase::Reference => print_jsx_reference_operand(operands),
                JsxMemberBase::This => "this".to_owned(),
            };

            for property in properties {
                name.push('.');
                name.push_str(property);
            }

            format!("member {name}")
        }

        JsxElementName::Namespaced { namespace, name } => {
            format!("namespaced {namespace}:{name}")
        }

        JsxElementName::This => "this".to_owned(),
    }
}

fn print_jsx_reference_operand(operands: &[ValueId]) -> String {
    let [reference] = operands else {
        panic!("JSX reference name must have exactly one operand");
    };

    print_value(*reference)
}

fn print_jsx_attribute(attribute: &JsxAttribute) -> String {
    match attribute {
        JsxAttribute::Named { name, value } => {
            let name = print_jsx_attribute_name(name);

            match value {
                Some(value) => format!("{name}={}", print_jsx_attribute_value(value)),
                None => name,
            }
        }

        JsxAttribute::Spread { expression } => {
            format!("...region @{}", expression.index())
        }
    }
}

fn print_jsx_attribute_name(name: &JsxAttributeName) -> String {
    match name {
        JsxAttributeName::Identifier(name) => format!("{name:?}"),

        JsxAttributeName::Namespaced { namespace, name } => {
            format!("{namespace}:{name}")
        }
    }
}

fn print_jsx_attribute_value(value: &JsxAttributeValue) -> String {
    match value {
        JsxAttributeValue::String(value) => format!("{:?}", value.as_str()),

        JsxAttributeValue::Expression { expression } => {
            format!("region @{}", expression.index())
        }

        JsxAttributeValue::Element { expression } => {
            format!("element region @{}", expression.index())
        }

        JsxAttributeValue::Fragment { expression } => {
            format!("fragment region @{}", expression.index())
        }
    }
}

fn print_jsx_child(child: &JsxChild) -> String {
    match child {
        JsxChild::Text(text) => format!("text {text:?}"),

        JsxChild::Expression { expression } => {
            format!("expression region @{}", expression.index())
        }

        JsxChild::Spread { expression } => {
            format!("spread region @{}", expression.index())
        }

        JsxChild::Element { expression } => {
            format!("element region @{}", expression.index())
        }

        JsxChild::Fragment { expression } => {
            format!("fragment region @{}", expression.index())
        }
    }
}

fn print_call_target(target: &CallTarget, operands: &[ValueId]) -> (String, Option<ValueId>) {
    match target {
        CallTarget::Value { receiver } => {
            let callee = *operands
                .first()
                .expect("value calls must have a callee operand");
            let receiver = match receiver {
                CallReceiver::None => None,
                CallReceiver::Explicit => Some(
                    *operands
                        .get(1)
                        .expect("explicit calls must have a receiver operand"),
                ),
            };

            (print_value(callee), receiver)
        }
        CallTarget::Property(PropertyKey::Static(name)) => {
            let receiver = *operands
                .first()
                .expect("property calls must have a receiver operand");

            (format!("{}[{name:?}]", print_value(receiver)), None)
        }
        CallTarget::Property(PropertyKey::Computed) => {
            let [receiver, key] = operands else {
                unreachable!("computed property calls must have receiver and key operands");
            };

            (
                format!("{}[{}]", print_value(*receiver), print_value(*key)),
                None,
            )
        }
        CallTarget::Property(PropertyKey::Private(private_name)) => {
            let receiver = *operands
                .first()
                .expect("private property calls must have a receiver operand");

            (
                format!(
                    "{}[private @{}]",
                    print_value(receiver),
                    private_name.index(),
                ),
                None,
            )
        }
        CallTarget::SuperProperty(SuperPropertyKey::Static(name)) => {
            (format!("super.{name:?}"), None)
        }
        CallTarget::SuperProperty(SuperPropertyKey::Computed) => {
            let key = *operands
                .first()
                .expect("computed super calls must have a key operand");

            (format!("super[{}]", print_value(key)), None)
        }
    }
}

fn print_call_arguments(arguments: &[CallArgument]) -> String {
    arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Value { expression } => format!("region @{}", expression.index()),
            CallArgument::Spread { expression } => format!("...region @{}", expression.index()),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_function_parameter(parameter: &FunctionParameter) -> String {
    let kind = match parameter.kind() {
        FunctionParameterKind::Argument => "argument",
        FunctionParameterKind::Rest => "rest",
    };

    format!(
        "param {} {kind} {}",
        print_value(parameter.value()),
        print_binding_pattern(parameter.target())
    )
}

fn print_block_parameter(parameter: &BlockParameter) -> String {
    let value = print_value(parameter.value());

    match parameter.source() {
        BlockParameterSource::Produced => format!("{value} [produced]"),
        BlockParameterSource::Forwarded => value,
        BlockParameterSource::Exception => format!("{value} [exception]"),
    }
}

fn print_binding_pattern(pattern: &BindingPattern) -> String {
    match pattern {
        BindingPattern::Binding { binding } => format!("@{}", binding.index()),

        BindingPattern::Array { elements, rest } => {
            let mut parts = elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(print_binding_pattern)
                        .unwrap_or_else(|| "_".into())
                })
                .collect::<Vec<_>>();

            if let Some(rest) = rest {
                parts.push(format!("...{}", print_binding_pattern(rest)));
            }

            format!("[{}]", parts.join(", "))
        }

        BindingPattern::Object { properties, rest } => {
            let mut parts = properties
                .iter()
                .map(|property| match property {
                    ObjectBindingProperty::Static { name, target } => {
                        format!("{name:?}: {}", print_binding_pattern(target))
                    }
                    ObjectBindingProperty::Computed { key, target } => format!(
                        "[region @{}]: {}",
                        key.region().index(),
                        print_binding_pattern(target)
                    ),
                })
                .collect::<Vec<_>>();

            if let Some(rest) = rest {
                parts.push(format!("...{}", print_binding_pattern(rest)));
            }

            format!("{{{}}}", parts.join(", "))
        }

        BindingPattern::Default {
            target,
            initializer,
        } => format!(
            "{} = region @{}",
            print_binding_pattern(target),
            initializer.region().index()
        ),
    }
}

fn print_assignment_pattern(pattern: &AssignmentPattern) -> String {
    match pattern {
        AssignmentPattern::Target { target } => print_assignment_target(target),

        AssignmentPattern::Array { elements, rest } => {
            let mut parts = elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(print_assignment_pattern)
                        .unwrap_or_else(|| "_".into())
                })
                .collect::<Vec<_>>();

            if let Some(rest) = rest {
                parts.push(format!("...{}", print_assignment_pattern(rest)));
            }

            format!("[{}]", parts.join(", "))
        }

        AssignmentPattern::Object { properties, rest } => {
            let mut parts = properties
                .iter()
                .map(|property| match property {
                    ObjectAssignmentProperty::Static { name, target } => {
                        format!("{name:?}: {}", print_assignment_pattern(target))
                    }
                    ObjectAssignmentProperty::Computed { key, target } => format!(
                        "[region @{}]: {}",
                        key.region().index(),
                        print_assignment_pattern(target)
                    ),
                })
                .collect::<Vec<_>>();

            if let Some(rest) = rest {
                parts.push(format!("...{}", print_assignment_pattern(rest)));
            }

            format!("{{{}}}", parts.join(", "))
        }

        AssignmentPattern::Default {
            target,
            initializer,
        } => format!(
            "{} = region @{}",
            print_assignment_pattern(target),
            initializer.region().index()
        ),
    }
}

fn print_assignment_target(target: &AssignmentTarget) -> String {
    match target {
        AssignmentTarget::Binding { binding } => format!("@{}", binding.index()),
        AssignmentTarget::Global { name } => format!("global {name:?}"),
        AssignmentTarget::StaticProperty { object, name } => {
            format!("property(region @{}, {name:?})", object.region().index())
        }
        AssignmentTarget::ComputedProperty { object, key } => format!(
            "property(region @{}, region @{})",
            object.region().index(),
            key.region().index()
        ),
        AssignmentTarget::PrivateProperty {
            object,
            private_name,
        } => format!(
            "property(region @{}, private @{})",
            object.region().index(),
            private_name.index()
        ),
        AssignmentTarget::StaticSuperProperty { name } => {
            format!("super_property({name:?})")
        }
        AssignmentTarget::ComputedSuperProperty { key } => {
            format!("super_property(region @{})", key.region().index())
        }
    }
}

fn print_class_element(element: &ClassElement) -> String {
    match element {
        ClassElement::Method(method) => {
            let placement = match method.placement() {
                ClassMethodPlacement::Prototype => "prototype",
                ClassMethodPlacement::Static => "static",
            };
            let kind = match method.kind() {
                ClassMethodKind::Constructor => "constructor",
                ClassMethodKind::Method => "method",
                ClassMethodKind::Getter => "getter",
                ClassMethodKind::Setter => "setter",
            };

            format!(
                "{placement} {kind} {}: @{}",
                print_class_element_key(method.key()),
                method.function().index(),
            )
        }

        ClassElement::Field(field) => {
            let placement = match field.placement() {
                ClassFieldPlacement::Instance => "instance",
                ClassFieldPlacement::Static => "static",
            };
            let initializer = field
                .initializer()
                .map(|function| format!("@{}", function.index()))
                .unwrap_or_else(|| "none".to_owned());

            format!(
                "{placement} field {}: {initializer}",
                print_class_element_key(field.key()),
            )
        }

        ClassElement::StaticBlock(block) => {
            format!("static block: @{}", block.body().index())
        }
    }
}

fn print_class_element_key(key: &ClassElementKey) -> String {
    match key {
        ClassElementKey::Static(name) => format!("{name:?}"),
        ClassElementKey::Computed(region) => format!("[region @{}]", region.index()),
        ClassElementKey::Private(name) => format!("private @{}", name.index()),
    }
}

fn print_block_target(target: BlockTarget, arguments: &[ValueId]) -> String {
    assert_eq!(
        arguments.len(),
        target.argument_count(),
        "block target argument count must match its operand partition"
    );

    if arguments.is_empty() {
        return format!("bb{}", target.block().index());
    }

    let arguments = arguments
        .iter()
        .copied()
        .map(print_value)
        .collect::<Vec<_>>()
        .join(", ");

    format!("bb{}({arguments})", target.block().index())
}

fn print_labels(labels: &[Box<str>]) -> String {
    let labels = labels
        .iter()
        .map(|label| format!("{label:?}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("[{labels}]")
}

fn append_loop_fields(text: &mut String, bindings: &[BindingId], labels: &[Box<str>]) {
    if !bindings.is_empty() {
        let bindings = bindings
            .iter()
            .map(|binding| format!("@{}", binding.index()))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!(", per_iteration: [{bindings}]"));
    }

    if !labels.is_empty() {
        text.push_str(&format!(", labels: {}", print_labels(labels)));
    }
}

fn print_value(value: ValueId) -> String {
    format!("%{}", value.index())
}

fn print_module_attributes(attributes: &[crate::ModuleAttribute]) -> String {
    if attributes.is_empty() {
        return String::new();
    }

    let attributes = attributes
        .iter()
        .map(|attribute| {
            let key = print_module_export_name(attribute.key());

            format!("{key}: {:?}", attribute.value())
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(" with {{ {attributes} }}")
}

fn print_module_export_name(name: &crate::ModuleExportName) -> String {
    match name {
        crate::ModuleExportName::Identifier(name) => name.to_string(),
        crate::ModuleExportName::String(name) => format!("{name:?}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BinaryOp, BinaryOperator, BindingKind, BindingPattern, ConstantOp, ConstantValue,
        CreateClassOp, CreateFunctionOp, FunctionKind, FunctionMode, FunctionParameterKind,
        IsNullishOp, JsModuleIr, JsxElementName, JsxElementOp, LoadGlobalOp, ModuleBuilder,
        OperationKind, ReturnOp, UnwindTarget,
    };

    use super::{print_function, print_module};

    #[test]
    fn prints_a_module_in_canonical_form() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            module_builder.create_binding(function, "answer", BindingKind::Const);

            module_builder.function_builder(function).append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
        }

        assert_eq!(
            print_module(&module),
            concat!(
                "module {\n",
                "  binding @0 const \"answer\"\n",
                "\n",
                "  function @0 entry {\n",
                "    bb0:\n",
                "      %0 = constant 42\n",
                "  }\n",
                "}",
            )
        );
    }

    #[test]
    fn prints_private_names() {
        let mut module = JsModuleIr::new();

        ModuleBuilder::new(&mut module).create_private_name("value");

        assert_eq!(
            print_module(&module),
            concat!(
                "module {\n",
                "  private_name @0 \"value\"\n",
                "\n",
                "  function @0 entry {\n",
                "    bb0:\n",
                "  }\n",
                "}",
            )
        );
    }

    #[test]
    fn prints_a_nested_function_reference() {
        let mut module = JsModuleIr::new();
        let entry_function = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let nested_function = module_builder.create_function(
                FunctionKind::Arrow,
                FunctionMode::Normal,
                entry_function,
            );

            module_builder
                .function_builder(entry_function)
                .append_operation(
                    crate::LocationId::UNKNOWN,
                    OperationKind::CreateFunction(CreateFunctionOp::new(nested_function)),
                    [],
                    crate::UnwindTarget::Propagate,
                );
        }

        assert_eq!(
            print_module(&module),
            concat!(
                "module {\n",
                "  function @0 entry {\n",
                "    bb0:\n",
                "      %0 = create_function @1\n",
                "  }\n",
                "\n",
                "  function @1 arrow {\n",
                "    bb0:\n",
                "  }\n",
                "}",
            )
        );
    }

    #[test]
    fn prints_a_source_level_function_parameter() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let function =
                module_builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, entry);
            let binding = module_builder.create_binding(function, "value", BindingKind::Parameter);

            module_builder.function_builder(function).append_parameter(
                FunctionParameterKind::Argument,
                BindingPattern::binding(binding),
            );
        }

        assert_eq!(
            print_module(&module),
            concat!(
                "module {\n",
                "  binding @0 parameter \"value\"\n",
                "\n",
                "  function @0 entry {\n",
                "    bb0:\n",
                "  }\n",
                "\n",
                "  function @1 arrow {\n",
                "    param %0 argument @0\n",
                "    \n",
                "    bb0:\n",
                "  }\n",
                "}",
            )
        );
    }

    #[test]
    fn prints_a_function_in_canonical_form() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let left = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(20.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let left = builder.operation_results(left)[0];
            let right = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(22.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let right = builder.operation_results(right)[0];

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
                [left, right],
                crate::UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            "bb0:\n  %0 = constant 20\n  %1 = constant 22\n  %2 = binary.add %0, %1"
        );
    }

    #[test]
    fn prints_exception_handlers_and_unwind_targets() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry_block = module.function(function_id).unwrap().entry_block();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let catch_entry = builder.create_block();
            let (handler, _) = builder.create_catch_handler(None, catch_entry);

            builder.switch_to_block(entry_block);
            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("value")),
                [],
                UnwindTarget::Handler(handler),
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            concat!(
                "handler @0 catch entry: bb1\n",
                "\n",
                "bb0:\n",
                "  %1 = load_global \"value\" [unwind: @0]\n",
                "\n",
                "bb1(%0 [exception]):",
            )
        );
    }

    #[test]
    fn prints_an_undefined_constant() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                crate::UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(print_function(function), "bb0:\n  %0 = constant undefined");
    }

    #[test]
    fn prints_an_empty_class() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::CreateClass(CreateClassOp::new(None, None, [])),
                [],
                crate::UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            "bb0:\n  %0 = create_class self: none, super: none, elements: []"
        );
    }

    #[test]
    fn prints_an_intrinsic_jsx_element() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::JsxElement(JsxElementOp::new(
                    JsxElementName::Intrinsic("div".into()),
                    [],
                    [],
                )),
                [],
                UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            "bb0:\n  %0 = jsx_element intrinsic \"div\", attributes: [], children: []"
        );
    }

    #[test]
    fn prints_a_nullish_predicate() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let value = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Null)),
                [],
                crate::UnwindTarget::Propagate,
            );
            let value = builder.operation_results(value)[0];

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::IsNullish(IsNullishOp::new()),
                [value],
                crate::UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            "bb0:\n  %0 = constant null\n  %1 = is_nullish %0"
        );
    }

    #[test]
    fn prints_a_return_terminator() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let constant = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let value = builder.operation_results(constant)[0];

            builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [value],
                crate::UnwindTarget::Propagate,
            );
        }

        let function = module.function(function_id).unwrap();

        assert_eq!(
            print_function(function),
            "bb0:\n  %0 = constant 42\n  return %0"
        );
    }
}
