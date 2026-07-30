//! Module-wide frontend lowering state.

use evrel_ir::{BindingId, PrivateNameId, SourceFileId};
use oxc_ast::ast::IdentifierReference;
use oxc_semantic::{Scoping, SymbolId};
use rustc_hash::FxHashMap;

/// Semantic state shared by every function lowered from one module.
pub(crate) struct LoweringContext<'semantic> {
    scoping: &'semantic Scoping,
    bindings_by_symbol: FxHashMap<SymbolId, BindingId>,
    private_name_scopes: Vec<FxHashMap<Box<str>, PrivateNameId>>,
    default_export_binding: Option<BindingId>,
    source_file: SourceFileId,
}

impl<'semantic> LoweringContext<'semantic> {
    pub(crate) fn new(
        scoping: &'semantic Scoping,
        bindings_by_symbol: FxHashMap<SymbolId, BindingId>,
        default_export_binding: Option<BindingId>,
        source_file: SourceFileId,
    ) -> Self {
        Self {
            scoping,
            bindings_by_symbol,
            private_name_scopes: Vec::new(),
            default_export_binding,
            source_file,
        }
    }

    pub(crate) const fn scoping(&self) -> &Scoping {
        self.scoping
    }

    pub(crate) const fn default_export_binding(&self) -> Option<BindingId> {
        self.default_export_binding
    }

    pub(crate) const fn source_file(&self) -> SourceFileId {
        self.source_file
    }

    pub(crate) fn contains_binding(&self, symbol: SymbolId) -> bool {
        self.bindings_by_symbol.contains_key(&symbol)
    }

    pub(crate) fn binding_for_reference(
        &self,
        identifier: &IdentifierReference<'_>,
    ) -> Option<BindingId> {
        let reference = identifier
            .reference_id
            .get()
            .expect("semantic analysis must assign every identifier reference");

        self.scoping
            .get_reference(reference)
            .symbol_id()
            .map(|symbol| self.binding_for_symbol(symbol))
    }

    pub(crate) fn binding_for_symbol(&self, symbol: SymbolId) -> BindingId {
        *self
            .bindings_by_symbol
            .get(&symbol)
            .expect("every declared Oxc symbol must have an Evrel binding")
    }

    pub(crate) fn register_binding(&mut self, symbol: SymbolId, binding: BindingId) {
        assert!(
            self.bindings_by_symbol.insert(symbol, binding).is_none(),
            "Oxc symbol already has an Evrel binding"
        );
    }

    pub(crate) fn push_private_name_scope(&mut self, scope: FxHashMap<Box<str>, PrivateNameId>) {
        self.private_name_scopes.push(scope);
    }

    pub(crate) fn pop_private_name_scope(&mut self) {
        self.private_name_scopes
            .pop()
            .expect("a private-name scope must be active");
    }

    pub(crate) fn private_name(&self, name: &str) -> PrivateNameId {
        self.private_name_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
            .expect("Oxc must validate every private-name reference")
    }
}
