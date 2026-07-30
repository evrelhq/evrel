//! JavaScript module lowering.

use evrel_ir::{
    BindingId, BindingKind, FunctionProperties, ModuleBuilder, ModuleExport,
    ModuleExportName as IrModuleExportName, ModuleImport, ModuleIr, SourceFileId, TextRange,
};
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, ImportOrExportKind,
    ModuleExportName as OxcModuleExportName, Statement,
};
use oxc_ecmascript::BoundNames;
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use rustc_hash::FxHashMap;

use crate::{FrontendError, module_attributes::lower_module_attributes, parse::ParsedModule};

use super::{
    FunctionLowerer, LoweringContext,
    declaration::{declare_root_bindings, instantiate_root_scope},
    statement::lower_statement_list,
};

/// Lowers a parsed JavaScript module into Evrel IR.
pub(crate) fn lower_module(
    parsed: &ParsedModule<'_>,
    source_name: &str,
    source_text: &str,
) -> Result<ModuleIr, FrontendError> {
    let mut properties = if parsed.program().source_type.is_strict() {
        FunctionProperties::strict()
    } else {
        FunctionProperties::default()
    };
    if parsed
        .program()
        .directives
        .iter()
        .any(|directive| directive.directive == "use strict")
    {
        properties = properties.with_use_strict_directive();
    }
    let mut module = ModuleIr::with_entry_properties(properties);

    {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let source_file = module_builder.add_source_file(source_name, source_text);
        let program_span = parsed.program().span();
        let program_location = module_builder.source_location(
            source_file,
            evrel_ir::TextRange::new(program_span.start, program_span.end),
        );
        let bindings_by_symbol = declare_root_bindings(
            &mut module_builder,
            parsed.scoping(),
            &parsed.program().body,
        )?;
        collect_imports(
            &mut module_builder,
            &parsed.program().body,
            &bindings_by_symbol,
            source_file,
        )?;
        let default_export_binding = collect_exports(
            &mut module_builder,
            &parsed.program().body,
            parsed.scoping(),
            &bindings_by_symbol,
            source_file,
        )?;
        let mut context = LoweringContext::new(
            parsed.scoping(),
            bindings_by_symbol,
            default_export_binding,
            source_file,
        );
        let entry_function = module_builder.entry_function();
        let function_builder = module_builder.function_builder(entry_function);
        let mut lowerer = FunctionLowerer::new(function_builder, &mut context, program_location);

        instantiate_root_scope(&mut lowerer, parsed.scoping(), &parsed.program().body)?;

        lower_statement_list(&mut lowerer, &parsed.program().body)?;
    }

    Ok(module)
}

fn collect_imports(
    builder: &mut ModuleBuilder<'_>,
    statements: &[Statement<'_>],
    bindings_by_symbol: &FxHashMap<SymbolId, BindingId>,
    source_file: SourceFileId,
) -> Result<(), FrontendError> {
    for statement in statements {
        let Statement::ImportDeclaration(declaration) = statement else {
            continue;
        };

        if declaration.import_kind == ImportOrExportKind::Type {
            continue;
        }

        if declaration.phase.is_some() {
            return Err(FrontendError::UnsupportedStatement);
        }

        let source = declaration.source.value.as_str();
        let attributes = lower_module_attributes(declaration.with_clause.as_deref());
        let Some(specifiers) = &declaration.specifiers else {
            let location = source_location(builder, source_file, declaration.span());
            builder.add_import(ModuleImport::bare(location, source, attributes));
            continue;
        };

        if specifiers.is_empty() {
            let location = source_location(builder, source_file, declaration.span());
            builder.add_import(ModuleImport::bare(location, source, attributes));
            continue;
        }

        for specifier in specifiers {
            let location = source_location(builder, source_file, specifier.span());

            match specifier {
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    let binding = import_binding(bindings_by_symbol, specifier.local.symbol_id());

                    builder.add_import(ModuleImport::default(
                        location,
                        source,
                        attributes.clone(),
                        binding,
                    ));
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    let binding = import_binding(bindings_by_symbol, specifier.local.symbol_id());

                    builder.add_import(ModuleImport::namespace(
                        location,
                        source,
                        attributes.clone(),
                        binding,
                    ));
                }
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    if specifier.import_kind == ImportOrExportKind::Type {
                        continue;
                    }

                    let binding = import_binding(bindings_by_symbol, specifier.local.symbol_id());
                    let imported = lower_module_export_name(&specifier.imported);

                    builder.add_import(ModuleImport::named(
                        location,
                        source,
                        attributes.clone(),
                        imported,
                        binding,
                    ));
                }
            }
        }
    }

    Ok(())
}

fn import_binding(
    bindings_by_symbol: &FxHashMap<SymbolId, BindingId>,
    symbol: SymbolId,
) -> BindingId {
    bindings_by_symbol
        .get(&symbol)
        .copied()
        .expect("Oxc import binding must have an Evrel binding")
}

fn collect_exports(
    builder: &mut ModuleBuilder<'_>,
    statements: &[Statement<'_>],
    scoping: &Scoping,
    bindings_by_symbol: &FxHashMap<SymbolId, BindingId>,
    source_file: SourceFileId,
) -> Result<Option<BindingId>, FrontendError> {
    let mut default_export_binding = None;

    for statement in statements {
        if let Statement::ExportDefaultDeclaration(declaration) = statement {
            let binding = match &declaration.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(function) => match &function.id {
                    Some(identifier) => *bindings_by_symbol
                        .get(&identifier.symbol_id())
                        .expect("named default function must have an Evrel binding"),

                    None => {
                        let entry_function = builder.entry_function();

                        builder.create_binding(entry_function, "*default*", BindingKind::Function)
                    }
                },

                ExportDefaultDeclarationKind::ClassDeclaration(class) => match &class.id {
                    Some(identifier) => *bindings_by_symbol
                        .get(&identifier.symbol_id())
                        .expect("named default class must have an Evrel binding"),

                    None => {
                        let entry_function = builder.entry_function();

                        builder.create_binding(entry_function, "*default*", BindingKind::Class)
                    }
                },

                declaration if declaration.as_expression().is_some() => {
                    let entry_function = builder.entry_function();

                    builder.create_binding(entry_function, "*default*", BindingKind::Const)
                }

                _ => return Err(FrontendError::UnsupportedStatement),
            };

            assert!(
                default_export_binding.replace(binding).is_none(),
                "a module cannot contain multiple default exports"
            );

            let location = source_location(builder, source_file, declaration.span());
            builder.add_export(ModuleExport::local(
                location,
                IrModuleExportName::Identifier("default".into()),
                binding,
            ));

            continue;
        }

        if let Statement::ExportAllDeclaration(declaration) = statement {
            if declaration.export_kind == ImportOrExportKind::Type {
                continue;
            }

            let source = declaration.source.value.as_str();
            let attributes = lower_module_attributes(declaration.with_clause.as_deref());
            let location = source_location(builder, source_file, declaration.span());
            let export = match &declaration.exported {
                Some(exported) => ModuleExport::namespace(
                    location,
                    source,
                    attributes,
                    lower_module_export_name(exported),
                ),

                None => ModuleExport::star(location, source, attributes),
            };

            builder.add_export(export);
            continue;
        }

        let Statement::ExportNamedDeclaration(declaration) = statement else {
            continue;
        };

        if declaration.export_kind == ImportOrExportKind::Type {
            continue;
        }

        if let Some(source) = &declaration.source {
            if declaration.declaration.is_some() || declaration.specifiers.is_empty() {
                return Err(FrontendError::UnsupportedStatement);
            }

            let source = source.value.as_str();
            let attributes = lower_module_attributes(declaration.with_clause.as_deref());

            for specifier in &declaration.specifiers {
                if specifier.export_kind == ImportOrExportKind::Type {
                    continue;
                }

                let imported = lower_module_export_name(&specifier.local);
                let exported = lower_module_export_name(&specifier.exported);
                let location = source_location(builder, source_file, specifier.span());

                builder.add_export(ModuleExport::indirect(
                    location,
                    source,
                    attributes.clone(),
                    imported,
                    exported,
                ));
            }

            continue;
        }

        if declaration.with_clause.is_some() {
            return Err(FrontendError::UnsupportedStatement);
        }

        if let Some(declaration) = &declaration.declaration {
            if is_ambient_declaration(declaration) {
                continue;
            }

            match declaration {
                Declaration::VariableDeclaration(_)
                | Declaration::FunctionDeclaration(_)
                | Declaration::ClassDeclaration(_) => {}
                _ => return Err(FrontendError::UnsupportedStatement),
            }

            declaration.bound_names(&mut |identifier| {
                let binding = *bindings_by_symbol
                    .get(&identifier.symbol_id())
                    .expect("exported Oxc symbol must have an Evrel binding");
                let exported = IrModuleExportName::Identifier(identifier.name.as_str().into());
                let location = source_location(builder, source_file, identifier.span());

                builder.add_export(ModuleExport::local(location, exported, binding));
            });

            continue;
        }

        for specifier in &declaration.specifiers {
            if specifier.export_kind == ImportOrExportKind::Type {
                continue;
            }

            let binding = local_export_binding(&specifier.local, scoping, bindings_by_symbol)?;
            let exported = lower_module_export_name(&specifier.exported);
            let location = source_location(builder, source_file, specifier.span());

            builder.add_export(ModuleExport::local(location, exported, binding));
        }
    }

    Ok(default_export_binding)
}

fn source_location(
    builder: &mut ModuleBuilder<'_>,
    source_file: SourceFileId,
    span: Span,
) -> evrel_ir::LocationId {
    builder.source_location(source_file, TextRange::new(span.start, span.end))
}

fn local_export_binding(
    local: &OxcModuleExportName<'_>,
    scoping: &Scoping,
    bindings_by_symbol: &FxHashMap<SymbolId, BindingId>,
) -> Result<BindingId, FrontendError> {
    let symbol = match local {
        OxcModuleExportName::IdentifierReference(identifier) => {
            let reference = identifier
                .reference_id
                .get()
                .expect("semantic analysis must assign export references");

            scoping
                .get_reference(reference)
                .symbol_id()
                .expect("local export must resolve to a symbol")
        }
        OxcModuleExportName::IdentifierName(identifier) => scoping
            .get_root_binding(identifier.name)
            .expect("local export must resolve to a root binding"),
        OxcModuleExportName::StringLiteral(_) => {
            return Err(FrontendError::UnsupportedStatement);
        }
    };

    Ok(*bindings_by_symbol
        .get(&symbol)
        .expect("exported Oxc symbol must have an Evrel binding"))
}

fn lower_module_export_name(name: &OxcModuleExportName<'_>) -> IrModuleExportName {
    match name {
        OxcModuleExportName::IdentifierName(identifier) => {
            IrModuleExportName::Identifier(identifier.name.as_str().into())
        }
        OxcModuleExportName::IdentifierReference(identifier) => {
            IrModuleExportName::Identifier(identifier.name.as_str().into())
        }
        OxcModuleExportName::StringLiteral(string) => {
            IrModuleExportName::String(string.value.as_str().into())
        }
    }
}

fn is_ambient_declaration(declaration: &Declaration<'_>) -> bool {
    match declaration {
        Declaration::VariableDeclaration(declaration) => declaration.declare,
        Declaration::FunctionDeclaration(declaration) => declaration.declare,
        Declaration::ClassDeclaration(declaration) => declaration.declare,
        Declaration::TSEnumDeclaration(declaration) => declaration.declare,
        Declaration::TSModuleDeclaration(declaration) => declaration.declare,
        Declaration::TSGlobalDeclaration(declaration) => declaration.declare,
        _ => false,
    }
}
