//! Dense tables indexed by Evrel arena IDs.

use std::marker::PhantomData;

use evrel_ir::{BindingId, FunctionId, OperationId, RegionId, ValueId};

pub(crate) trait DenseId: Copy {
    fn index(self) -> usize;
}

impl DenseId for FunctionId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for OperationId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for ValueId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for BindingId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for RegionId {
    fn index(self) -> usize {
        self.index()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DenseMap<I, T> {
    slots: Vec<Option<T>>,
    id: PhantomData<fn(I)>,
}

impl<I, T> DenseMap<I, T>
where
    I: DenseId,
{
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            id: PhantomData,
        }
    }

    pub(crate) fn insert(&mut self, id: I, value: T) -> Option<T> {
        let index = id.index();

        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, || None);
        }

        self.slots[index].replace(value)
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        self.slots.get(id.index())?.as_ref()
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().filter_map(Option::as_ref)
    }
}

impl<I, T> Default for DenseMap<I, T>
where
    I: DenseId,
{
    fn default() -> Self {
        Self::new()
    }
}
