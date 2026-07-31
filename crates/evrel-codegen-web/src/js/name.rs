//! Collision-free JavaScript name allocation.

use evrel_js_ir::JsModuleIr;
use oxc_syntax::{identifier::is_identifier_name, keyword::is_reserved_keyword};
use rustc_hash::FxHashSet;

/// Names that generated JavaScript bindings must not shadow.
#[derive(Debug, Default)]
pub(crate) struct JsReservedNames {
    names: FxHashSet<Box<str>>,
}

impl JsReservedNames {
    /// Collects every unresolved global name referenced by the module.
    pub(crate) fn collect(module: &JsModuleIr) -> Self {
        let mut reserved = Self::default();

        for (_, binding) in module.bindings() {
            reserved.insert(binding.name());
        }

        for (_, function) in module.functions() {
            for (_, operation) in function.operations() {
                operation.kind().visit_referenced_global_names(|name| {
                    reserved.insert(name);
                });
            }
        }

        reserved
    }

    pub(crate) fn insert(&mut self, name: impl Into<Box<str>>) {
        self.names.insert(name.into());
    }

    pub(crate) fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

/// Allocates unique generated names within one JavaScript function.
#[derive(Debug)]
pub(crate) struct JsNameAllocator<'reserved> {
    reserved: &'reserved JsReservedNames,
    used: FxHashSet<Box<str>>,
    next_generated: u32,
}

impl<'reserved> JsNameAllocator<'reserved> {
    pub(crate) fn new(reserved: &'reserved JsReservedNames) -> Self {
        Self {
            reserved,
            used: FxHashSet::default(),
            next_generated: 0,
        }
    }

    /// Allocates the next available backend-local name.
    pub(crate) fn allocate_generated(&mut self) -> Box<str> {
        loop {
            let index = self.next_generated;

            self.next_generated = self
                .next_generated
                .checked_add(1)
                .expect("a function cannot contain more than u32::MAX generated names");

            let candidate = format!("$evrel{index}").into_boxed_str();

            if self.reserved.contains(&candidate) {
                continue;
            }

            if !self.used.insert(candidate.clone()) {
                continue;
            }

            return candidate;
        }
    }

    /// Allocates a valid source binding name, renaming only invalid spellings
    /// and collisions within the declaring function.
    pub(crate) fn allocate_binding(&mut self, preferred: &str, ordinal: usize) -> Box<str> {
        let base = if is_identifier_name(preferred) && !is_reserved_keyword(preferred) {
            preferred.to_owned()
        } else {
            format!("$binding{ordinal}")
        };

        if self.used.insert(base.clone().into_boxed_str()) {
            return base.into_boxed_str();
        }

        let mut suffix = ordinal;
        loop {
            let candidate = format!("{base}${suffix}").into_boxed_str();

            if self.used.insert(candidate.clone()) {
                return candidate;
            }

            suffix = suffix
                .checked_add(1)
                .expect("a module cannot contain more than usize::MAX bindings");
        }
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{JsModuleIr, LoadGlobalOp, ModuleBuilder, OperationKind, UnwindTarget};

    use super::{JsNameAllocator, JsReservedNames};

    #[test]
    fn collects_unresolved_global_names_from_the_module() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        ModuleBuilder::new(&mut module)
            .function_builder(function)
            .append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("$evrel0")),
                [],
                UnwindTarget::Propagate,
            );

        let reserved = JsReservedNames::collect(&module);

        assert!(reserved.contains("$evrel0"));
        assert!(!reserved.contains("$evrel1"));
    }

    #[test]
    fn generated_names_skip_referenced_globals() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        ModuleBuilder::new(&mut module)
            .function_builder(function)
            .append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("$evrel0")),
                [],
                UnwindTarget::Propagate,
            );

        let reserved = JsReservedNames::collect(&module);
        let mut allocator = JsNameAllocator::new(&reserved);

        assert_eq!(allocator.allocate_generated().as_ref(), "$evrel1",);
        assert_eq!(allocator.allocate_generated().as_ref(), "$evrel2",);
    }

    #[test]
    fn generated_names_are_deterministic() {
        let reserved = JsReservedNames::default();
        let mut first = JsNameAllocator::new(&reserved);
        let mut second = JsNameAllocator::new(&reserved);

        assert_eq!(first.allocate_generated(), second.allocate_generated(),);
        assert_eq!(first.allocate_generated(), second.allocate_generated(),);
    }
}
