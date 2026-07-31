//! Compiler source files and IR provenance locations.

use rustc_hash::FxHashMap;

use crate::{LocationId, SourceFileId, arena::Arena};

/// A half-open range of UTF-8 byte offsets in one source file.
///
/// This uses the same coordinate system as Oxc spans. Line and column
/// coordinates are derived later according to the consuming format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: u32,
    end: u32,
}

impl TextRange {
    /// Creates a half-open byte range.
    pub fn new(start: u32, end: u32) -> Self {
        assert!(start <= end, "a text range cannot end before it starts");

        Self { start, end }
    }

    /// Returns the inclusive starting byte offset.
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the exclusive ending byte offset.
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Returns the range length in bytes.
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Returns whether the range contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Original compiler input retained for diagnostics and source maps.
#[derive(Debug)]
pub struct SourceFile {
    name: Box<str>,
    text: Box<str>,
}

impl SourceFile {
    fn new(name: impl Into<Box<str>>, text: impl Into<Box<str>>) -> Self {
        let text = text.into();

        assert!(
            u32::try_from(text.len()).is_ok(),
            "a source file cannot exceed u32::MAX UTF-8 bytes",
        );

        Self {
            name: name.into(),
            text,
        }
    }

    /// Returns the host-provided source name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the original UTF-8 source text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Why a location does not correspond directly to one source range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntheticReason {
    /// JavaScript semantics required IR that had no explicit syntax node.
    Implicit,

    /// One source construct was expanded into compiler IR.
    Desugared,

    /// Multiple existing constructs were combined into one.
    Combined,
}

/// Source provenance associated with compiler IR.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CompilerLocation {
    /// A concrete range in an original source file.
    Source {
        file: SourceFileId,
        range: TextRange,
    },

    /// Compiler-created IR derived from one or more existing locations.
    Synthetic {
        reason: SyntheticReason,
        origins: Box<[LocationId]>,
    },

    /// No meaningful provenance is known.
    Unknown,
}

/// Owns source files and canonical compiler locations for one compilation.
///
/// Operations store compact [`LocationId`] values. The database retains source
/// text and the provenance graph required to resolve those IDs.
pub struct SourceDatabase {
    files: Arena<SourceFileId, SourceFile>,
    locations: Arena<LocationId, CompilerLocation>,
    location_ids: FxHashMap<CompilerLocation, LocationId>,
}

impl SourceDatabase {
    /// Creates an empty database containing the canonical unknown location.
    pub fn new() -> Self {
        let mut locations = Arena::new();
        let unknown = locations.alloc(CompilerLocation::Unknown);

        assert_eq!(
            unknown,
            LocationId::UNKNOWN,
            "the unknown location must be allocated first",
        );

        let mut location_ids = FxHashMap::default();
        location_ids.insert(CompilerLocation::Unknown, unknown);

        Self {
            files: Arena::new(),
            locations,
            location_ids,
        }
    }

    /// Registers one compiler input.
    ///
    /// Each call creates a distinct source identity, even when the name or text
    /// matches an existing input. The host determines source identity.
    pub fn add_file(
        &mut self,
        name: impl Into<Box<str>>,
        text: impl Into<Box<str>>,
    ) -> SourceFileId {
        self.files.alloc(SourceFile::new(name, text))
    }

    /// Returns a registered source file.
    pub fn file(&self, file: SourceFileId) -> Option<&SourceFile> {
        self.files.get(file)
    }

    /// Returns a compiler location.
    pub fn location(&self, location: LocationId) -> Option<&CompilerLocation> {
        self.locations.get(location)
    }

    /// Returns the canonical location for a concrete source range.
    pub fn source_location(&mut self, file: SourceFileId, range: TextRange) -> LocationId {
        let source = self
            .files
            .get(file)
            .expect("a source location must reference a registered file");

        let start = range.start() as usize;
        let end = range.end() as usize;

        assert!(
            end <= source.text().len(),
            "a source location must remain within its source file",
        );
        assert!(
            source.text().is_char_boundary(start) && source.text().is_char_boundary(end),
            "a source location must use UTF-8 character boundaries",
        );

        self.intern(CompilerLocation::Source { file, range })
    }

    /// Returns a canonical location for compiler-created IR.
    pub fn synthetic_location(
        &mut self,
        reason: SyntheticReason,
        origins: impl IntoIterator<Item = LocationId>,
    ) -> LocationId {
        let origins = origins.into_iter().collect::<Box<[_]>>();

        assert!(
            !origins.is_empty(),
            "a synthetic location must have at least one origin",
        );

        for origin in &origins {
            assert!(
                self.locations.get(*origin).is_some(),
                "a synthetic location must reference existing origins",
            );
        }

        self.intern(CompilerLocation::Synthetic { reason, origins })
    }

    fn intern(&mut self, location: CompilerLocation) -> LocationId {
        if let Some(location) = self.location_ids.get(&location) {
            return *location;
        }

        let id = self.locations.alloc(location.clone());
        self.location_ids.insert(location, id);

        id
    }
}

impl Default for SourceDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::LocationId;

    use super::{CompilerLocation, SourceDatabase, SyntheticReason, TextRange};

    #[test]
    fn reserves_the_first_location_for_unknown_provenance() {
        let sources = SourceDatabase::new();

        assert_eq!(
            sources.location(LocationId::UNKNOWN),
            Some(&CompilerLocation::Unknown),
        );
    }

    #[test]
    fn canonicalizes_equal_source_locations() {
        let mut sources = SourceDatabase::new();
        let file = sources.add_file("entry.js", "const value = 42;");
        let range = TextRange::new(14, 16);

        let first = sources.source_location(file, range);
        let second = sources.source_location(file, range);

        assert_eq!(first, second);
        assert_eq!(
            sources.location(first),
            Some(&CompilerLocation::Source { file, range }),
        );
    }

    #[test]
    fn retains_all_origins_of_combined_ir() {
        let mut sources = SourceDatabase::new();
        let file = sources.add_file("entry.js", "(value + 1) + 2");

        let inner = sources.source_location(file, TextRange::new(1, 10));
        let outer = sources.source_location(file, TextRange::new(0, 15));
        let combined = sources.synthetic_location(SyntheticReason::Combined, [inner, outer]);

        assert_eq!(
            sources.location(combined),
            Some(&CompilerLocation::Synthetic {
                reason: SyntheticReason::Combined,
                origins: Box::new([inner, outer]),
            }),
        );
    }
}
