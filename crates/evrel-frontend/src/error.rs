//! Errors produced while parsing and lowering JavaScript.

use thiserror::Error;

/// Failure to translate JavaScript source into Evrel IR.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrontendError {
    /// The source name does not have a supported extension.
    #[error("cannot determine the source type for `{source_name}`: {reason}")]
    UnknownSourceType {
        /// Source name provided by the caller.
        source_name: Box<str>,

        /// Explanation returned by source-type detection.
        reason: Box<str>,
    },

    /// Oxc reported one or more parse diagnostics.
    #[error("JavaScript parsing failed")]
    Parse {
        /// Parser diagnostics in source order.
        diagnostics: Vec<String>,
    },

    /// Oxc reported one or more semantic diagnostics.
    #[error("JavaScript semantic analysis failed")]
    Semantic {
        /// Semantic diagnostics in source order.
        diagnostics: Vec<String>,
    },

    /// The frontend does not yet support a statement form.
    #[error("unsupported JavaScript statement")]
    UnsupportedStatement,

    /// The frontend does not yet support an expression form.
    #[error("unsupported JavaScript expression")]
    UnsupportedExpression,

    /// A JSX attribute used an empty expression container.
    #[error("JSX attribute expression cannot be empty")]
    EmptyJsxAttributeExpression,

    /// The frontend does not yet support this declaration category.
    #[error("unsupported declaration for binding `{name}`")]
    UnsupportedDeclaration {
        /// Name of the unsupported binding.
        name: Box<str>,
    },

    /// TypeScript parameter properties require runtime field initialization.
    #[error("TypeScript parameter properties are not supported")]
    UnsupportedParameterProperty,

    /// The frontend does not yet support this variable declaration kind.
    #[error("unsupported variable declaration kind `{kind}`")]
    UnsupportedVariableDeclarationKind {
        /// JavaScript spelling of the declaration kind.
        kind: Box<str>,
    },

    /// A binding pattern cannot be represented by Evrel IR.
    #[error("invalid JavaScript binding pattern")]
    InvalidBindingPattern,

    /// A declaration requiring an initializer did not have one.
    #[error("binding `{name}` requires an initializer")]
    MissingBindingInitializer {
        /// Name of the binding.
        name: Box<str>,
    },

    /// A destructuring declaration omitted its required initializer.
    #[error("destructuring declaration requires an initializer")]
    MissingDestructuringInitializer,

    /// The frontend does not yet support this assignment operator.
    #[error("unsupported JavaScript assignment operator")]
    UnsupportedAssignmentOperator,

    /// The frontend does not yet support this assignment target.
    #[error("unsupported JavaScript assignment target")]
    UnsupportedAssignmentTarget,

    /// Optional-chain syntax appeared outside its chain boundary.
    #[error("invalid optional-chain structure")]
    InvalidOptionalChain,
}
