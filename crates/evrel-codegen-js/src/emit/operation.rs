//! Planned operation and value emission.

use evrel_ir::{
    DeleteTarget, FunctionIr, FunctionKind, ModuleIr, OperationId, OperationKind, PropertyKey,
    SuperPropertyKey, ValueId,
};
use oxc_allocator::Vec as ArenaVec;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, Statement},
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    emit::unary::{emit_typeof_expression, emit_unary_expression},
    plan::{JsFunctionPlan, JsModulePlan, JsOperationPlan, JsValueRepresentation},
};

use super::{
    FunctionEmission,
    array::emit_array_literal_expression,
    binary::emit_binary_expression,
    binding::{
        emit_initialize_binding_statement, emit_load_binding_expression,
        emit_store_binding_statement,
    },
    call::{emit_call_expression, emit_construct_expression, emit_super_call_expression},
    class::emit_class_expression,
    completion::{emit_return_statement, emit_throw_statement},
    constant::emit_constant_expression,
    context::{
        emit_load_arguments_expression, emit_load_this_expression, emit_meta_property_expression,
    },
    delete::{emit_delete_property_expression, emit_delete_value_expression},
    destructure::{emit_destructure_assignment_statement, emit_destructure_binding_statement},
    function::{emit_create_function_expression, emit_function_declaration_statement},
    global::{emit_load_global_expression, emit_store_global_expression},
    jsx::{emit_jsx_element_expression, emit_jsx_fragment_expression},
    module::emit_dynamic_import_expression,
    object::emit_object_literal_expression,
    operand::emit_operand_expression,
    predicate::{emit_has_private_name_expression, emit_is_nullish_expression},
    property::{
        emit_computed_member_expression, emit_private_member_expression,
        emit_property_read_expression, emit_property_store_statement,
        emit_static_member_expression,
    },
    regexp::emit_regexp_literal_expression,
    suspension::{emit_await_expression, emit_yield_expression},
    template::{emit_tagged_template_expression, emit_template_literal_expression},
    update::emit_update_statement,
    value::emit_value_expression,
};

/// Emits an operation according to its planned statement behavior.
pub(crate) fn emit_operation<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    function_plan: &JsFunctionPlan,
    statements: &mut ArenaVec<'ast, Statement<'ast>>,
    operation: OperationId,
) -> Result<(), JsCodegenError> {
    match function_plan.operation(operation) {
        Some(JsOperationPlan::Omitted) => return Ok(()),
        Some(JsOperationPlan::FunctionDeclaration { function, binding }) => {
            let name = function_plan
                .binding_name(binding)
                .ok_or(JsCodegenError::UnknownBinding { binding })?;
            statements.push(emit_function_declaration_statement(
                builder,
                module,
                output_plan,
                function,
                name,
            )?);
            return Ok(());
        }
        Some(JsOperationPlan::VarDeclaration) | None => {}
    }

    statements.push(emit_operation_statement(
        FunctionEmission::new(builder, module, output_plan, function, function_plan),
        operation,
    )?);

    Ok(())
}

/// Emits one operation as a JavaScript statement.
fn emit_operation_statement<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
) -> Result<Statement<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let module = emission.module;
    let output_plan = emission.output_plan;
    let function = emission.function;
    let plan = emission.plan;
    let operation_data = function
        .operation(operation)
        .ok_or(JsCodegenError::UnknownOperation { operation })?;

    match operation_data.kind() {
        OperationKind::Constant(constant) => Ok(Statement::new_expression_statement(
            SPAN,
            emit_constant_expression(builder, constant.value()),
            builder,
        )),

        OperationKind::RegExpLiteral(literal) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression = emit_regexp_literal_expression(builder, operation, literal)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::ArrayLiteral(array) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression =
                emit_array_literal_expression(builder, module, output_plan, function, plan, array)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::TemplateLiteral(template) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_template_literal_expression(emission, template)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::TaggedTemplate(template) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_tagged_template_expression(
                emission,
                operation,
                template,
                operation_data.operands(),
            )?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::ObjectLiteral(object) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_object_literal_expression(
                builder,
                module,
                output_plan,
                function,
                plan,
                object,
            )?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::JsxElement(element) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_jsx_element_expression(
                emission,
                operation,
                element,
                operation_data.operands(),
            )?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::JsxFragment(fragment) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_jsx_fragment_expression(
                emission,
                operation,
                fragment,
                operation_data.operands(),
            )?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::CreateFunction(create) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression =
                emit_create_function_expression(builder, module, output_plan, operation, create)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::CreateClass(class) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression =
                emit_class_expression(builder, module, output_plan, function, plan, class)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::LoadThis(_) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [_] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            Ok(Statement::new_expression_statement(
                SPAN,
                emit_load_this_expression(builder),
                builder,
            ))
        }

        OperationKind::LoadArguments(_) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [_] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            Ok(Statement::new_expression_statement(
                SPAN,
                emit_load_arguments_expression(builder),
                builder,
            ))
        }

        OperationKind::MetaProperty(meta) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [_] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            Ok(Statement::new_expression_statement(
                SPAN,
                emit_meta_property_expression(builder, meta),
                builder,
            ))
        }

        OperationKind::DynamicImport(import) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let (source, options) = if import.has_options() {
                let [source, options] = operation_data.operands() else {
                    return Err(JsCodegenError::MalformedOperation { operation });
                };

                (
                    emit_value_expression(builder, function, plan, *source)?,
                    Some(emit_value_expression(builder, function, plan, *options)?),
                )
            } else {
                let [source] = operation_data.operands() else {
                    return Err(JsCodegenError::MalformedOperation { operation });
                };

                (
                    emit_value_expression(builder, function, plan, *source)?,
                    None,
                )
            };

            let expression = emit_dynamic_import_expression(builder, import, source, options);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::HasPrivateName(check) => {
            let [object] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression =
                emit_has_private_name_expression(builder, module, function, plan, check, *object)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::IsNullish(_) => {
            let [operand] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression = emit_is_nullish_expression(builder, function, plan, *operand)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Unary(unary) => {
            let [operand] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let argument = emit_value_expression(builder, function, plan, *operand)?;

            let expression = emit_unary_expression(builder, unary, argument);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Update(update) => emit_update_statement(
            emission,
            operation,
            update,
            operation_data.operands(),
            operation_data.results(),
        ),

        OperationKind::Binary(binary) => {
            let [left, right] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let left = emit_value_expression(builder, function, plan, *left)?;
            let right = emit_value_expression(builder, function, plan, *right)?;
            let expression = emit_binary_expression(builder, binary, left, right);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Typeof(typeof_operation) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression = emit_typeof_expression(
                emission,
                operation,
                typeof_operation,
                operation_data.operands(),
            )?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Delete(delete) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression = match delete.target() {
                DeleteTarget::Value => {
                    let [value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let value = emit_value_expression(builder, function, plan, *value)?;

                    emit_delete_value_expression(builder, value)
                }

                DeleteTarget::Property(PropertyKey::Static(name)) => {
                    let [object] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let member = emit_static_member_expression(builder, object, name);

                    emit_delete_property_expression(builder, member)
                }

                DeleteTarget::Property(PropertyKey::Computed) => {
                    let [object, key] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let key = emit_value_expression(builder, function, plan, *key)?;
                    let member = emit_computed_member_expression(builder, object, key);

                    emit_delete_property_expression(builder, member)
                }

                DeleteTarget::Property(PropertyKey::Private(_)) => {
                    return Err(JsCodegenError::UnsupportedOperation {
                        operation,
                        reason: concat!(file!(), ":", line!()),
                    });
                }
            };

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Await(_) => {
            if function.kind() != FunctionKind::Module && !function.mode().is_async() {
                return Err(JsCodegenError::UnsupportedOperation {
                    operation,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = emit_value_expression(builder, function, plan, *value)?;
            let expression = emit_await_expression(builder, value);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Yield(yield_operation) => {
            if !function.mode().is_generator() {
                return Err(JsCodegenError::UnsupportedOperation {
                    operation,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = emit_value_expression(builder, function, plan, *value)?;
            let expression = emit_yield_expression(builder, yield_operation, value);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Call(call) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression =
                emit_call_expression(emission, operation, call, operation_data.operands())?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::SuperCall(call) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_super_call_expression(emission, call)?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::Construct(construct) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let expression = emit_construct_expression(
                emission,
                operation,
                construct,
                operation_data.operands(),
            )?;
            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::InitializeBinding(initialize) => {
            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = if plan.operation(operation) == Some(JsOperationPlan::VarDeclaration) {
                None
            } else {
                Some(emit_operand_expression(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    *value,
                )?)
            };

            emit_initialize_binding_statement(
                builder,
                module,
                plan,
                operation,
                initialize.binding(),
                value,
            )
        }

        OperationKind::DestructureBinding(destructure) => emit_destructure_binding_statement(
            emission,
            operation,
            destructure,
            operation_data.operands(),
        ),

        OperationKind::DestructureAssignment(destructure) => emit_destructure_assignment_statement(
            emission,
            operation,
            destructure,
            operation_data.operands(),
        ),

        OperationKind::LoadBinding(load) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let expression = emit_load_binding_expression(builder, plan, load.binding())?;

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::StoreBinding(store) => {
            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value =
                emit_operand_expression(builder, module, output_plan, function, plan, *value)?;

            emit_store_binding_statement(builder, plan, store.binding(), value)
        }

        OperationKind::LoadGlobal(global) => {
            let expression = emit_load_global_expression(builder, global);

            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::LoadProperty(load) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let member = match load.key() {
                PropertyKey::Static(name) => {
                    let [object] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };

                    let object = emit_value_expression(builder, function, plan, *object)?;

                    emit_static_member_expression(builder, object, name)
                }

                PropertyKey::Computed => {
                    let [object, key] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };

                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let key = emit_value_expression(builder, function, plan, *key)?;

                    emit_computed_member_expression(builder, object, key)
                }

                PropertyKey::Private(private_name) => {
                    let [object] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let object = emit_value_expression(builder, function, plan, *object)?;

                    emit_private_member_expression(builder, module, object, *private_name)?
                }
            };

            let expression = emit_property_read_expression(member);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::StoreProperty(store) => {
            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let (member, value) = match store.key() {
                PropertyKey::Static(name) => {
                    let [object, value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };

                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let value = emit_operand_expression(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        *value,
                    )?;

                    (emit_static_member_expression(builder, object, name), value)
                }

                PropertyKey::Computed => {
                    let [object, key, value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };

                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let key = emit_value_expression(builder, function, plan, *key)?;
                    let value = emit_operand_expression(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        *value,
                    )?;

                    (emit_computed_member_expression(builder, object, key), value)
                }

                PropertyKey::Private(private_name) => {
                    let [object, value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let object = emit_value_expression(builder, function, plan, *object)?;
                    let value = emit_operand_expression(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        *value,
                    )?;

                    (
                        emit_private_member_expression(builder, module, object, *private_name)?,
                        value,
                    )
                }
            };

            Ok(emit_property_store_statement(builder, member, value))
        }

        OperationKind::LoadSuperProperty(load) => {
            let [result] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let object = Expression::new_super(SPAN, builder);
            let member = match load.key() {
                SuperPropertyKey::Static(name) => {
                    let [] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    emit_static_member_expression(builder, object, name)
                }
                SuperPropertyKey::Computed => {
                    let [key] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let key = emit_value_expression(builder, function, plan, *key)?;
                    emit_computed_member_expression(builder, object, key)
                }
            };
            let expression = emit_property_read_expression(member);

            emit_result_statement(builder, plan, *result, expression)
        }

        OperationKind::StoreSuperProperty(store) => {
            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };
            let object = Expression::new_super(SPAN, builder);
            let (member, value) = match store.key() {
                SuperPropertyKey::Static(name) => {
                    let [value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let value = emit_value_expression(builder, function, plan, *value)?;
                    (emit_static_member_expression(builder, object, name), value)
                }
                SuperPropertyKey::Computed => {
                    let [key, value] = operation_data.operands() else {
                        return Err(JsCodegenError::MalformedOperation { operation });
                    };
                    let key = emit_value_expression(builder, function, plan, *key)?;
                    let value = emit_value_expression(builder, function, plan, *value)?;
                    (emit_computed_member_expression(builder, object, key), value)
                }
            };

            Ok(emit_property_store_statement(builder, member, value))
        }

        OperationKind::StoreGlobal(global) => {
            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = emit_value_expression(builder, function, plan, *value)?;

            Ok(Statement::new_expression_statement(
                SPAN,
                emit_store_global_expression(builder, global, value),
                builder,
            ))
        }

        OperationKind::Return(_) => {
            if matches!(
                function.kind(),
                FunctionKind::Module | FunctionKind::ClassStaticBlock
            ) {
                return Err(JsCodegenError::UnsupportedOperation {
                    operation,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = emit_value_expression(builder, function, plan, *value)?;

            Ok(emit_return_statement(builder, value))
        }

        OperationKind::Throw(_) => {
            let [value] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let value = emit_value_expression(builder, function, plan, *value)?;

            Ok(emit_throw_statement(builder, value))
        }

        OperationKind::Debugger(_) => {
            let [] = operation_data.operands() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            let [] = operation_data.results() else {
                return Err(JsCodegenError::MalformedOperation { operation });
            };

            Ok(Statement::new_debugger_statement(SPAN, builder))
        }

        _ => Err(JsCodegenError::UnsupportedOperation {
            operation,
            reason: concat!(file!(), ":", line!()),
        }),
    }
}

fn emit_local_assignment<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
    value: ValueId,
    local: crate::plan::JsLocalId,
    initializer: Expression<'ast>,
) -> Result<Statement<'ast>, JsCodegenError> {
    let name = plan
        .local_name(local)
        .ok_or(JsCodegenError::UnsupportedValue { value })?;

    let name = builder.allocator.alloc_str(name);
    Ok(Statement::new_expression_statement(
        SPAN,
        Expression::new_assignment_expression(
            SPAN,
            oxc_syntax::operator::AssignmentOperator::Assign,
            oxc_ast::ast::AssignmentTarget::new_assignment_target_identifier(SPAN, name, builder),
            initializer,
            builder,
        ),
        builder,
    ))
}

fn emit_result_statement<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
    value: ValueId,
    expression: Expression<'ast>,
) -> Result<Statement<'ast>, JsCodegenError> {
    match plan.value(value) {
        Some(JsValueRepresentation::Temporary(local)) => {
            emit_local_assignment(builder, plan, value, local, expression)
        }
        None => Ok(Statement::new_expression_statement(
            SPAN, expression, builder,
        )),
        Some(
            JsValueRepresentation::Binding(_)
            | JsValueRepresentation::Inline
            | JsValueRepresentation::CreationAtUse
            | JsValueRepresentation::DirectEval,
        ) => Err(JsCodegenError::UnsupportedValue { value }),
    }
}
