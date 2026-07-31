//! Construction-time JavaScript function properties.

use super::FunctionKind;

/// Semantic and source-preservation properties fixed when a function is
/// created.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FunctionProperties {
    strict: bool,
    use_strict_directive: bool,
}

impl FunctionProperties {
    /// Creates properties for an implicitly strict execution context.
    pub const fn strict() -> Self {
        Self {
            strict: true,
            use_strict_directive: false,
        }
    }

    /// Records an authored `"use strict"` directive.
    ///
    /// Directive provenance is independent from inherited or intrinsic
    /// strictness because code generation must preserve authored directives.
    pub const fn with_use_strict_directive(self) -> Self {
        Self {
            strict: true,
            use_strict_directive: true,
        }
    }

    /// Returns whether the function executes with strict-mode semantics.
    pub const fn is_strict(self) -> bool {
        self.strict
    }

    /// Returns whether code generation must preserve an explicit
    /// `"use strict"` directive.
    pub const fn has_use_strict_directive(self) -> bool {
        self.use_strict_directive
    }

    pub(crate) const fn resolve(self, kind: FunctionKind, parent_is_strict: bool) -> Self {
        Self {
            strict: self.strict || parent_is_strict || kind.is_intrinsically_strict(),
            use_strict_directive: self.use_strict_directive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FunctionKind, FunctionProperties};

    #[test]
    fn resolves_every_strictness_source_at_construction() {
        let intrinsic =
            FunctionProperties::default().resolve(FunctionKind::ClassStaticBlock, false);
        let inherited = FunctionProperties::default().resolve(FunctionKind::Ordinary, true);
        let explicit = FunctionProperties::default()
            .with_use_strict_directive()
            .resolve(FunctionKind::Ordinary, false);

        assert!(intrinsic.is_strict());
        assert!(inherited.is_strict());
        assert!(explicit.is_strict());
        assert!(!intrinsic.has_use_strict_directive());
        assert!(!inherited.has_use_strict_directive());
        assert!(explicit.has_use_strict_directive());
    }
}
