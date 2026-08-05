//! Function-local pointer analysis for JavaScript values.

mod points_to;
mod solver;

#[cfg(test)]
mod tests;

use evrel_js_ir::{FunctionId, JsModuleIr, ValueId};
use rustc_hash::FxHashMap;

pub use points_to::{AbstractObject, AbstractObjectId, AbstractObjectKind, PointsToSet};

use points_to::{AnalysisOwner, SparseBitSet};
use solver::PointerSolver;

/// Relationship between the object identities denoted by two SSA values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AliasResult {
    /// The values cannot denote the same runtime object.
    NoAlias,

    /// The values may denote the same runtime object.
    MayAlias,

    /// The values are proven to denote the same runtime object.
    MustAlias,
}

/// Whether a tracked abstract object may leave the analyzed activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscapeResult {
    /// The object remains within the analyzed activation.
    DoesNotEscape,

    /// The object may become reachable outside the analyzed activation.
    MayEscape,
}

/// Immutable pointer, alias, and escape facts for one function snapshot.
///
/// The analysis is flow-insensitive and allocation-site based. Unknown calls
/// and JavaScript operations that may retain an operand are conservative
/// escape boundaries. Recompute after changing the function, its control flow,
/// or its lexical binding relationships.
#[derive(Debug, Clone)]
pub struct FunctionPointerAnalysis {
    owner: AnalysisOwner,
    function: FunctionId,
    points_to: FxHashMap<ValueId, PointsToSet>,
    objects: Vec<AbstractObject>,
    escaping_objects: SparseBitSet,
}

impl FunctionPointerAnalysis {
    /// Computes function-local points-to, alias, and escape facts.
    pub fn analyze(module: &JsModuleIr, function: FunctionId) -> Option<Self> {
        let function_ir = module.function(function)?;
        let result = PointerSolver::analyze(module, function, function_ir);

        Some(Self {
            owner: result.owner,
            function,
            points_to: result.points_to,
            objects: result.objects,
            escaping_objects: result.escaping_objects,
        })
    }

    /// Returns the analyzed function.
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    /// Returns the object points-to fact for a live value in the function.
    ///
    /// Use `FunctionValueAnalysis` instead when primitive type information is
    /// required.
    pub fn points_to(&self, value: ValueId) -> Option<&PointsToSet> {
        self.points_to.get(&value)
    }

    /// Returns metadata for an object tracked by this analysis.
    pub fn object(&self, object: AbstractObjectId) -> Option<&AbstractObject> {
        (object.owner() == self.owner)
            .then(|| self.objects.get(object.index()))
            .flatten()
    }

    /// Determines whether two values may denote the same runtime object.
    pub fn alias(&self, left: ValueId, right: ValueId) -> AliasResult {
        let (Some(left_points_to), Some(right_points_to)) =
            (self.points_to(left), self.points_to(right))
        else {
            return AliasResult::MayAlias;
        };

        if !left_points_to.may_be_object() || !right_points_to.may_be_object() {
            return AliasResult::NoAlias;
        }

        if left == right && left_points_to.is_definitely_object() {
            return AliasResult::MustAlias;
        }

        let left_object = left_points_to.sole_known_object();
        let right_object = right_points_to.sole_known_object();
        if left_points_to.is_definitely_object()
            && right_points_to.is_definitely_object()
            && left_object == right_object
            && left_object.is_some_and(|object| {
                self.object(object)
                    .is_some_and(|object| object.kind().is_stable_contextual_identity())
            })
        {
            return AliasResult::MustAlias;
        }

        if left_points_to.known_objects_overlap(right_points_to)
            || (left_points_to.may_point_to_unknown_object() && right_points_to.may_be_object())
            || (right_points_to.may_point_to_unknown_object() && left_points_to.may_be_object())
        {
            AliasResult::MayAlias
        } else {
            AliasResult::NoAlias
        }
    }

    /// Conservatively answers whether two values may denote the same object.
    pub fn may_alias(&self, left: ValueId, right: ValueId) -> bool {
        self.alias(left, right) != AliasResult::NoAlias
    }

    /// Returns the escape result for an object tracked by this analysis.
    ///
    /// Returns `None` when the object does not belong to this analysis result.
    pub fn object_escape_result(&self, object: AbstractObjectId) -> Option<EscapeResult> {
        self.object(object).map(|_| {
            if self.escaping_objects.contains(object.index()) {
                EscapeResult::MayEscape
            } else {
                EscapeResult::DoesNotEscape
            }
        })
    }

    /// Conservatively answers whether a tracked object may escape.
    ///
    /// Objects that do not belong to this analysis return `true`.
    pub fn object_may_escape(&self, object: AbstractObjectId) -> bool {
        self.object_escape_result(object) != Some(EscapeResult::DoesNotEscape)
    }

    /// Returns the escape result for known objects denoted by a value.
    ///
    /// `None` means that the value has no known abstract object. Use
    /// [`Self::may_escape`] when a conservative boolean answer is required.
    pub fn escape_result(&self, value: ValueId) -> Option<EscapeResult> {
        let points_to = self.points_to(value)?;
        points_to.has_known_objects().then(|| {
            if points_to.may_point_to_unknown_object()
                || points_to
                    .objects()
                    .any(|object| self.escaping_objects.contains(object.index()))
            {
                EscapeResult::MayEscape
            } else {
                EscapeResult::DoesNotEscape
            }
        })
    }

    /// Conservatively answers whether an object denoted by a value may escape.
    pub fn may_escape(&self, value: ValueId) -> bool {
        let Some(points_to) = self.points_to(value) else {
            return true;
        };

        points_to.may_point_to_unknown_object()
            || points_to
                .objects()
                .any(|object| self.escaping_objects.contains(object.index()))
    }
}
