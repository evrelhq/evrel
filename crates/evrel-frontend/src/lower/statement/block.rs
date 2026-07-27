//! JavaScript block-statement lowering.

use oxc_ast::ast::BlockStatement;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        declaration::{declare_scope_bindings, instantiate_block_scope},
        statement::lower_statement_list,
    },
};

/// Lowers a block after discovering its lexical declarations.
pub(super) fn lower_block_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &BlockStatement<'_>,
) -> Result<(), FrontendError> {
    if let Some(scope) = statement.scope_id.get() {
        declare_scope_bindings(lowerer, scope)?;
    }

    instantiate_block_scope(lowerer, &statement.body)?;
    lower_statement_list(lowerer, &statement.body)
}
