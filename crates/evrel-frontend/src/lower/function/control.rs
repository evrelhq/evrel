//! Active break and continue targets during function lowering.

use evrel_ir::BlockId;

use super::FunctionLowerer;

pub(super) enum ControlContext {
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
}

impl ControlContext {
    fn labels(&self) -> &[Box<str>] {
        match self {
            Self::Loop { labels, .. }
            | Self::Switch { labels, .. }
            | Self::LabeledStatement { labels, .. } => labels,
        }
    }

    const fn break_target(&self) -> BlockId {
        match self {
            Self::Loop { break_target, .. }
            | Self::Switch { break_target, .. }
            | Self::LabeledStatement { break_target, .. } => *break_target,
        }
    }

    const fn accepts_unlabeled_break(&self) -> bool {
        matches!(self, Self::Loop { .. } | Self::Switch { .. })
    }

    fn has_label(&self, label: &str) -> bool {
        self.labels()
            .iter()
            .any(|candidate| candidate.as_ref() == label)
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
        let depth = self.controls.len();

        self.controls.push(ControlContext::Loop {
            labels,
            break_target,
            continue_target,
        });

        let result = lower(self);

        self.controls.truncate(depth);

        result
    }

    /// Lowers within an active switch control context.
    pub(crate) fn with_switch_control<R>(
        &mut self,
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.controls.len();

        self.controls.push(ControlContext::Switch {
            labels,
            break_target,
        });

        let result = lower(self);

        self.controls.truncate(depth);

        result
    }

    /// Lowers within a generic labeled-statement control context.
    pub(crate) fn with_labeled_statement_control<R>(
        &mut self,
        labels: Box<[Box<str>]>,
        break_target: BlockId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.controls.len();

        self.controls.push(ControlContext::LabeledStatement {
            labels,
            break_target,
        });

        let result = lower(self);

        self.controls.truncate(depth);

        result
    }

    /// Resolves a source-level `break` target.
    pub(crate) fn break_target(&self, label: Option<&str>) -> Option<BlockId> {
        self.controls.iter().rev().find_map(|control| match label {
            Some(label) => control.has_label(label).then(|| control.break_target()),
            None => control
                .accepts_unlabeled_break()
                .then(|| control.break_target()),
        })
    }

    /// Resolves a source-level `continue` target.
    pub(crate) fn continue_target(&self, label: Option<&str>) -> Option<BlockId> {
        self.controls.iter().rev().find_map(|control| {
            let ControlContext::Loop {
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
                    .then_some(*continue_target),
                None => Some(*continue_target),
            }
        })
    }
}
