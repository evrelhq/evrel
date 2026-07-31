//! JavaScript debugger-statement lowering.

use evrel_js_ir::{DebuggerOp, OperationKind};
use oxc_ast::ast::DebuggerStatement;

use crate::lower::FunctionLowerer;

pub(super) fn lower_debugger_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    _statement: &DebuggerStatement,
) {
    lowerer.emit(OperationKind::Debugger(DebuggerOp::new()), []);
}
