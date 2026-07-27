//! Logical JavaScript locals and their final names.

use crate::name::JsNameAllocator;

/// Identifies one backend-created JavaScript local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct JsLocalId(u32);

impl JsLocalId {
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a function cannot contain more than u32::MAX locals");

        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Allocates logical locals before they receive names.
#[derive(Debug, Default)]
pub(crate) struct JsLocalAllocator {
    count: usize,
}

impl JsLocalAllocator {
    pub(crate) fn allocate(&mut self) -> JsLocalId {
        let local = JsLocalId::from_index(self.count);

        self.count = self
            .count
            .checked_add(1)
            .expect("a function cannot contain more than usize::MAX locals");

        local
    }

    pub(crate) const fn count(&self) -> usize {
        self.count
    }
}

/// Final JavaScript names for one function's logical locals.
#[derive(Debug)]
pub(crate) struct JsNamePlan {
    locals: Vec<Box<str>>,
}

impl JsNamePlan {
    pub(crate) fn build(local_count: usize, mut allocator: JsNameAllocator<'_>) -> Self {
        let locals = (0..local_count)
            .map(|_| allocator.allocate_generated())
            .collect();

        Self { locals }
    }

    pub(crate) fn local(&self, id: JsLocalId) -> Option<&str> {
        self.locals.get(id.index()).map(Box::as_ref)
    }

    pub(crate) const fn len(&self) -> usize {
        self.locals.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::name::{JsNameAllocator, JsReservedNames};

    use super::{JsLocalAllocator, JsNamePlan};

    #[test]
    fn assigns_names_after_allocating_logical_locals() {
        let mut locals = JsLocalAllocator::default();
        let first = locals.allocate();
        let second = locals.allocate();

        let reserved = JsReservedNames::default();
        let names = JsNamePlan::build(locals.count(), JsNameAllocator::new(&reserved));

        assert_eq!(names.local(first), Some("$evrel0"));
        assert_eq!(names.local(second), Some("$evrel1"));
    }
}
