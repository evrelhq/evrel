//! Typed indexed storage for IR entities.

use std::marker::PhantomData;

/// An identifier that can address an entity in an arena.
pub(crate) trait ArenaId: Copy {
    fn from_index(index: usize) -> Self;
    fn index(self) -> usize;
}

/// Owns IR entities and provides stable typed IDs for them.
///
/// Removed entries become tombstones, and their IDs are never reused.
#[derive(Clone)]
pub(crate) struct Arena<I, T> {
    entries: Vec<Option<T>>,
    len: usize,
    id: PhantomData<fn() -> I>,
}

impl<I, T> Arena<I, T>
where
    I: ArenaId,
{
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            len: 0,
            id: PhantomData,
        }
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn alloc(&mut self, value: T) -> I {
        let id = I::from_index(self.entries.len());

        self.entries.push(Some(value));
        self.len += 1;

        id
    }

    /// Allocates an entity that needs to store its own stable ID.
    pub(crate) fn alloc_with_id(&mut self, create: impl FnOnce(I) -> T) -> I {
        let id = I::from_index(self.entries.len());

        self.entries.push(Some(create(id)));
        self.len += 1;

        id
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        self.entries.get(id.index())?.as_ref()
    }

    pub(crate) fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.entries.get_mut(id.index())?.as_mut()
    }

    pub(crate) fn remove(&mut self, id: I) -> Option<T> {
        let value = self.entries.get_mut(id.index())?.take()?;
        self.len -= 1;

        Some(value)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (I, &T)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|value| (I::from_index(index), value)))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (I, &mut T)> + '_ {
        self.entries
            .iter_mut()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_mut().map(|value| (I::from_index(index), value)))
    }
}

impl<I, T> Default for Arena<I, T>
where
    I: ArenaId,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::OperationId;

    use super::Arena;

    #[test]
    fn allocates_and_retrieves_entities() {
        let mut arena = Arena::<OperationId, _>::new();

        let first = arena.alloc("first");
        let second = arena.alloc("second");

        assert_eq!(arena.get(first), Some(&"first"));
        assert_eq!(arena.get(second), Some(&"second"));
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn makes_an_id_available_during_allocation() {
        let mut arena = Arena::<OperationId, _>::new();

        let operation = arena.alloc_with_id(|id| id.index());

        assert_eq!(arena.get(operation), Some(&operation.index()));
    }

    #[test]
    fn removes_without_reusing_the_id() {
        let mut arena = Arena::<OperationId, _>::new();

        let removed = arena.alloc("removed");
        let retained = arena.alloc("retained");

        assert_eq!(arena.remove(removed), Some("removed"));
        assert_eq!(arena.get(removed), None);
        assert_eq!(arena.get(retained), Some(&"retained"));
        assert_eq!(arena.len(), 1);

        let later = arena.alloc("later");

        assert_ne!(later, removed);
        assert_eq!(arena.get(later), Some(&"later"));
        assert_eq!(arena.len(), 2);
    }
}
