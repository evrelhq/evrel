//! JavaScript `try` statement lowering.

use evrel_ir::{
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
struct CatchTarget {
    block: BlockId,
    handler: ExceptionHandlerId,
    exception: ValueId,
}

/// Lowers a JavaScript `try` statement.
pub(super) fn lower_try_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &TryStatement<'_>,
) -> Result<(), FrontendError> {
    let catch_pattern = lower_catch_pattern(lowerer, statement.handler.as_deref())?;
    let completion_block = lowerer.create_block();

    let finally_block = statement.finalizer.as_ref().map(|_| lowerer.create_block());
    let finally_handler = finally_block.map(|block| lowerer.create_finally_handler(block));

    let catch_target = statement
        .handler
        .as_ref()
        .map(|_| create_catch_target(lowerer, finally_handler));

    let protected_handler = catch_target
        .map(|target| target.handler)
        .or(finally_handler)
        .expect("try statement must have catch or finally");
    let try_block = lowerer.with_unwind_handler(protected_handler, FunctionLowerer::create_block);

    lowerer.terminate(
        OperationKind::Try(TryOp::new(
            BlockTarget::new(try_block, 0),
            catch_target.map(|target| target.block),
            finally_block,
            completion_block,
        )),
        [],
    );

    let normal_target = finally_block.unwrap_or(completion_block);

    lowerer.with_unwind_handler(protected_handler, |lowerer| {
        lowerer.switch_to_block(try_block);
        lower_block_statement(lowerer, &statement.block)?;
        jump_if_open(lowerer, normal_target);

        Ok::<_, FrontendError>(())
    })?;

    if let (Some(clause), Some(target)) = (statement.handler.as_ref(), catch_target) {
        match finally_handler {
            Some(handler) => lowerer.with_unwind_handler(handler, |lowerer| {
                lower_catch_clause(lowerer, clause, target, catch_pattern, normal_target)
            })?,

            None => {
                lower_catch_clause(lowerer, clause, target, catch_pattern, normal_target)?;
            }
        }
    }

    if let (Some(finalizer), Some(block)) = (statement.finalizer.as_ref(), finally_block) {
        lowerer.switch_to_block(block);
        lower_block_statement(lowerer, finalizer)?;
        jump_if_open(lowerer, completion_block);
    }

    lowerer.switch_to_block(completion_block);

    Ok(())
}

fn create_catch_target(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    finally_handler: Option<ExceptionHandlerId>,
) -> CatchTarget {
    match finally_handler {
        Some(handler) => lowerer.with_unwind_handler(handler, create_catch_target_in_context),
        None => create_catch_target_in_context(lowerer),
    }
}

fn create_catch_target_in_context(lowerer: &mut FunctionLowerer<'_, '_, '_>) -> CatchTarget {
    let block = lowerer.create_block();
    let (handler, exception) = lowerer.create_catch_handler(block);

    CatchTarget {
        block,
        handler,
        exception,
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
    target: CatchTarget,
    pattern: Option<BindingPattern>,
    normal_target: BlockId,
) -> Result<(), FrontendError> {
    lowerer.switch_to_block(target.block);

    if let Some(pattern) = pattern {
        let operation = match pattern.as_binding() {
            Some(binding) => OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),

            None => OperationKind::DestructureBinding(DestructureBindingOp::new(
                pattern,
                BindingWriteMode::Initialize,
            )),
        };

        lowerer.emit(operation, [target.exception]);
    }

    lower_block_statement(lowerer, &clause.body)?;
    jump_if_open(lowerer, normal_target);

    Ok(())
}

fn jump_if_open(lowerer: &mut FunctionLowerer<'_, '_, '_>, target: BlockId) {
    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
            [],
        );
    }
}
