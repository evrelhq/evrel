use evrel_js_ir::{FunctionId, OperationId};
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};

const WORD_BITS: usize = u64::BITS as usize;
static NEXT_ANALYSIS_OWNER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct AnalysisOwner {
    generation: u64,
    function: FunctionId,
}

impl AnalysisOwner {
    pub(super) fn fresh(function: FunctionId) -> Self {
        let generation = NEXT_ANALYSIS_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("exhausted pointer-analysis owner identities");
        Self {
            generation,
            function,
        }
    }
}

/// Dense identity for one abstract object in a function analysis.
///
/// The ID is scoped to one [`FunctionPointerAnalysis`](super::FunctionPointerAnalysis)
/// result. IDs from another result are rejected, even when both results analyze
/// the same function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractObjectId {
    owner: AnalysisOwner,
    index: u32,
}

impl AbstractObjectId {
    pub(super) fn from_index(owner: AnalysisOwner, index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a function analysis cannot contain more than u32::MAX abstract objects");
        Self { owner, index }
    }

    /// Returns the function whose analysis created this object.
    pub const fn function(self) -> FunctionId {
        self.owner.function
    }

    pub(super) const fn owner(self) -> AnalysisOwner {
        self.owner
    }

    pub(super) const fn index(self) -> usize {
        self.index as usize
    }
}

/// Static origin represented by an abstract object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractObjectKind {
    /// An object produced by one allocation operation.
    Allocation(OperationId),

    /// The analyzed function's implicit `arguments` object.
    ArgumentsObject,

    /// The current module's stable `import.meta` object.
    ImportMeta,
}

impl AbstractObjectKind {
    pub(super) const fn is_stable_contextual_identity(self) -> bool {
        matches!(self, Self::ArgumentsObject | Self::ImportMeta)
    }
}

/// Metadata for one abstract JavaScript object.
///
/// An allocation operation inside a loop can represent multiple concrete
/// runtime objects. Consequently, equal allocation-site IDs prove possible
/// aliasing, but do not by themselves prove must-aliasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractObject {
    id: AbstractObjectId,
    kind: AbstractObjectKind,
}

impl AbstractObject {
    pub(super) const fn new(id: AbstractObjectId, kind: AbstractObjectKind) -> Self {
        Self { id, kind }
    }

    /// Returns this object's dense analysis-local identity.
    pub const fn id(self) -> AbstractObjectId {
        self.id
    }

    /// Returns the function whose analysis created this object.
    pub const fn function(self) -> FunctionId {
        self.id.function()
    }

    /// Returns the static origin represented by this object.
    pub const fn kind(self) -> AbstractObjectKind {
        self.kind
    }
}

/// The object component of an abstract JavaScript value.
///
/// Known objects are retained even after the value is widened to include an
/// unknown object. This is required for sound escape propagation through a
/// merge of local and external values.
///
/// An empty object set is not a JavaScript type fact. Consumers that need
/// primitive or scalar information should use `FunctionValueAnalysis`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointsToSet {
    owner: AnalysisOwner,
    objects: SparseBitSet,
    may_point_to_unknown_object: bool,
    may_be_primitive: bool,
}

impl PointsToSet {
    pub(super) fn bottom(owner: AnalysisOwner) -> Self {
        Self {
            owner,
            objects: SparseBitSet::new(),
            may_point_to_unknown_object: false,
            may_be_primitive: false,
        }
    }

    pub(super) fn primitive(owner: AnalysisOwner) -> Self {
        Self {
            owner,
            objects: SparseBitSet::new(),
            may_point_to_unknown_object: false,
            may_be_primitive: true,
        }
    }

    pub(super) fn unknown(owner: AnalysisOwner) -> Self {
        Self {
            owner,
            objects: SparseBitSet::new(),
            may_point_to_unknown_object: true,
            may_be_primitive: true,
        }
    }

    pub(super) fn unknown_object(owner: AnalysisOwner) -> Self {
        Self {
            owner,
            objects: SparseBitSet::new(),
            may_point_to_unknown_object: true,
            may_be_primitive: false,
        }
    }

    pub(super) fn singleton(object: AbstractObjectId) -> Self {
        let mut objects = SparseBitSet::new();
        objects.insert(object.index());

        Self {
            owner: object.owner(),
            objects,
            may_point_to_unknown_object: false,
            may_be_primitive: false,
        }
    }

    /// Iterates over every known abstract object ID.
    pub fn objects(&self) -> impl Iterator<Item = AbstractObjectId> + '_ {
        self.objects
            .iter()
            .map(|index| AbstractObjectId::from_index(self.owner, index))
    }

    /// Returns whether the value may point to an unmodelled object.
    pub const fn may_point_to_unknown_object(&self) -> bool {
        self.may_point_to_unknown_object
    }

    /// Returns whether this set contains a particular known object.
    pub fn contains(&self, object: AbstractObjectId) -> bool {
        object.owner() == self.owner && self.objects.contains(object.index())
    }

    pub(super) fn owner(&self) -> AnalysisOwner {
        self.owner
    }

    pub(super) fn has_known_objects(&self) -> bool {
        !self.objects.is_empty()
    }

    pub(super) fn may_be_object(&self) -> bool {
        self.may_point_to_unknown_object || self.has_known_objects()
    }

    pub(super) fn is_definitely_object(&self) -> bool {
        !self.may_be_primitive && self.may_be_object()
    }

    pub(super) fn sole_known_object(&self) -> Option<AbstractObjectId> {
        if self.may_point_to_unknown_object {
            return None;
        }

        self.objects
            .sole_index()
            .map(|index| AbstractObjectId::from_index(self.owner, index))
    }

    /// Unions another fact and returns only newly discovered information.
    pub(super) fn join_delta(&mut self, other: &Self) -> Self {
        assert_eq!(
            self.owner, other.owner,
            "cannot join points-to facts from different analysis results"
        );

        let mut delta = Self::bottom(self.owner);
        self.objects.union_delta(&other.objects, &mut delta.objects);

        if other.may_point_to_unknown_object && !self.may_point_to_unknown_object {
            self.may_point_to_unknown_object = true;
            delta.may_point_to_unknown_object = true;
        }

        if other.may_be_primitive && !self.may_be_primitive {
            self.may_be_primitive = true;
            delta.may_be_primitive = true;
        }

        delta
    }

    pub(super) fn known_objects_overlap(&self, other: &Self) -> bool {
        self.owner == other.owner && self.objects.intersects(&other.objects)
    }

    pub(super) fn union_objects_into(&self, target: &mut SparseBitSet) {
        target.union(&self.objects);
    }

    pub(super) fn is_bottom(&self) -> bool {
        self.objects.is_empty() && !self.may_point_to_unknown_object && !self.may_be_primitive
    }
}

/// A bitset that stores only nonzero 64-bit words.
///
/// Points-to facts are commonly sparse (especially allocation singletons), so
/// this retains word-level set operations without allocating every zero word
/// below the highest object ID or repeatedly copying a growing vector.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SparseBitSet {
    words: FxHashMap<u32, u64>,
}

impl SparseBitSet {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn insert(&mut self, index: usize) -> bool {
        let word_index = u32::try_from(index / WORD_BITS)
            .expect("an abstract-object bit index cannot exceed u32::MAX words");
        let bit = 1_u64 << (index % WORD_BITS);
        self.insert_word(word_index, bit) != 0
    }

    pub(super) fn contains(&self, index: usize) -> bool {
        let Ok(word_index) = u32::try_from(index / WORD_BITS) else {
            return false;
        };
        let bit = 1_u64 << (index % WORD_BITS);
        self.words
            .get(&word_index)
            .is_some_and(|bits| bits & bit != 0)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub(super) fn sole_index(&self) -> Option<usize> {
        if self.words.len() != 1 {
            return None;
        }
        let (&word_index, &bits) = self.words.iter().next()?;
        (bits.count_ones() == 1)
            .then(|| word_index as usize * WORD_BITS + bits.trailing_zeros() as usize)
    }

    pub(super) fn intersects(&self, other: &Self) -> bool {
        let (smaller, larger) = if self.words.len() <= other.words.len() {
            (self, other)
        } else {
            (other, self)
        };
        smaller.words.iter().any(|(index, bits)| {
            larger
                .words
                .get(index)
                .is_some_and(|other| bits & other != 0)
        })
    }

    pub(super) fn union(&mut self, other: &Self) {
        for (&index, &bits) in &other.words {
            self.insert_word(index, bits);
        }
    }

    pub(super) fn union_delta(&mut self, other: &Self, delta: &mut Self) {
        debug_assert!(delta.is_empty());
        for (&index, &bits) in &other.words {
            let new_bits = self.insert_word(index, bits);
            if new_bits != 0 {
                delta.insert_word(index, new_bits);
            }
        }
    }

    pub(super) fn iter(&self) -> SparseBitSetIter<'_> {
        SparseBitSetIter {
            words: self.words.iter(),
            current_word: 0,
            remaining: 0,
        }
    }

    fn insert_word(&mut self, index: u32, bits: u64) -> u64 {
        debug_assert_ne!(bits, 0);
        let current = self.words.entry(index).or_default();
        let new_bits = bits & !*current;
        *current |= bits;
        new_bits
    }
}

pub(super) struct SparseBitSetIter<'a> {
    words: std::collections::hash_map::Iter<'a, u32, u64>,
    current_word: u32,
    remaining: u64,
}

impl Iterator for SparseBitSetIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.remaining != 0 {
                let bit = self.remaining.trailing_zeros() as usize;
                self.remaining &= self.remaining - 1;
                return Some(self.current_word as usize * WORD_BITS + bit);
            }

            let (word, bits) = self.words.next()?;
            (self.current_word, self.remaining) = (*word, *bits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SparseBitSet;

    fn sorted_indices(set: &SparseBitSet) -> Vec<usize> {
        let mut indices = set.iter().collect::<Vec<_>>();
        indices.sort_unstable();
        indices
    }

    #[test]
    fn sparse_bitsets_report_only_new_union_bits() {
        let mut current = SparseBitSet::new();
        current.insert(1);
        current.insert(130);

        let mut incoming = SparseBitSet::new();
        incoming.insert(1);
        incoming.insert(64);
        incoming.insert(130);

        let mut delta = SparseBitSet::new();
        current.union_delta(&incoming, &mut delta);

        assert_eq!(sorted_indices(&current), vec![1, 64, 130]);
        assert_eq!(sorted_indices(&delta), vec![64]);
    }

    #[test]
    fn sparse_bitsets_do_not_store_leading_zero_words() {
        let mut set = SparseBitSet::new();
        set.insert(1_000_000);

        assert_eq!(set.words.len(), 1);
        assert_eq!(sorted_indices(&set), vec![1_000_000]);
    }

    #[test]
    fn sparse_bitsets_merge_interleaved_words() {
        let mut current = SparseBitSet::new();
        current.insert(64);
        current.insert(192);

        let mut incoming = SparseBitSet::new();
        incoming.insert(1);
        incoming.insert(128);

        let mut delta = SparseBitSet::new();
        current.union_delta(&incoming, &mut delta);

        assert_eq!(sorted_indices(&current), vec![1, 64, 128, 192]);
        assert_eq!(sorted_indices(&delta), vec![1, 128]);
        assert!(current.intersects(&incoming));
    }
}
