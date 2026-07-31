//! JavaScript module-expression emission.

use std::collections::{HashMap, HashSet};

use evrel_js_ir::{
    BindingId, DynamicImportOp, DynamicImportPhase as IrDynamicImportPhase, JsModuleIr,
    ModuleAttribute, ModuleExport, ModuleExportName as IrModuleExportName, ModuleImport,
};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        BindingIdentifier, BindingPattern, Declaration, ExportSpecifier, Expression,
        ImportAttribute, ImportAttributeKey, ImportDeclarationSpecifier, ImportOrExportKind,
        ImportPhase as AstImportPhase, ModuleExportName as AstModuleExportName, Statement,
        StringLiteral, WithClause, WithClauseKeyword,
    },
};
use oxc_span::SPAN;

use crate::{JsCodegenError, js::plan::JsModulePlan};

/// Emits static imports before the executable module body.
pub(crate) fn emit_module_imports<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    plan: &JsModulePlan,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let function_plan =
        plan.function(module.entry_function())
            .ok_or(JsCodegenError::MissingFunctionPlan {
                function: module.entry_function(),
            })?;
    let mut statements = ArenaVec::with_capacity_in(module.imports().len(), builder);

    for import in module.imports() {
        let specifiers = match import {
            ModuleImport::Bare { .. } => None,
            ModuleImport::Default { binding, .. } => Some(ArenaVec::from_array_in(
                [ImportDeclarationSpecifier::new_import_default_specifier(
                    SPAN,
                    emit_binding_identifier(builder, function_plan, *binding)?,
                    builder,
                )],
                builder,
            )),
            ModuleImport::Namespace { binding, .. } => Some(ArenaVec::from_array_in(
                [ImportDeclarationSpecifier::new_import_namespace_specifier(
                    SPAN,
                    emit_binding_identifier(builder, function_plan, *binding)?,
                    builder,
                )],
                builder,
            )),
            ModuleImport::Named {
                imported, binding, ..
            } => Some(ArenaVec::from_array_in(
                [ImportDeclarationSpecifier::new_import_specifier(
                    SPAN,
                    emit_module_name(builder, imported),
                    emit_binding_identifier(builder, function_plan, *binding)?,
                    ImportOrExportKind::Value,
                    builder,
                )],
                builder,
            )),
        };

        statements.push(Statement::new_import_declaration(
            SPAN,
            specifiers,
            emit_string_literal(builder, import.source()),
            None,
            emit_module_attributes(builder, import.attributes()),
            ImportOrExportKind::Value,
            builder,
        ));
    }

    Ok(statements)
}

/// Attaches same-named local exports directly to representable declarations.
pub(crate) fn attach_local_export_declarations<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    plan: &JsModulePlan,
    statements: ArenaVec<'ast, Statement<'ast>>,
) -> (ArenaVec<'ast, Statement<'ast>>, HashSet<BindingId>) {
    let Some(function_plan) = plan.function(module.entry_function()) else {
        return (statements, HashSet::new());
    };
    let mut attachable = HashMap::new();
    for export in module.exports() {
        let ModuleExport::Local {
            exported, binding, ..
        } = export
        else {
            continue;
        };
        let IrModuleExportName::Identifier(exported) = exported else {
            continue;
        };
        let Some(local) = function_plan.binding_name(*binding) else {
            continue;
        };
        if exported.as_ref() == local {
            attachable.entry(local).or_insert(*binding);
        }
    }

    let mut output = ArenaVec::with_capacity_in(statements.len(), builder);
    let mut attached = HashSet::new();
    for statement in statements {
        let Some(name) = declaration_binding_name(&statement) else {
            output.push(statement);
            continue;
        };
        let Some(&binding) = attachable.get(name) else {
            output.push(statement);
            continue;
        };
        if !attached.insert(binding) {
            output.push(statement);
            continue;
        }

        output.push(Statement::new_export_named_declaration(
            SPAN,
            Some(statement.into_declaration()),
            ArenaVec::new_in(builder),
            None,
            ImportOrExportKind::Value,
            None::<ArenaBox<'ast, WithClause<'ast>>>,
            builder,
        ));
    }

    (output, attached)
}

fn declaration_binding_name<'statement, 'ast>(
    statement: &'statement Statement<'ast>,
) -> Option<&'statement str> {
    match statement.as_declaration()? {
        Declaration::VariableDeclaration(declaration) => {
            let [declarator] = declaration.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                return None;
            };
            Some(identifier.name.as_str())
        }
        Declaration::FunctionDeclaration(function) => function
            .id
            .as_ref()
            .map(|identifier| identifier.name.as_str()),
        Declaration::ClassDeclaration(class) => {
            class.id.as_ref().map(|identifier| identifier.name.as_str())
        }
        _ => None,
    }
}

/// Emits remaining export lists and re-exports after the executable body.
pub(crate) fn emit_module_exports<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    plan: &JsModulePlan,
    attached: &HashSet<BindingId>,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let function_plan =
        plan.function(module.entry_function())
            .ok_or(JsCodegenError::MissingFunctionPlan {
                function: module.entry_function(),
            })?;
    let mut statements = ArenaVec::with_capacity_in(module.exports().len(), builder);

    for export in module.exports() {
        if matches!(
            export,
            ModuleExport::Local { exported, binding, .. }
                if attached.contains(binding)
                    && matches!(
                        exported,
                        IrModuleExportName::Identifier(exported)
                            if function_plan.binding_name(*binding) == Some(exported.as_ref())
                    )
        ) {
            continue;
        }
        statements.push(match export {
            ModuleExport::Empty {
                source, attributes, ..
            } => Statement::new_export_named_declaration(
                SPAN,
                None,
                ArenaVec::new_in(builder),
                Some(emit_string_literal(builder, source)),
                ImportOrExportKind::Value,
                emit_module_attributes(builder, attributes),
                builder,
            ),
            ModuleExport::Local {
                exported, binding, ..
            } => {
                let local = function_plan
                    .binding_name(*binding)
                    .ok_or(JsCodegenError::UnknownBinding { binding: *binding })?;
                let specifier = ExportSpecifier::new(
                    SPAN,
                    AstModuleExportName::new_identifier_reference(
                        SPAN,
                        builder.allocator().alloc_str(local),
                        builder,
                    ),
                    emit_module_name(builder, exported),
                    ImportOrExportKind::Value,
                    builder,
                );

                Statement::new_export_named_declaration(
                    SPAN,
                    None,
                    ArenaVec::from_array_in([specifier], builder),
                    None,
                    ImportOrExportKind::Value,
                    None::<ArenaBox<'ast, WithClause<'ast>>>,
                    builder,
                )
            }
            ModuleExport::Indirect {
                source,
                attributes,
                imported,
                exported,
                ..
            } => {
                let specifier = ExportSpecifier::new(
                    SPAN,
                    emit_module_name(builder, imported),
                    emit_module_name(builder, exported),
                    ImportOrExportKind::Value,
                    builder,
                );

                Statement::new_export_named_declaration(
                    SPAN,
                    None,
                    ArenaVec::from_array_in([specifier], builder),
                    Some(emit_string_literal(builder, source)),
                    ImportOrExportKind::Value,
                    emit_module_attributes(builder, attributes),
                    builder,
                )
            }
            ModuleExport::Namespace {
                source,
                attributes,
                exported,
                ..
            } => Statement::new_export_all_declaration(
                SPAN,
                Some(emit_module_name(builder, exported)),
                emit_string_literal(builder, source),
                emit_module_attributes(builder, attributes),
                ImportOrExportKind::Value,
                builder,
            ),
            ModuleExport::Star {
                source, attributes, ..
            } => Statement::new_export_all_declaration(
                SPAN,
                None,
                emit_string_literal(builder, source),
                emit_module_attributes(builder, attributes),
                ImportOrExportKind::Value,
                builder,
            ),
        });
    }

    Ok(statements)
}

/// Emits one JavaScript dynamic-import expression.
///
/// Operand validation and value emission remain the responsibility of the
/// operation emitter.
pub(crate) fn emit_dynamic_import_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &DynamicImportOp,
    source: Expression<'ast>,
    options: Option<Expression<'ast>>,
) -> Expression<'ast> {
    let phase = match operation.phase() {
        IrDynamicImportPhase::Evaluation => None,
        IrDynamicImportPhase::Source => Some(AstImportPhase::Source),
        IrDynamicImportPhase::Defer => Some(AstImportPhase::Defer),
    };

    Expression::new_import_expression(SPAN, source, options, phase, builder)
}

fn emit_binding_identifier<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &crate::js::plan::JsFunctionPlan,
    binding: evrel_js_ir::BindingId,
) -> Result<BindingIdentifier<'ast>, JsCodegenError> {
    let name = plan
        .binding_name(binding)
        .ok_or(JsCodegenError::UnknownBinding { binding })?;

    Ok(BindingIdentifier::new(
        SPAN,
        builder.allocator().alloc_str(name),
        builder,
    ))
}

fn emit_module_name<'ast>(
    builder: &AstBuilder<'ast>,
    name: &IrModuleExportName,
) -> AstModuleExportName<'ast> {
    match name {
        IrModuleExportName::Identifier(name) => AstModuleExportName::new_identifier_name(
            SPAN,
            builder.allocator().alloc_str(name),
            builder,
        ),
        IrModuleExportName::String(name) => AstModuleExportName::new_string_literal(
            SPAN,
            builder.allocator().alloc_str(name),
            None,
            builder,
        ),
    }
}

fn emit_module_attributes<'ast>(
    builder: &AstBuilder<'ast>,
    attributes: &[ModuleAttribute],
) -> Option<ArenaBox<'ast, WithClause<'ast>>> {
    if attributes.is_empty() {
        return None;
    }

    let mut entries = ArenaVec::with_capacity_in(attributes.len(), builder);

    for attribute in attributes {
        let key = match attribute.key() {
            IrModuleExportName::Identifier(name) => ImportAttributeKey::new_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ),
            IrModuleExportName::String(name) => ImportAttributeKey::new_string_literal(
                SPAN,
                builder.allocator().alloc_str(name),
                None,
                builder,
            ),
        };

        entries.push(ImportAttribute::new(
            SPAN,
            key,
            emit_string_literal(builder, attribute.value()),
            builder,
        ));
    }

    Some(WithClause::boxed(
        SPAN,
        WithClauseKeyword::With,
        entries,
        builder,
    ))
}

fn emit_string_literal<'ast>(builder: &AstBuilder<'ast>, value: &str) -> StringLiteral<'ast> {
    StringLiteral::new(SPAN, builder.allocator().alloc_str(value), None, builder)
}
