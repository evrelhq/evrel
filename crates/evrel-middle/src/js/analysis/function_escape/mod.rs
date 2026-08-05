//! Function-local escape analysis for JavaScript values.

mod graph;

#[cfg(test)]
mod tests;

use evrel_js_ir::{FunctionId, JsModuleIr, OperationKind, ValueId};
use rustc_hash::FxHashSet;

use graph::EscapeGraph;

/// Whether a value may become reachable outside its function activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueEscape {
    DoesNotEscape,
    MayEscape,
}

/// Conservative escape facts for function-local JavaScript identities.
///
/// This analysis answers only whether a value, or a value reachable through a
/// local container, may leave the current function activation. It does not
/// prove that the value's identity or representation is unobservable.
///
/// The analysis is flow-insensitive. Unknown calls and JavaScript operations
/// that may retain an operand are escape boundaries. The result is an immutable
/// snapshot and must be recomputed after changing the function or its lexical
/// binding relationships.
#[derive(Debug, Clone)]
pub struct FunctionEscapeAnalysis {
    function: FunctionId,
    local_values: FxHashSet<ValueId>,
    escaping: FxHashSet<ValueId>,
}

impl FunctionEscapeAnalysis {
    /// Analyzes escape facts for one function.
    pub fn analyze(module: &JsModuleIr, function: FunctionId) -> Option<Self> {
        let function_ir = module.function(function)?;
        let local_values = function_ir
            .operations()
            .filter_map(|(_, operation)| {
                let kind = match operation.kind() {
                    OperationKind::Invoke(invoke) => invoke.operation(),
                    kind => kind,
                };
                if !is_local_identity(kind) {
                    return None;
                }

                let result = match operation.kind() {
                    OperationKind::Invoke(invoke) => function_ir
                        .block(invoke.normal_target().block())
                        .expect("invoke normal target must remain live")
                        .parameters()
                        .first()
                        .expect("local identity invoke must produce one result")
                        .value(),
                    _ => {
                        let [result] = operation.results() else {
                            unreachable!("local identity operations must have one result")
                        };
                        *result
                    }
                };

                Some(result)
            })
            .collect();
        let escaping = EscapeGraph::analyze(module, function, function_ir);

        Some(Self {
            function,
            local_values,
            escaping,
        })
    }

    /// Returns the analyzed function.
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    /// Returns the escape result for a known function-local identity.
    ///
    /// Values that are not known to originate locally return `None`. In
    /// particular, a generic call or JSX runtime result may already alias an
    /// externally retained value.
    pub fn escape_result(&self, value: ValueId) -> Option<ValueEscape> {
        self.local_values.contains(&value).then(|| {
            if self.escaping.contains(&value) {
                ValueEscape::MayEscape
            } else {
                ValueEscape::DoesNotEscape
            }
        })
    }

    /// Conservatively returns whether a value may escape.
    ///
    /// Unknown values return `true`, preventing a missing fact from enabling an
    /// unsafe transformation.
    pub fn may_escape(&self, value: ValueId) -> bool {
        self.escape_result(value) != Some(ValueEscape::DoesNotEscape)
    }
}

fn is_local_identity(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::RegExpLiteral(_)
            | OperationKind::ArrayLiteral(_)
            | OperationKind::ObjectLiteral(_)
            | OperationKind::CreateFunction(_)
            | OperationKind::CreateClass(_)
            | OperationKind::DynamicImport(_)
    )
}
