//! JavaScript declaration discovery.

use evrel_ir::{
    BindingId, BindingKind, ConstantOp, ConstantValue, CreateFunctionOp, InitializeBindingOp,
    ModuleBuilder, OperationKind, StoreBindingOp,
};
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportOrExportKind, Statement, SwitchStatement,
};
use oxc_semantic::{ScopeId, Scoping, SymbolFlags, SymbolId};
use oxc_span::GetSpan;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_ordinary_function_definition;

/// Creates Evrel bindings for declarations owned by the root scope.
///
/// This creates static binding metadata only. Runtime initialization is emitted
/// later while lowering declaration statements.
pub(super) fn declare_root_bindings(
    builder: &mut ModuleBuilder<'_>,
    scoping: &Scoping,
    statements: &[Statement<'_>],
) -> Result<FxHashMap<SymbolId, BindingId>, FrontendError> {
    let mut bindings_by_symbol = FxHashMap::default();
    let declaring_function = builder.entry_function();
    let erased_symbols = erased_root_symbols(statements);

    for symbol in root_symbols(scoping) {
        if erased_symbols.contains(&symbol) {
            continue;
        }

        let kind = binding_kind(scoping, symbol)?;
        let name = scoping.symbol_name(symbol);
        let binding = builder.create_binding(declaring_function, name, kind);

        assert!(
            bindings_by_symbol.insert(symbol, binding).is_none(),
            "Oxc symbol already has an Evrel binding"
        );
    }

    Ok(bindings_by_symbol)
}

fn erased_root_symbols(statements: &[Statement<'_>]) -> FxHashSet<SymbolId> {
    statements
        .iter()
        .filter_map(|statement| {
            let Statement::TSImportEqualsDeclaration(declaration) = statement else {
                return None;
            };

            (declaration.import_kind == ImportOrExportKind::Type)
                .then(|| declaration.id.symbol_id())
        })
        .collect()
}

/// Emits declaration-instantiation work for the root scope.
pub(super) fn instantiate_root_scope(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    scoping: &Scoping,
    statements: &[Statement<'_>],
) -> Result<(), FrontendError> {
    instantiate_hoistable_function_declarations(lowerer, statements.iter())?;
    initialize_root_var_bindings(lowerer, scoping)
}

/// Emits declaration-instantiation work for one function scope.
pub(super) fn instantiate_function_scope(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    scope: ScopeId,
    statements: &[Statement<'_>],
) -> Result<(), FrontendError> {
    instantiate_hoistable_function_declarations(lowerer, statements.iter())?;
    initialize_function_var_bindings(lowerer, scope)
}

/// Emits declaration-instantiation work for one block scope.
pub(super) fn instantiate_block_scope(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statements: &[Statement<'_>],
) -> Result<(), FrontendError> {
    instantiate_hoistable_function_declarations(lowerer, statements.iter())
}

/// Emits declaration-instantiation work shared by all switch clauses.
pub(super) fn instantiate_switch_scope(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &SwitchStatement<'_>,
) -> Result<(), FrontendError> {
    instantiate_hoistable_function_declarations(
        lowerer,
        statement
            .cases
            .iter()
            .flat_map(|case| case.consequent.iter()),
    )
}

fn initialize_root_var_bindings(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    scoping: &Scoping,
) -> Result<(), FrontendError> {
    for symbol in root_symbols(scoping) {
        if binding_kind(scoping, symbol)? != BindingKind::Var {
            continue;
        }

        let binding = lowerer.binding_for_symbol(symbol);
        let span = lowerer.scoping().symbol_span(symbol);

        lowerer.with_span(span, |lowerer| {
            let undefined = lowerer.emit_value(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
            );

            lowerer.emit(
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [undefined],
            );
        });
    }

    Ok(())
}

/// Creates Evrel bindings declared directly in one Oxc scope.
pub(super) fn declare_scope_bindings(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    scope: ScopeId,
) -> Result<(), FrontendError> {
    let symbols = scope_symbols(lowerer.scoping(), scope);

    for symbol in symbols {
        // Parameter lowering registers parameter bindings first. A `var`
        // redeclaration of a parameter also resolves to that same symbol.
        if lowerer.contains_binding(symbol) {
            continue;
        }

        let (name, kind) = {
            let scoping = lowerer.scoping();

            (
                Box::<str>::from(scoping.symbol_name(symbol)),
                binding_kind(scoping, symbol)?,
            )
        };

        lowerer.declare_binding(symbol, name, kind);
    }

    Ok(())
}

fn initialize_function_var_bindings(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    scope: ScopeId,
) -> Result<(), FrontendError> {
    let symbols = scope_symbols(lowerer.scoping(), scope);

    for symbol in symbols {
        if lowerer.binding_kind_for_symbol(symbol) != BindingKind::Var {
            continue;
        }

        let binding = lowerer.binding_for_symbol(symbol);
        let span = lowerer.scoping().symbol_span(symbol);

        lowerer.with_span(span, |lowerer| {
            let undefined = lowerer.emit_value(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
            );

            lowerer.emit(
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [undefined],
            );
        });
    }

    Ok(())
}

/// Instantiates direct hoistable declarations before remaining `var` bindings.
fn instantiate_hoistable_function_declarations<'statement, 'ast>(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statements: impl DoubleEndedIterator<Item = &'statement Statement<'ast>>,
) -> Result<(), FrontendError>
where
    'ast: 'statement,
{
    let mut seen = FxHashSet::default();
    let mut declarations = Vec::new();

    for statement in statements.rev() {
        let declaration = match statement {
            Statement::FunctionDeclaration(declaration) => declaration,

            Statement::ExportNamedDeclaration(export) => {
                let Some(Declaration::FunctionDeclaration(declaration)) = &export.declaration
                else {
                    continue;
                };

                declaration
            }

            Statement::ExportDefaultDeclaration(export) => {
                let ExportDefaultDeclarationKind::FunctionDeclaration(declaration) =
                    &export.declaration
                else {
                    continue;
                };

                declaration
            }

            _ => continue,
        };

        if declaration.declare {
            continue;
        }

        let (binding, kind) = match &declaration.id {
            Some(identifier) => {
                let symbol = identifier.symbol_id();

                (
                    lowerer.binding_for_symbol(symbol),
                    lowerer.binding_kind_for_symbol(symbol),
                )
            }

            None => (
                lowerer
                    .default_export_binding()
                    .expect("anonymous default function must have a synthetic binding"),
                BindingKind::Function,
            ),
        };

        if seen.insert(binding) {
            declarations.push((declaration, binding, kind));
        }
    }

    // Keep source order after selecting the last declaration for each binding.
    declarations.reverse();

    for (declaration, binding, kind) in declarations {
        lowerer.with_span(declaration.span(), |lowerer| {
            let function = lower_ordinary_function_definition(lowerer, declaration)?;
            let value = lowerer.emit_value(
                OperationKind::CreateFunction(CreateFunctionOp::new(function)),
                [],
            );

            match kind {
                BindingKind::Parameter => {
                    lowerer.emit(
                        OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                        [value],
                    );
                }

                BindingKind::Function => {
                    lowerer.emit(
                        OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                        [value],
                    );
                }

                kind => panic!("unexpected function declaration binding kind: {kind:?}"),
            }

            Ok::<_, FrontendError>(())
        })?;
    }

    Ok(())
}

fn root_symbols(scoping: &Scoping) -> Vec<SymbolId> {
    scope_symbols(scoping, scoping.root_scope_id())
}

fn scope_symbols(scoping: &Scoping, scope: ScopeId) -> Vec<SymbolId> {
    let mut symbols = scoping
        .iter_bindings_in(scope)
        .filter(|symbol| {
            let flags = scoping.symbol_flags(*symbol);

            flags.is_value() && !flags.is_type_import() && !flags.is_ambient()
        })
        .collect::<Vec<_>>();

    // Oxc's scope bindings are stored in a hash map. Sort them so Evrel IDs are
    // allocated deterministically in source order.
    symbols.sort_unstable_by_key(|symbol| scoping.symbol_span(*symbol).start);

    symbols
}

fn binding_kind(scoping: &Scoping, symbol: SymbolId) -> Result<BindingKind, FrontendError> {
    let flags = scoping.symbol_flags(symbol);

    // These categories may also be lexical but have distinct semantics.
    if flags.contains(SymbolFlags::Import) {
        return Ok(BindingKind::Import);
    }

    if flags.contains(SymbolFlags::Class) {
        return Ok(BindingKind::Class);
    }

    if flags.contains(SymbolFlags::CatchVariable)
        || flags.contains(SymbolFlags::TypeImport)
        || flags.contains(SymbolFlags::Ambient)
    {
        return Err(unsupported_declaration(scoping, symbol));
    }

    if flags.contains(SymbolFlags::Function) {
        return Ok(BindingKind::Function);
    }

    if flags.contains(SymbolFlags::ConstVariable) {
        return Ok(BindingKind::Const);
    }

    if flags.contains(SymbolFlags::BlockScopedVariable) {
        return Ok(BindingKind::Let);
    }

    if flags.contains(SymbolFlags::FunctionScopedVariable) {
        return Ok(BindingKind::Var);
    }

    Err(unsupported_declaration(scoping, symbol))
}

fn unsupported_declaration(scoping: &Scoping, symbol: SymbolId) -> FrontendError {
    FrontendError::UnsupportedDeclaration {
        name: scoping.symbol_name(symbol).into(),
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::{BindingKind, ModuleBuilder, ModuleIr};
    use oxc_allocator::Allocator;
    use oxc_span::SourceType;

    use crate::parse::parse_module;

    use super::declare_root_bindings;

    #[test]
    fn declares_root_const_bindings() {
        let allocator = Allocator::new();
        let parsed =
            parse_module(&allocator, r#"const message = "hello";"#, SourceType::mjs()).unwrap();
        let mut module = ModuleIr::new();

        {
            let mut builder = ModuleBuilder::new(&mut module);

            declare_root_bindings(&mut builder, parsed.scoping(), &parsed.program().body).unwrap();
        }

        let mut bindings = module.bindings();
        let (_, binding) = bindings.next().unwrap();

        assert_eq!(binding.name(), "message");
        assert_eq!(binding.kind(), BindingKind::Const);
        assert!(bindings.next().is_none());
    }

    #[test]
    fn declares_root_let_bindings() {
        let allocator = Allocator::new();
        let parsed = parse_module(&allocator, "let value;", SourceType::mjs()).unwrap();
        let mut module = ModuleIr::new();

        {
            let mut builder = ModuleBuilder::new(&mut module);

            declare_root_bindings(&mut builder, parsed.scoping(), &parsed.program().body).unwrap();
        }

        let mut bindings = module.bindings();
        let (_, binding) = bindings.next().unwrap();

        assert_eq!(binding.name(), "value");
        assert_eq!(binding.kind(), BindingKind::Let);
        assert!(bindings.next().is_none());
    }

    #[test]
    fn declares_root_function_bindings() {
        let allocator = Allocator::new();
        let parsed = parse_module(&allocator, "function read() {}", SourceType::mjs()).unwrap();
        let mut module = ModuleIr::new();

        {
            let mut builder = ModuleBuilder::new(&mut module);

            declare_root_bindings(&mut builder, parsed.scoping(), &parsed.program().body).unwrap();
        }

        let mut bindings = module.bindings();
        let (_, binding) = bindings.next().unwrap();

        assert_eq!(binding.name(), "read");
        assert_eq!(binding.kind(), BindingKind::Function);
        assert!(bindings.next().is_none());
    }
}
