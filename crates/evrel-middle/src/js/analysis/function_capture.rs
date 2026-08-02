//! Lexical bindings captured across function boundaries.

use std::collections::BTreeMap;

use evrel_js_ir::{BindingId, FunctionId, JsModuleIr, OperationKind};

/// How a function directly accesses a captured binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureAccess {
    /// The binding is only read.
    Read,

    /// The binding is only written.
    Write,

    /// The binding is both read and written.
    ReadWrite,
}

impl CaptureAccess {
    /// Returns whether the capture may read the binding.
    pub const fn reads(self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite)
    }

    /// Returns whether the capture may write the binding.
    pub const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }

    const fn merge(self, other: Self) -> Self {
        match (
            self.reads() || other.reads(),
            self.writes() || other.writes(),
        ) {
            (true, false) => Self::Read,
            (false, true) => Self::Write,
            (true, true) => Self::ReadWrite,
            (false, false) => unreachable!(),
        }
    }
}

/// One lexical binding captured by a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingCapture {
    binding: BindingId,
    access: CaptureAccess,
}

impl BindingCapture {
    /// Returns the captured binding.
    pub const fn binding(self) -> BindingId {
        self.binding
    }

    /// Returns how the capturing function accesses the binding.
    pub const fn access(self) -> CaptureAccess {
        self.access
    }
}

/// Immutable lexical-binding captures for every function in one module.
///
/// A function captures a binding when its own operations reference a binding
/// declared by a lexical ancestor. `captured_bindings` contains these direct
/// references. `captured_locals` provides the inverse aggregate: bindings
/// declared by a function and referenced by any descendant function.
///
/// This analysis reports statically explicit references. It does not model
/// bindings made observable through direct `eval`. Recompute it after changing
/// functions, their ancestry, bindings, or operations that reference bindings.
#[derive(Debug, Clone)]
pub struct FunctionCaptureAnalysis {
    captured_bindings: BTreeMap<FunctionId, Box<[BindingCapture]>>,
    captured_locals: BTreeMap<FunctionId, Box<[BindingCapture]>>,
}

impl FunctionCaptureAnalysis {
    /// Computes lexical binding captures for every live function in `module`.
    pub fn analyze(module: &JsModuleIr) -> Self {
        let mut captured_bindings = empty_capture_maps(module);
        let mut captured_locals = empty_capture_maps(module);

        for (function_id, function) in module.functions() {
            for (_, operation) in function.operations() {
                visit_binding_accesses(operation.kind(), |binding, access| {
                    let declaring_function = module
                        .binding(binding)
                        .expect("operation must reference a live binding")
                        .declaring_function();

                    if declaring_function == function_id {
                        return;
                    }

                    assert!(
                        is_lexical_ancestor(module, declaring_function, function_id),
                        "a captured binding must be declared by a lexical ancestor"
                    );

                    record_capture(
                        captured_bindings
                            .get_mut(&function_id)
                            .expect("capturing function must remain live"),
                        binding,
                        access,
                    );
                    record_capture(
                        captured_locals
                            .get_mut(&declaring_function)
                            .expect("declaring function must remain live"),
                        binding,
                        access,
                    );
                });
            }
        }

        Self {
            captured_bindings: finish_capture_maps(captured_bindings),
            captured_locals: finish_capture_maps(captured_locals),
        }
    }

    /// Returns bindings directly captured by `function` in binding-ID order.
    pub fn captured_bindings(&self, function: FunctionId) -> Option<&[BindingCapture]> {
        self.captured_bindings.get(&function).map(Box::as_ref)
    }

    /// Returns locals of `function` captured by any lexical descendant.
    ///
    /// Access is aggregated across all directly referencing descendants.
    pub fn captured_locals(&self, function: FunctionId) -> Option<&[BindingCapture]> {
        self.captured_locals.get(&function).map(Box::as_ref)
    }
}

fn empty_capture_maps(
    module: &JsModuleIr,
) -> BTreeMap<FunctionId, BTreeMap<BindingId, CaptureAccess>> {
    module
        .functions()
        .map(|(function, _)| (function, BTreeMap::new()))
        .collect()
}

fn finish_capture_maps(
    captures: BTreeMap<FunctionId, BTreeMap<BindingId, CaptureAccess>>,
) -> BTreeMap<FunctionId, Box<[BindingCapture]>> {
    captures
        .into_iter()
        .map(|(function, captures)| {
            let captures = captures
                .into_iter()
                .map(|(binding, access)| BindingCapture { binding, access })
                .collect();
            (function, captures)
        })
        .collect()
}

fn record_capture(
    captures: &mut BTreeMap<BindingId, CaptureAccess>,
    binding: BindingId,
    access: CaptureAccess,
) {
    captures
        .entry(binding)
        .and_modify(|existing| *existing = existing.merge(access))
        .or_insert(access);
}

fn is_lexical_ancestor(
    module: &JsModuleIr,
    ancestor: FunctionId,
    mut descendant: FunctionId,
) -> bool {
    while let Some(parent) = module
        .function(descendant)
        .expect("function ancestry must remain live")
        .parent_function()
    {
        if parent == ancestor {
            return true;
        }
        descendant = parent;
    }

    false
}

fn visit_binding_accesses(kind: &OperationKind, mut visit: impl FnMut(BindingId, CaptureAccess)) {
    match kind {
        OperationKind::InitializeBinding(operation) => {
            visit(operation.binding(), CaptureAccess::Write);
        }
        OperationKind::DestructureBinding(operation) => {
            operation
                .pattern()
                .binding_ids()
                .into_iter()
                .for_each(|binding| visit(binding, CaptureAccess::Write));
        }
        OperationKind::DestructureAssignment(operation) => {
            operation
                .pattern()
                .binding_ids()
                .into_iter()
                .for_each(|binding| visit(binding, CaptureAccess::Write));
        }
        OperationKind::LoadBinding(operation) => {
            visit(operation.binding(), CaptureAccess::Read);
        }
        OperationKind::StoreBinding(operation) => {
            visit(operation.binding(), CaptureAccess::Write);
        }
        OperationKind::CreateClass(operation) => {
            if let Some(binding) = operation.self_binding() {
                visit(binding, CaptureAccess::Write);
            }
        }

        // These bindings describe per-iteration environment cloning. They are
        // not binding reads or writes performed by the loop operation itself.
        OperationKind::For(_) | OperationKind::ForIn(_) | OperationKind::ForOf(_) => {}

        _ => {
            debug_assert!({
                let mut has_binding = false;
                kind.visit_referenced_bindings(|_| has_binding = true);
                !has_binding
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BindingId, BindingKind, ConstantOp, ConstantValue, FunctionId, FunctionKind, FunctionMode,
        JsModuleIr, LoadBindingOp, LocationId, ModuleBuilder, OperationKind, StoreBindingOp,
    };

    use super::{BindingCapture, CaptureAccess, FunctionCaptureAnalysis};

    fn append_read(module: &mut JsModuleIr, function: FunctionId, binding: BindingId) {
        let mut module_builder = ModuleBuilder::new(module);
        module_builder.function_builder(function).append_operation(
            LocationId::UNKNOWN,
            OperationKind::LoadBinding(LoadBindingOp::new(binding)),
            [],
        );
    }

    fn append_write(module: &mut JsModuleIr, function: FunctionId, binding: BindingId) {
        let mut module_builder = ModuleBuilder::new(module);
        let mut builder = module_builder.function_builder(function);
        let constant = builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
            [],
        );
        let value = builder.operation_results(constant)[0];
        builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::StoreBinding(StoreBindingOp::new(binding)),
            [value],
        );
    }

    #[test]
    fn computes_direct_captures_and_captured_locals() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();
        let (component, callback, nested, module_binding, local_binding) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let component =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let callback =
                builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, component);
            let nested =
                builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, callback);
            let module_binding = builder.create_binding(entry, "theme", BindingKind::Let);
            let local_binding = builder.create_binding(component, "count", BindingKind::Let);
            (component, callback, nested, module_binding, local_binding)
        };

        append_read(&mut module, callback, module_binding);
        append_read(&mut module, callback, local_binding);
        append_write(&mut module, callback, local_binding);
        append_write(&mut module, nested, local_binding);

        let captures = FunctionCaptureAnalysis::analyze(&module);

        assert_eq!(captures.captured_bindings(entry), Some([].as_slice()));
        assert_eq!(captures.captured_bindings(component), Some([].as_slice()));
        assert_eq!(
            captures.captured_bindings(callback),
            Some(
                [
                    BindingCapture {
                        binding: module_binding,
                        access: CaptureAccess::Read,
                    },
                    BindingCapture {
                        binding: local_binding,
                        access: CaptureAccess::ReadWrite,
                    },
                ]
                .as_slice()
            )
        );
        assert_eq!(
            captures.captured_bindings(nested),
            Some(
                [BindingCapture {
                    binding: local_binding,
                    access: CaptureAccess::Write,
                }]
                .as_slice()
            )
        );
        assert_eq!(
            captures.captured_locals(entry),
            Some(
                [BindingCapture {
                    binding: module_binding,
                    access: CaptureAccess::Read,
                }]
                .as_slice()
            )
        );
        assert_eq!(
            captures.captured_locals(component),
            Some(
                [BindingCapture {
                    binding: local_binding,
                    access: CaptureAccess::ReadWrite,
                }]
                .as_slice()
            )
        );
        assert_eq!(captures.captured_locals(callback), Some([].as_slice()));
    }

    #[test]
    fn exposes_capture_access_queries() {
        assert!(CaptureAccess::Read.reads());
        assert!(!CaptureAccess::Read.writes());
        assert!(!CaptureAccess::Write.reads());
        assert!(CaptureAccess::Write.writes());
        assert!(CaptureAccess::ReadWrite.reads());
        assert!(CaptureAccess::ReadWrite.writes());
    }
}
