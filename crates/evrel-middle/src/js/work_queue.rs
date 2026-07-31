//! A deterministic queue for fixed-point work.

use std::collections::VecDeque;
use std::hash::Hash;

use rustc_hash::FxHashSet;

/// A FIFO queue that hols each pending item at most once.
///
/// Popping an item removes it from the pending set, allowing it to be queued
/// again if later analysis updates make revisiting it necessary.
pub(crate) struct WorkQueue<T> {
    queue: VecDeque<T>,
    queued: FxHashSet<T>,
}

impl<T> WorkQueue<T>
where
    T: Copy + Eq + Hash,
{
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            queued: FxHashSet::default(),
        }
    }

    /// Adds an item unless it is already waiting to be processed.
    pub(crate) fn push(&mut self, item: T) {
        if self.queued.insert(item) {
            self.queue.push_back(item);
        }
    }

    /// Removes the next item in insertion order.
    pub(crate) fn pop(&mut self) -> Option<T> {
        let item = self.queue.pop_front()?;
        self.queued.remove(&item);
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::WorkQueue;

    #[test]
    fn processes_items_in_insertion_order() {
        let mut queue = WorkQueue::new();

        queue.push(2);
        queue.push(1);

        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn holds_each_pending_item_once() {
        let mut queue = WorkQueue::new();

        queue.push(1);
        queue.push(1);

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn allows_processed_items_to_be_queued_again() {
        let mut queue = WorkQueue::new();

        queue.push(1);
        assert_eq!(queue.pop(), Some(1));

        queue.push(1);
        assert_eq!(queue.pop(), Some(1));
    }
}
