//! Pending JavaScript completions crossing `finally` clauses.

use evrel_js_ir::{
    BlockId, BlockTarget, CompletionCase, CompletionKind, EnterFinallyOp, ExceptionHandlerId,
    JumpOp, OperationId, OperationKind, ResumeCompletionOp, ReturnOp, ValueId,
};

use super::{
    FunctionLowerer,
    control::{ControlFrame, ResolvedControlTarget},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PendingCompletion {
    Normal,
    Return,
    Throw,
    Break(ResolvedControlTarget),
    Continue(ResolvedControlTarget),
}

impl PendingCompletion {
    const fn ir_kind(self) -> CompletionKind {
        match self {
            Self::Normal => CompletionKind::Normal,
            Self::Return => CompletionKind::Return,
            Self::Throw => CompletionKind::Throw,
            Self::Break(target) => CompletionKind::Break(target.block()),
            Self::Continue(target) => CompletionKind::Continue(target.block()),
        }
    }
}

pub(crate) struct CleanupFrame {
    exception_handler: ExceptionHandlerId,
    entry: BlockId,
    completion_parameter: ValueId,
    continuations: Vec<PendingCompletion>,
}

impl CleanupFrame {
    pub(super) const fn exception_handler(&self) -> ExceptionHandlerId {
        self.exception_handler
    }

    fn register(&mut self, completion: PendingCompletion) {
        if !self.continuations.contains(&completion) {
            self.continuations.push(completion);
        }
    }
}

struct ResumeTarget {
    completion: PendingCompletion,
    block: BlockId,
    payload: Option<ValueId>,
}

impl<'ir, 'context, 'semantic> FunctionLowerer<'ir, 'context, 'semantic> {
    pub(crate) fn push_cleanup(
        &mut self,
        exception_handler: ExceptionHandlerId,
        entry: BlockId,
        completion_parameter: ValueId,
    ) {
        self.control_frames
            .push(ControlFrame::Cleanup(CleanupFrame {
                exception_handler,
                entry,
                completion_parameter,
                continuations: vec![PendingCompletion::Normal, PendingCompletion::Throw],
            }));
    }

    pub(crate) fn pop_cleanup(&mut self, exception_handler: ExceptionHandlerId) -> CleanupFrame {
        let frame = self
            .control_frames
            .pop()
            .expect("cleanup frame stack must remain balanced");
        let ControlFrame::Cleanup(cleanup) = frame else {
            panic!("cleanup frames must nest");
        };

        assert_eq!(
            cleanup.exception_handler, exception_handler,
            "cleanup frames must nest"
        );

        cleanup
    }

    pub(crate) fn terminate_normal_through_finally(&mut self) -> OperationId {
        let frame_index = self
            .innermost_cleanup_frame()
            .expect("normal finally entry requires an active finalizer");

        self.enter_finally(frame_index, PendingCompletion::Normal, [])
    }

    pub(crate) fn terminate_exception_through_finally(&mut self, value: ValueId) -> OperationId {
        let frame_index = self
            .innermost_cleanup_frame()
            .expect("exceptional finally entry requires an active finalizer");

        self.enter_finally(frame_index, PendingCompletion::Throw, [value])
    }

    pub(crate) fn terminate_return(&mut self, value: ValueId) -> OperationId {
        match self.innermost_cleanup_frame() {
            Some(frame_index) => {
                self.enter_finally(frame_index, PendingCompletion::Return, [value])
            }
            None => self.terminate(OperationKind::Return(ReturnOp::new()), [value]),
        }
    }

    pub(crate) fn terminate_break(&mut self, target: ResolvedControlTarget) -> OperationId {
        self.terminate_control(PendingCompletion::Break(target), target)
    }

    pub(crate) fn terminate_continue(&mut self, target: ResolvedControlTarget) -> OperationId {
        self.terminate_control(PendingCompletion::Continue(target), target)
    }

    pub(crate) fn resume_finally(&mut self, cleanup: CleanupFrame, normal_target: BlockId) {
        if self.current_block_is_terminated() {
            return;
        }

        let mut cases = Vec::with_capacity(cleanup.continuations.len());
        let mut targets = Vec::new();

        for pending in cleanup.continuations {
            if pending == PendingCompletion::Normal {
                cases.push(CompletionCase::new(
                    CompletionKind::Normal,
                    BlockTarget::new(normal_target, 0),
                ));
                continue;
            }

            let block = self.create_block();
            let payload = matches!(
                pending,
                PendingCompletion::Return | PendingCompletion::Throw
            )
            .then(|| self.append_produced_block_parameter(block));

            cases.push(CompletionCase::new(
                pending.ir_kind(),
                BlockTarget::new(block, 0),
            ));
            targets.push(ResumeTarget {
                completion: pending,
                block,
                payload,
            });
        }

        self.terminate(
            OperationKind::ResumeCompletion(ResumeCompletionOp::new(cases)),
            [cleanup.completion_parameter],
        );

        for target in targets {
            self.switch_to_block(target.block);

            match target.completion {
                PendingCompletion::Return => {
                    self.terminate_return(target.payload.expect("return completion needs a value"));
                }
                PendingCompletion::Throw => {
                    self.terminate_throw(target.payload.expect("throw completion needs a value"));
                }
                PendingCompletion::Break(control) => {
                    self.terminate_break(control);
                }
                PendingCompletion::Continue(control) => {
                    self.terminate_continue(control);
                }
                PendingCompletion::Normal => unreachable!("normal completion has no bridge"),
            }
        }
    }

    fn terminate_control(
        &mut self,
        completion: PendingCompletion,
        target: ResolvedControlTarget,
    ) -> OperationId {
        let cleanup =
            self.control_frames
                .iter()
                .enumerate()
                .rev()
                .find_map(|(frame_index, frame)| {
                    if frame_index <= target.frame_index() {
                        return None;
                    }

                    matches!(frame, ControlFrame::Cleanup(_)).then_some(frame_index)
                });

        match cleanup {
            Some(frame_index) => self.enter_finally(frame_index, completion, []),
            None => self.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target.block(), 0))),
                [],
            ),
        }
    }

    fn enter_finally(
        &mut self,
        frame_index: usize,
        completion: PendingCompletion,
        payload: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        let frame = self
            .control_frames
            .get_mut(frame_index)
            .expect("completion must reference an active cleanup");
        let ControlFrame::Cleanup(cleanup) = frame else {
            panic!("completion must reference an active cleanup");
        };
        cleanup.register(completion);
        let entry = cleanup.entry;

        self.builder.terminate(
            self.current_location,
            OperationKind::EnterFinally(EnterFinallyOp::new(
                completion.ir_kind(),
                BlockTarget::new(entry, 0),
            )),
            payload,
        )
    }

    fn innermost_cleanup_frame(&self) -> Option<usize> {
        self.control_frames
            .iter()
            .rposition(|frame| matches!(frame, ControlFrame::Cleanup(_)))
    }
}
