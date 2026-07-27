//! SSA edge-transfer planning and parallel-copy scheduling.

use std::collections::HashMap;

use evrel_ir::{BindingId, FunctionIr, ValueId};

use crate::JsCodegenError;

use super::{DenseMap, JsEdgeKey, JsLocalAllocator, JsLocalId, JsValueRepresentation};

/// A value that can safely be read while executing an edge transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsMoveSource {
    /// Read a source-level JavaScript binding.
    Binding(BindingId),

    /// Read a generated JavaScript local.
    Local(JsLocalId),

    /// Recreate a context-free IR expression at the transfer site.
    Inline(ValueId),
}

/// One local assignment in an edge transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsMove {
    destination: JsLocalId,
    source: JsMoveSource,
}

impl JsMove {
    const fn new(destination: JsLocalId, source: JsMoveSource) -> Self {
        Self {
            destination,
            source,
        }
    }

    pub(crate) const fn destination(self) -> JsLocalId {
        self.destination
    }

    pub(crate) const fn source(self) -> JsMoveSource {
        self.source
    }
}

/// Sequential assignments implementing one simultaneous edge transfer.
#[derive(Debug)]
pub(crate) struct JsEdgeTransfer {
    moves: Vec<JsMove>,
}

impl JsEdgeTransfer {
    pub(crate) fn moves(&self) -> &[JsMove] {
        &self.moves
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

/// Converts simultaneous copies into a safe sequential assignment order.
pub(crate) fn schedule_parallel_copies(
    copies: &[JsMove],
    locals: &mut JsLocalAllocator,
) -> JsEdgeTransfer {
    let mut pending = copies.to_vec();
    let mut moves = Vec::with_capacity(copies.len());
    let mut scratch = None;

    while !pending.is_empty() {
        if let Some(index) = pending.iter().position(|copy| {
            !pending.iter().any(|other| {
                matches!(
                    other.source(),
                    JsMoveSource::Local(source) if source == copy.destination()
                )
            })
        }) {
            let copy = pending.remove(index);
            moves.push(copy);
            continue;
        }

        let preserved = pending[0].destination();
        let scratch = *scratch.get_or_insert_with(|| locals.allocate());

        moves.push(JsMove::new(scratch, JsMoveSource::Local(preserved)));

        for copy in &mut pending {
            if copy.source() == JsMoveSource::Local(preserved) {
                copy.source = JsMoveSource::Local(scratch);
            }
        }
    }

    JsEdgeTransfer { moves }
}

pub(crate) fn build_edge_transfers(
    function: &FunctionIr,
    edges: &[JsEdgeKey],
    values: &DenseMap<ValueId, JsValueRepresentation>,
    locals: &mut JsLocalAllocator,
) -> Result<HashMap<JsEdgeKey, JsEdgeTransfer>, JsCodegenError> {
    let mut transfers = HashMap::with_capacity(edges.len());

    for &edge in edges {
        let copies = build_parallel_copies(function, edge, values)?;
        let transfer = schedule_parallel_copies(&copies, locals);

        if transfers.insert(edge, transfer).is_some() {
            return Err(JsCodegenError::MalformedOperation {
                operation: edge.terminator(),
            });
        }
    }

    Ok(transfers)
}

pub(super) fn build_parallel_copies(
    function: &FunctionIr,
    edge: JsEdgeKey,
    values: &DenseMap<ValueId, JsValueRepresentation>,
) -> Result<Vec<JsMove>, JsCodegenError> {
    let terminator =
        function
            .operation(edge.terminator())
            .ok_or(JsCodegenError::UnknownOperation {
                operation: edge.terminator(),
            })?;
    let successor = terminator
        .successors()
        .get(edge.successor_index())
        .copied()
        .ok_or(JsCodegenError::MalformedOperation {
            operation: edge.terminator(),
        })?;
    let target =
        function
            .block(successor.target().block())
            .ok_or(JsCodegenError::UnknownBlock {
                block: successor.target().block(),
            })?;
    let parameters = target
        .parameters()
        .get(successor.produced_argument_count()..)
        .ok_or(JsCodegenError::MalformedOperation {
            operation: edge.terminator(),
        })?;
    let arguments = successor.arguments(terminator.operands());

    if arguments.len() != parameters.len() {
        return Err(JsCodegenError::MalformedOperation {
            operation: edge.terminator(),
        });
    }

    let mut copies = Vec::with_capacity(arguments.len());

    for (&argument, parameter) in arguments.iter().zip(parameters) {
        let parameter = parameter.value();
        let Some(JsValueRepresentation::Temporary(destination)) = values.get(parameter).copied()
        else {
            return Err(JsCodegenError::UnsupportedValue { value: parameter });
        };
        let source = match values.get(argument).copied() {
            Some(JsValueRepresentation::Binding(binding)) => JsMoveSource::Binding(binding),
            Some(JsValueRepresentation::Temporary(local)) => JsMoveSource::Local(local),
            Some(JsValueRepresentation::Inline) => JsMoveSource::Inline(argument),
            Some(JsValueRepresentation::CreationAtUse) => {
                return Err(JsCodegenError::UnsupportedValue { value: argument });
            }
            Some(JsValueRepresentation::DirectEval) => {
                return Err(JsCodegenError::UnsupportedValue { value: argument });
            }
            None => return Err(JsCodegenError::UnsupportedValue { value: argument }),
        };

        if source == JsMoveSource::Local(destination) {
            continue;
        }

        if let Some(existing) = copies
            .iter()
            .find(|copy: &&JsMove| copy.destination() == destination)
        {
            if existing.source() == source {
                continue;
            }

            return Err(JsCodegenError::MalformedOperation {
                operation: edge.terminator(),
            });
        }

        copies.push(JsMove::new(destination, source));
    }

    Ok(copies)
}

#[cfg(test)]
mod tests {
    use super::{JsMove, JsMoveSource, schedule_parallel_copies};
    use crate::plan::{JsLocalAllocator, JsLocalId};

    fn local(index: usize) -> JsLocalId {
        JsLocalId::from_index(index)
    }

    #[test]
    fn schedules_an_acyclic_copy_chain_without_scratch() {
        let mut locals = JsLocalAllocator::default();
        let copies = [
            JsMove::new(local(0), JsMoveSource::Local(local(1))),
            JsMove::new(local(1), JsMoveSource::Local(local(2))),
        ];

        let transfer = schedule_parallel_copies(&copies, &mut locals);
        assert_eq!(
            transfer.moves(),
            [
                JsMove::new(local(0), JsMoveSource::Local(local(1))),
                JsMove::new(local(1), JsMoveSource::Local(local(2))),
            ],
        );
    }

    #[test]
    fn breaks_a_two_local_swap_with_one_scratch() {
        let mut locals = JsLocalAllocator::default();
        let first = locals.allocate();
        let second = locals.allocate();
        let copies = [
            JsMove::new(first, JsMoveSource::Local(second)),
            JsMove::new(second, JsMoveSource::Local(first)),
        ];

        let transfer = schedule_parallel_copies(&copies, &mut locals);
        let scratch = local(2);

        assert_eq!(
            transfer.moves(),
            [
                JsMove::new(scratch, JsMoveSource::Local(first)),
                JsMove::new(first, JsMoveSource::Local(second)),
                JsMove::new(second, JsMoveSource::Local(scratch)),
            ],
        );
    }

    #[test]
    fn uses_one_scratch_for_multiple_disjoint_cycles() {
        let mut locals = JsLocalAllocator::default();
        let a = locals.allocate();
        let b = locals.allocate();
        let c = locals.allocate();
        let d = locals.allocate();
        let copies = [
            JsMove::new(a, JsMoveSource::Local(b)),
            JsMove::new(b, JsMoveSource::Local(a)),
            JsMove::new(c, JsMoveSource::Local(d)),
            JsMove::new(d, JsMoveSource::Local(c)),
        ];

        let transfer = schedule_parallel_copies(&copies, &mut locals);
        let scratch = local(4);

        assert_eq!(
            transfer
                .moves()
                .iter()
                .filter(|movement| movement.destination() == scratch)
                .count(),
            2,
        );
    }
}
