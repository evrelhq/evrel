//! JavaScript `try` statement lowering.

use evrel_js_ir::{
    BindingKind, BindingPattern, BindingWriteMode, BlockId, BlockTarget, DestructureBindingOp,
    ExceptionHandlerId, InitializeBindingOp, JumpOp, OperationKind, TryOp, ValueId,
};
use oxc_ast::ast::{CatchClause, TryStatement};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        pattern::{declare_pattern_bindings, lower_binding_pattern},
    },
};

use super::block::lower_block_statement;

#[derive(Clone, Copy)]
struct CatchEntry {
    block: BlockId,
    handler: ExceptionHandlerId,
    exception_parameter: ValueId,
}

#[derive(Clone, Copy)]
struct FinallyEntries {
    body: BlockId,
    exception: BlockId,
    exception_handler: ExceptionHandlerId,
    exception_parameter: ValueId,
    completion_parameter: ValueId,
}

/// Lowers a JavaScript `try` statement.
pub(super) fn lower_try_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &TryStatement<'_>,
) -> Result<(), FrontendError> {
    let catch_pattern = lower_catch_pattern(lowerer, statement.handler.as_deref())?;
    let completion_block = lowerer.create_block();

    let finally_entries = statement.finalizer.as_ref().map(|_| {
        let body = lowerer.create_block();
        let completion_parameter = lowerer.append_completion_block_parameter(body);
        let exception = lowerer.create_block();
        let (exception_handler, exception_parameter) = lowerer.create_finally_handler(exception);

        FinallyEntries {
            body,
            exception,
            exception_handler,
            exception_parameter,
            completion_parameter,
        }
    });

    if let Some(finally) = finally_entries {
        lowerer.push_cleanup(
            finally.exception_handler,
            finally.body,
            finally.completion_parameter,
        );
    }

    let catch_entry = statement
        .handler
        .as_ref()
        .map(|_| create_catch_entry(lowerer));
    let try_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Try(TryOp::new(
            BlockTarget::new(try_block, 0),
            catch_entry.map(|entry| entry.block),
            finally_entries.map(|entries| entries.body),
            finally_entries.map(|entries| entries.exception),
            completion_block,
        )),
        [],
    );

    let lower_try_body = |lowerer: &mut FunctionLowerer<'_, '_, '_>| {
        lowerer.switch_to_block(try_block);
        lower_block_statement(lowerer, &statement.block)?;
        complete_normally(lowerer, finally_entries, completion_block);

        Ok::<_, FrontendError>(())
    };

    match catch_entry {
        Some(entry) => lowerer.with_catch_handler(entry.handler, lower_try_body)?,
        None => lower_try_body(lowerer)?,
    }

    if let (Some(clause), Some(entry)) = (statement.handler.as_ref(), catch_entry) {
        lower_catch_clause(
            lowerer,
            clause,
            entry,
            catch_pattern,
            finally_entries,
            completion_block,
        )?;
    }

    if let (Some(finalizer_statement), Some(finally)) =
        (statement.finalizer.as_ref(), finally_entries)
    {
        lowerer.switch_to_block(finally.exception);
        lowerer.terminate_exception_through_finally(finally.exception_parameter);

        let cleanup = lowerer.pop_cleanup(finally.exception_handler);

        lowerer.switch_to_block(finally.body);
        lower_block_statement(lowerer, finalizer_statement)?;
        lowerer.resume_finally(cleanup, completion_block);
    }

    lowerer.switch_to_block(completion_block);

    Ok(())
}

fn create_catch_entry(lowerer: &mut FunctionLowerer<'_, '_, '_>) -> CatchEntry {
    let block = lowerer.create_block();
    let (handler, exception_parameter) = lowerer.create_catch_handler(block);

    CatchEntry {
        block,
        handler,
        exception_parameter,
    }
}

fn lower_catch_pattern(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    clause: Option<&CatchClause<'_>>,
) -> Result<Option<BindingPattern>, FrontendError> {
    let Some(parameter) = clause.and_then(|clause| clause.param.as_ref()) else {
        return Ok(None);
    };

    declare_pattern_bindings(lowerer, &parameter.pattern, BindingKind::Catch);

    lower_binding_pattern(lowerer, &parameter.pattern).map(Some)
}

fn lower_catch_clause(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    clause: &CatchClause<'_>,
    entry: CatchEntry,
    pattern: Option<BindingPattern>,
    finally: Option<FinallyEntries>,
    completion_block: BlockId,
) -> Result<(), FrontendError> {
    lowerer.switch_to_block(entry.block);

    if let Some(pattern) = pattern {
        let operation = match pattern.as_binding() {
            Some(binding) => OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),

            None => OperationKind::DestructureBinding(DestructureBindingOp::new(
                pattern,
                BindingWriteMode::Initialize,
            )),
        };

        lowerer.emit(operation, [entry.exception_parameter]);
    }

    lower_block_statement(lowerer, &clause.body)?;
    complete_normally(lowerer, finally, completion_block);

    Ok(())
}

fn complete_normally(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    finally: Option<FinallyEntries>,
    completion_block: BlockId,
) {
    if !lowerer.current_block_is_terminated() {
        match finally {
            Some(_) => {
                lowerer.terminate_normal_through_finally();
            }
            None => {
                lowerer.terminate(
                    OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 0))),
                    [],
                );
            }
        }
    }
}
