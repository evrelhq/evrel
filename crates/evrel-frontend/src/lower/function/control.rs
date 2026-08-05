//! Active JavaScript control frames during function lowering.

use evrel_js_ir::{BlockId, ExceptionHandlerId};

use super::{FunctionLowerer, completion::CleanupFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ResolvedControlTarget {
    block: BlockId,
    frame_index: usize,
}

impl ResolvedControlTarget {
    pub(crate) const fn block(self) -> BlockId {
        self.block
    }

    pub(super) const fn frame_index(self) -> usize {
        self.frame_index
    }
}

pub(super) enum ControlFrame {
    Loop {
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        continue_target: BlockId,
    },

    Switch {
        labels: Box<[Box<str>]>,
        break_target: BlockId,
    },

    LabeledStatement {
        labels: Box<[Box<str>]>,
        break_target: BlockId,
    },

    Catch {
        handler: ExceptionHandlerId,
    },

    Cleanup(CleanupFrame),
}

impl ControlFrame {
    fn labels(&self) -> Option<&[Box<str>]> {
        match self {
            Self::Loop { labels, .. }
            | Self::Switch { labels, .. }
            | Self::LabeledStatement { labels, .. } => Some(labels),
            Self::Catch { .. } | Self::Cleanup(_) => None,
        }
    }

    const fn break_target(&self) -> Option<BlockId> {
        match self {
            Self::Loop { break_target, .. }
            | Self::Switch { break_target, .. }
            | Self::LabeledStatement { break_target, .. } => Some(*break_target),
            Self::Catch { .. } | Self::Cleanup(_) => None,
        }
    }

    const fn accepts_unlabeled_break(&self) -> bool {
        matches!(self, Self::Loop { .. } | Self::Switch { .. })
    }

    fn has_label(&self, label: &str) -> bool {
        self.labels()
            .is_some_and(|labels| labels.iter().any(|candidate| candidate.as_ref() == label))
    }

    pub(super) const fn exception_handler(&self) -> Option<ExceptionHandlerId> {
        match self {
            Self::Catch { handler } => Some(*handler),
            Self::Cleanup(cleanup) => Some(cleanup.exception_handler()),
            Self::Loop { .. } | Self::Switch { .. } | Self::LabeledStatement { .. } => None,
        }
    }
}

impl<'ir, 'context, 'semantic> FunctionLowerer<'ir, 'context, 'semantic> {
    /// Lowers within an active loop control context.
    pub(crate) fn with_loop_control<R>(
        &mut self,
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        continue_target: BlockId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.control_frames.len();

        self.control_frames.push(ControlFrame::Loop {
            labels,
            break_target,
            continue_target,
        });

        let result = lower(self);

        self.control_frames.truncate(depth);

        result
    }

    /// Lowers within an active switch control context.
    pub(crate) fn with_switch_control<R>(
        &mut self,
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.control_frames.len();

        self.control_frames.push(ControlFrame::Switch {
            labels,
            break_target,
        });

        let result = lower(self);

        self.control_frames.truncate(depth);

        result
    }

    /// Lowers within a generic labeled-statement control context.
    pub(crate) fn with_labeled_statement_control<R>(
        &mut self,
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.control_frames.len();

        self.control_frames.push(ControlFrame::LabeledStatement {
            labels,
            break_target,
        });

        let result = lower(self);

        self.control_frames.truncate(depth);

        result
    }

    /// Resolves a source-level `break` target.
    pub(crate) fn break_target(&self, label: Option<&str>) -> Option<ResolvedControlTarget> {
        self.control_frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(frame_index, control)| {
                let matches = match label {
                    Some(label) => control.has_label(label),
                    None => control.accepts_unlabeled_break(),
                };

                matches.then(|| ResolvedControlTarget {
                    block: control
                        .break_target()
                        .expect("matching control frame must accept break"),
                    frame_index,
                })
            })
    }

    /// Resolves a source-level `continue` target.
    pub(crate) fn continue_target(&self, label: Option<&str>) -> Option<ResolvedControlTarget> {
        self.control_frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(frame_index, control)| {
                let ControlFrame::Loop {
                    labels,
                    continue_target,
                    ..
                } = control
                else {
                    return None;
                };

                match label {
                    Some(label) => labels
                        .iter()
                        .any(|candidate| candidate.as_ref() == label)
                        .then_some(ResolvedControlTarget {
                            block: *continue_target,
                            frame_index,
                        }),
                    None => Some(ResolvedControlTarget {
                        block: *continue_target,
                        frame_index,
                    }),
                }
            })
    }

    /// Lowers while routing thrown completions to one catch handler.
    pub(crate) fn with_catch_handler<R>(
        &mut self,
        handler: ExceptionHandlerId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.control_frames.len();
        self.control_frames.push(ControlFrame::Catch { handler });

        let result = lower(self);

        self.control_frames.truncate(depth);

        result
    }

    pub(super) fn active_exception_handler(&self) -> Option<ExceptionHandlerId> {
        let current_region = self.builder.current_region();

        self.control_frames
            .iter()
            .rev()
            .filter_map(ControlFrame::exception_handler)
            .find(|&handler| {
                let handler = self
                    .builder
                    .exception_handler(handler)
                    .expect("active exception handler must remain live");

                self.builder.block_region(handler.entry_block()) == Some(current_region)
            })
    }
}
