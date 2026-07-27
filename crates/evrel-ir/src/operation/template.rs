//! Structured JavaScript template-literal operations.

use crate::{JsString, RegionId, TemplateSiteId};

use super::{CallTarget, OperationEffects};

/// One static segment of a JavaScript template literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateQuasi {
    raw: Box<str>,
    cooked: Option<JsString>,
}

impl TemplateQuasi {
    /// Creates a template segment from its raw and interpreted text.
    pub fn new(raw: impl Into<Box<str>>, cooked: Option<JsString>) -> Self {
        Self {
            raw: raw.into(),
            cooked,
        }
    }

    /// Returns the uninterpreted source text between substitutions.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the segment's interpreted string value.
    pub const fn cooked(&self) -> Option<&JsString> {
        self.cooked.as_ref()
    }
}

/// Evaluates an untagged JavaScript template literal.
///
/// Each substitution region is evaluated and converted to a string before
/// evaluation advances to the following substitution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateLiteralOp {
    quasis: Box<[TemplateQuasi]>,
    substitutions: Box<[RegionId]>,
}

impl TemplateLiteralOp {
    /// Creates a template literal in source evaluation order.
    pub fn new(
        quasis: impl Into<Box<[TemplateQuasi]>>,
        substitutions: impl Into<Box<[RegionId]>>,
    ) -> Self {
        let quasis = quasis.into();
        let substitutions = substitutions.into();

        assert_eq!(
            quasis.len(),
            substitutions.len() + 1,
            "template literals require one more quasi than substitution"
        );
        assert!(
            quasis.iter().all(|quasi| quasi.cooked().is_some()),
            "untagged template literals require valid cooked segments"
        );

        Self {
            quasis,
            substitutions,
        }
    }

    /// Returns static segments in source order.
    pub fn quasis(&self) -> &[TemplateQuasi] {
        &self.quasis
    }

    /// Returns substitution regions in source order.
    pub fn substitutions(&self) -> &[RegionId] {
        &self.substitutions
    }

    /// Returns substitution regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.substitutions.to_vec()
    }

    /// Returns the intrinsic effects of assembling the template string.
    pub fn effects(&self) -> OperationEffects {
        if self.substitutions.is_empty() {
            OperationEffects::NONE
        } else {
            OperationEffects::MAY_THROW
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Calls a JavaScript tag with one cached template-object site.
///
/// Target operands are evaluated before the substitution regions. Substitution
/// values are passed to the tag without string conversion.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaggedTemplateOp {
    site: TemplateSiteId,
    target: CallTarget,
    quasis: Box<[TemplateQuasi]>,
    substitutions: Box<[RegionId]>,
}

impl TaggedTemplateOp {
    pub fn new(
        site: TemplateSiteId,
        target: CallTarget,
        quasis: impl Into<Box<[TemplateQuasi]>>,
        substitutions: impl Into<Box<[RegionId]>>,
    ) -> Self {
        let quasis = quasis.into();
        let substitutions = substitutions.into();

        assert_eq!(
            quasis.len(),
            substitutions.len() + 1,
            "tagged templates require one more quasi than substitution"
        );

        Self {
            site,
            target,
            quasis,
            substitutions,
        }
    }

    pub const fn site(&self) -> TemplateSiteId {
        self.site
    }

    pub const fn target(&self) -> &CallTarget {
        &self.target
    }

    pub fn quasis(&self) -> &[TemplateQuasi] {
        &self.quasis
    }

    pub fn substitutions(&self) -> &[RegionId] {
        &self.substitutions
    }

    pub fn regions(&self) -> Vec<RegionId> {
        self.substitutions.to_vec()
    }

    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::MAY_THROW
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.target.operand_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::{CallReceiver, CallTarget, JsString, RegionId, TemplateSiteId};

    use super::{TaggedTemplateOp, TemplateQuasi};

    #[test]
    fn preserves_tagged_template_site_target_and_substitutions() {
        let site = TemplateSiteId::from_index(3);
        let substitution = RegionId::from_index(4);
        let operation = TaggedTemplateOp::new(
            site,
            CallTarget::Value {
                receiver: CallReceiver::Explicit,
            },
            [
                TemplateQuasi::new("value: ", Some(JsString::new("value: ", false))),
                TemplateQuasi::new("", Some(JsString::new("", false))),
            ],
            [substitution],
        );

        assert_eq!(operation.site(), site);
        assert_eq!(operation.regions(), [substitution]);
        assert_eq!(operation.operand_count(), 2);
        assert_eq!(operation.result_count(), 1);
        assert!(operation.effects().may_throw());
    }

    #[test]
    fn permits_missing_cooked_text_for_tagged_templates() {
        let quasi = TemplateQuasi::new("\\unicode", None);

        assert_eq!(quasi.raw(), "\\unicode");
        assert_eq!(quasi.cooked(), None);
    }
}
