//! Constant-producing operations.

/// An owned JavaScript string.
///
/// Oxc uses an encoded UTF-8 representation for lone UTF-16 surrogates and
/// records whether that encoding is present.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsString {
    value: Box<str>,
    lone_surrogates: bool,
}

impl JsString {
    /// Creates a JavaScript string from Oxc's decoded representation.
    pub fn new(value: impl Into<Box<str>>, lone_surrogates: bool) -> Self {
        Self {
            value: value.into(),
            lone_surrogates,
        }
    }

    /// Returns Oxc's decoded string representation.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Returns whether the representation contains encoded lone surrogates.
    pub const fn has_lone_surrogates(&self) -> bool {
        self.lone_surrogates
    }
}

/// A constant JavaScript value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    /// The ECMAScript undefined value.
    Undefined,

    /// An ECMAScript boolean.
    Boolean(bool),

    /// The ECMAScript null value.
    Null,

    /// An ECMAScript number.
    Number(f64),

    /// An arbitrary-precision ECMAScript integer in canonical decimal form.
    BigInt(Box<str>),

    /// An ECMAScript string.
    String(JsString),
}

/// Produces a constant value.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstantOp {
    value: ConstantValue,
}

impl ConstantOp {
    /// Creates a constant-producing operation.
    pub const fn new(value: ConstantValue) -> Self {
        Self { value }
    }

    /// Returns the produced constant value.
    pub const fn value(&self) -> &ConstantValue {
        &self.value
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}
