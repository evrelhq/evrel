//! JavaScript global operations.

/// Reads the value of an unresolved JavaScript identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadGlobalOp {
    name: Box<str>,
}

impl LoadGlobalOp {
    /// Creates a global-read operation.
    pub fn new(name: impl Into<Box<str>>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the unresolved identifier name.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Writes the value of an unresolved JavaScript identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoreGlobalOp {
    name: Box<str>,
}

impl StoreGlobalOp {
    /// Creates a global-write operation.
    pub fn new(name: impl Into<Box<str>>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the unresolved identifier name.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{LoadGlobalOp, StoreGlobalOp};

    #[test]
    fn stores_the_global_name() {
        let operation = LoadGlobalOp::new("console");

        assert_eq!(operation.name(), "console");
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn stores_the_written_global_name() {
        let operation = StoreGlobalOp::new("result");

        assert_eq!(operation.name(), "result");
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }
}
