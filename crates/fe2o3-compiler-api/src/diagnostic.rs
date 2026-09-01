//! Stable, bounded compiler diagnostics.

use core::fmt;

use crate::{CompilerStageV1, DiagnosticSubjectIdentityV1};

/// Maximum UTF-8 byte length of a V1 diagnostic message.
pub const MAX_DIAGNOSTIC_MESSAGE_BYTES_V1: usize = 4 * 1024;

/// Maximum UTF-8 byte length of a presentation-only source name.
pub const MAX_DIAGNOSTIC_SOURCE_NAME_BYTES_V1: usize = 4 * 1024;
/// Maximum number of retained call frames in one presentation diagnostic.
pub const MAX_DIAGNOSTIC_CALL_FRAMES_V1: usize = 64;
/// Maximum number of retained notes in one presentation diagnostic.
pub const MAX_DIAGNOSTIC_NOTES_V1: usize = 32;

/// Why a diagnostic code was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCodeErrorV1 {
    /// Zero is reserved for the absence of a diagnostic code.
    Zero,
}

impl fmt::Display for DiagnosticCodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("diagnostic code must be nonzero"),
        }
    }
}

impl std::error::Error for DiagnosticCodeErrorV1 {}

/// Stable, producer-owned numeric diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DiagnosticCodeV1(u32);

impl DiagnosticCodeV1 {
    /// Creates a nonzero diagnostic code.
    pub const fn new(code: u32) -> Result<Self, DiagnosticCodeErrorV1> {
        if code == 0 {
            return Err(DiagnosticCodeErrorV1::Zero);
        }
        Ok(Self(code))
    }

    /// Returns the numeric code.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Why diagnostic presentation text was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticMessageErrorV1 {
    /// The message was empty.
    Empty,
    /// The message exceeds the hard V1 byte limit.
    TooLong {
        /// Observed UTF-8 byte length.
        actual: usize,
        /// Maximum admitted UTF-8 byte length.
        maximum: usize,
    },
    /// Leading or trailing whitespace would make presentation noncanonical.
    SurroundingWhitespace,
    /// NUL or carriage return is not admitted in canonical presentation text.
    UnsupportedControlCharacter,
}

impl fmt::Display for DiagnosticMessageErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("diagnostic message must not be empty"),
            Self::TooLong { actual, maximum } => write!(
                formatter,
                "diagnostic message is {actual} bytes, exceeding the {maximum}-byte limit"
            ),
            Self::SurroundingWhitespace => {
                formatter.write_str("diagnostic message has surrounding whitespace")
            }
            Self::UnsupportedControlCharacter => {
                formatter.write_str("diagnostic message contains an unsupported control character")
            }
        }
    }
}

impl std::error::Error for DiagnosticMessageErrorV1 {}

/// Checked UTF-8 presentation text for one diagnostic.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticMessageV1(String);

impl DiagnosticMessageV1 {
    /// Validates and owns diagnostic presentation text.
    pub fn new(message: impl Into<String>) -> Result<Self, DiagnosticMessageErrorV1> {
        let message = message.into();
        if message.is_empty() {
            return Err(DiagnosticMessageErrorV1::Empty);
        }
        if message.len() > MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 {
            return Err(DiagnosticMessageErrorV1::TooLong {
                actual: message.len(),
                maximum: MAX_DIAGNOSTIC_MESSAGE_BYTES_V1,
            });
        }
        if message.trim() != message {
            return Err(DiagnosticMessageErrorV1::SurroundingWhitespace);
        }
        if message.contains(['\0', '\r']) {
            return Err(DiagnosticMessageErrorV1::UnsupportedControlCharacter);
        }
        Ok(Self(message))
    }

    /// Borrows the validated text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Severity of a canonical compiler diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum DiagnosticSeverityV1 {
    /// Compilation cannot produce a successful result.
    Error = 1,
    /// Compilation may proceed, but a deterministic concern was reported.
    Warning = 2,
    /// Informational context without success or failure semantics.
    Note = 3,
}

/// Deterministically ordered diagnostic record.
///
/// Paths, timestamps, process IDs, compiler object handles, and formatted
/// source spans are deliberately absent. The subject is an untrusted semantic
/// commitment and does not authenticate the diagnosed entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDiagnosticV1 {
    sequence: u16,
    code: DiagnosticCodeV1,
    severity: DiagnosticSeverityV1,
    stage: Option<CompilerStageV1>,
    subject: Option<DiagnosticSubjectIdentityV1>,
    message: DiagnosticMessageV1,
}

impl CanonicalDiagnosticV1 {
    /// Creates one checked diagnostic record.
    pub const fn new(
        sequence: u16,
        code: DiagnosticCodeV1,
        severity: DiagnosticSeverityV1,
        stage: Option<CompilerStageV1>,
        subject: Option<DiagnosticSubjectIdentityV1>,
        message: DiagnosticMessageV1,
    ) -> Self {
        Self {
            sequence,
            code,
            severity,
            stage,
            subject,
            message,
        }
    }

    /// Returns the zero-based diagnostic sequence number.
    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Returns the stable diagnostic code.
    pub const fn code(&self) -> DiagnosticCodeV1 {
        self.code
    }

    /// Returns the diagnostic severity.
    pub const fn severity(&self) -> DiagnosticSeverityV1 {
        self.severity
    }

    /// Returns the stage that emitted the diagnostic, when stage-specific.
    pub const fn stage(&self) -> Option<CompilerStageV1> {
        self.stage
    }

    /// Returns the diagnosed semantic subject, when one is available.
    pub const fn subject(&self) -> Option<DiagnosticSubjectIdentityV1> {
        self.subject
    }

    /// Returns the bounded presentation text.
    pub const fn message(&self) -> &DiagnosticMessageV1 {
        &self.message
    }

    /// Reports whether this diagnostic rejects a successful output.
    pub const fn is_error(&self) -> bool {
        matches!(self.severity, DiagnosticSeverityV1::Error)
    }
}

/// A source coordinate used only for diagnostic presentation.
///
/// Lines are one-based. Columns are producer-defined and may be zero-based.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticSourcePositionV1 {
    line: u32,
    column: u32,
}

impl DiagnosticSourcePositionV1 {
    /// Constructs a valid source coordinate.
    pub const fn new(line: u32, column: u32) -> Option<Self> {
        if line == 0 {
            return None;
        }
        Some(Self { line, column })
    }

    /// Returns the one-based line.
    pub const fn line(self) -> u32 {
        self.line
    }

    /// Returns the producer-defined column.
    pub const fn column(self) -> u32 {
        self.column
    }
}

/// Validation failure for presentation-only diagnostic context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredDiagnosticErrorV1 {
    /// The source name was empty.
    EmptySourceName,
    /// The source name exceeded its hard byte limit.
    SourceNameTooLong {
        /// Observed UTF-8 byte length.
        actual: usize,
        /// Maximum admitted UTF-8 byte length.
        maximum: usize,
    },
    /// The source name contained a control character unsafe for presentation.
    UnsupportedSourceNameCharacter,
    /// The end coordinate preceded the start coordinate.
    ReversedSourceSpan,
    /// The call chain exceeded its hard frame limit.
    TooManyCallFrames {
        /// Observed frame count.
        actual: usize,
        /// Maximum admitted frame count.
        maximum: usize,
    },
    /// The note collection exceeded its hard item limit.
    TooManyNotes {
        /// Observed note count.
        actual: usize,
        /// Maximum admitted note count.
        maximum: usize,
    },
}

impl fmt::Display for StructuredDiagnosticErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceName => {
                formatter.write_str("diagnostic source name must not be empty")
            }
            Self::SourceNameTooLong { actual, maximum } => write!(
                formatter,
                "diagnostic source name is {actual} bytes, exceeding the {maximum}-byte limit"
            ),
            Self::UnsupportedSourceNameCharacter => formatter
                .write_str("diagnostic source name contains an unsupported control character"),
            Self::ReversedSourceSpan => {
                formatter.write_str("diagnostic source span ends before it starts")
            }
            Self::TooManyCallFrames { actual, maximum } => write!(
                formatter,
                "diagnostic has {actual} call frames, exceeding the {maximum}-frame limit"
            ),
            Self::TooManyNotes { actual, maximum } => write!(
                formatter,
                "diagnostic has {actual} notes, exceeding the {maximum}-note limit"
            ),
        }
    }
}

impl std::error::Error for StructuredDiagnosticErrorV1 {}

/// A checked source span for local presentation. It is deliberately excluded
/// from canonical artifact identities so builds remain location-independent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticSourceSpanV1 {
    source_name: String,
    start: DiagnosticSourcePositionV1,
    end: DiagnosticSourcePositionV1,
}

impl DiagnosticSourceSpanV1 {
    /// Validates and owns a presentation source name and ordered span.
    pub fn new(
        source_name: impl Into<String>,
        start: DiagnosticSourcePositionV1,
        end: DiagnosticSourcePositionV1,
    ) -> Result<Self, StructuredDiagnosticErrorV1> {
        let source_name = source_name.into();
        if source_name.is_empty() {
            return Err(StructuredDiagnosticErrorV1::EmptySourceName);
        }
        if source_name.len() > MAX_DIAGNOSTIC_SOURCE_NAME_BYTES_V1 {
            return Err(StructuredDiagnosticErrorV1::SourceNameTooLong {
                actual: source_name.len(),
                maximum: MAX_DIAGNOSTIC_SOURCE_NAME_BYTES_V1,
            });
        }
        if source_name.contains(['\0', '\r', '\n']) {
            return Err(StructuredDiagnosticErrorV1::UnsupportedSourceNameCharacter);
        }
        if end < start {
            return Err(StructuredDiagnosticErrorV1::ReversedSourceSpan);
        }
        Ok(Self {
            source_name,
            start,
            end,
        })
    }

    /// Borrows the presentation source name.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Returns the inclusive start coordinate.
    pub const fn start(&self) -> DiagnosticSourcePositionV1 {
        self.start
    }

    /// Returns the inclusive end coordinate.
    pub const fn end(&self) -> DiagnosticSourcePositionV1 {
        self.end
    }
}

/// One bounded frame in a root-to-failure call chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCallFrameV1 {
    function: DiagnosticMessageV1,
    call_site: Option<DiagnosticSourceSpanV1>,
}

impl DiagnosticCallFrameV1 {
    /// Constructs a checked function frame with an optional call-site span.
    pub const fn new(
        function: DiagnosticMessageV1,
        call_site: Option<DiagnosticSourceSpanV1>,
    ) -> Self {
        Self {
            function,
            call_site,
        }
    }

    /// Returns the function name.
    pub const fn function(&self) -> &DiagnosticMessageV1 {
        &self.function
    }

    /// Returns the call-site span when the caller supplied one.
    pub const fn call_site(&self) -> Option<&DiagnosticSourceSpanV1> {
        self.call_site.as_ref()
    }
}

/// Rich local presentation for a canonical diagnostic.
///
/// This record is never hashed into compiler artifacts. It keeps filesystem
/// names and call paths out of reproducible identities while exposing enough
/// structure for a CLI, IDE, or JSON renderer to produce actionable errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredCompilerDiagnosticV1 {
    canonical: CanonicalDiagnosticV1,
    kernel: Option<DiagnosticMessageV1>,
    function: Option<DiagnosticMessageV1>,
    source_span: Option<DiagnosticSourceSpanV1>,
    call_chain: Box<[DiagnosticCallFrameV1]>,
    notes: Box<[DiagnosticMessageV1]>,
}

impl StructuredCompilerDiagnosticV1 {
    /// Constructs a bounded presentation diagnostic around a canonical record.
    pub fn new(
        canonical: CanonicalDiagnosticV1,
        kernel: Option<DiagnosticMessageV1>,
        function: Option<DiagnosticMessageV1>,
        source_span: Option<DiagnosticSourceSpanV1>,
        call_chain: Vec<DiagnosticCallFrameV1>,
        notes: Vec<DiagnosticMessageV1>,
    ) -> Result<Self, StructuredDiagnosticErrorV1> {
        if call_chain.len() > MAX_DIAGNOSTIC_CALL_FRAMES_V1 {
            return Err(StructuredDiagnosticErrorV1::TooManyCallFrames {
                actual: call_chain.len(),
                maximum: MAX_DIAGNOSTIC_CALL_FRAMES_V1,
            });
        }
        if notes.len() > MAX_DIAGNOSTIC_NOTES_V1 {
            return Err(StructuredDiagnosticErrorV1::TooManyNotes {
                actual: notes.len(),
                maximum: MAX_DIAGNOSTIC_NOTES_V1,
            });
        }
        Ok(Self {
            canonical,
            kernel,
            function,
            source_span,
            call_chain: call_chain.into_boxed_slice(),
            notes: notes.into_boxed_slice(),
        })
    }

    /// Returns the reproducible canonical diagnostic.
    pub const fn canonical(&self) -> &CanonicalDiagnosticV1 {
        &self.canonical
    }

    /// Returns the affected kernel name when known.
    pub const fn kernel(&self) -> Option<&DiagnosticMessageV1> {
        self.kernel.as_ref()
    }

    /// Returns the affected function name when known.
    pub const fn function(&self) -> Option<&DiagnosticMessageV1> {
        self.function.as_ref()
    }

    /// Returns the primary presentation span when known.
    pub const fn source_span(&self) -> Option<&DiagnosticSourceSpanV1> {
        self.source_span.as_ref()
    }

    /// Returns the root-to-failure call chain.
    pub const fn call_chain(&self) -> &[DiagnosticCallFrameV1] {
        &self.call_chain
    }

    /// Returns bounded supplemental notes.
    pub const fn notes(&self) -> &[DiagnosticMessageV1] {
        &self.notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_codes_are_nonzero() {
        assert_eq!(DiagnosticCodeV1::new(0), Err(DiagnosticCodeErrorV1::Zero));
        assert_eq!(DiagnosticCodeV1::new(7).unwrap().get(), 7);
    }

    #[test]
    fn messages_reject_empty_oversized_and_noncanonical_text() {
        assert_eq!(
            DiagnosticMessageV1::new(""),
            Err(DiagnosticMessageErrorV1::Empty)
        );
        assert_eq!(
            DiagnosticMessageV1::new("x".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 + 1)),
            Err(DiagnosticMessageErrorV1::TooLong {
                actual: MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 + 1,
                maximum: MAX_DIAGNOSTIC_MESSAGE_BYTES_V1,
            })
        );
        assert_eq!(
            DiagnosticMessageV1::new(" leading"),
            Err(DiagnosticMessageErrorV1::SurroundingWhitespace)
        );
        assert_eq!(
            DiagnosticMessageV1::new("bad\rline"),
            Err(DiagnosticMessageErrorV1::UnsupportedControlCharacter)
        );
        assert_eq!(
            DiagnosticMessageV1::new("bad\0line"),
            Err(DiagnosticMessageErrorV1::UnsupportedControlCharacter)
        );
    }

    #[test]
    fn diagnostic_accessors_preserve_stable_fields() {
        let diagnostic = CanonicalDiagnosticV1::new(
            2,
            DiagnosticCodeV1::new(41).unwrap(),
            DiagnosticSeverityV1::Error,
            Some(CompilerStageV1::Kernel),
            Some(DiagnosticSubjectIdentityV1::from_untrusted_bytes([3; 32])),
            DiagnosticMessageV1::new("unsupported operation").unwrap(),
        );

        assert_eq!(diagnostic.sequence(), 2);
        assert_eq!(diagnostic.code().get(), 41);
        assert_eq!(diagnostic.stage(), Some(CompilerStageV1::Kernel));
        assert_eq!(diagnostic.message().as_str(), "unsupported operation");
        assert!(diagnostic.is_error());
    }

    #[test]
    fn structured_diagnostic_retains_bounded_local_context() {
        let start = DiagnosticSourcePositionV1::new(12, 4).unwrap();
        let end = DiagnosticSourcePositionV1::new(12, 17).unwrap();
        let span = DiagnosticSourceSpanV1::new("src/kernel.rs", start, end).unwrap();
        let canonical = CanonicalDiagnosticV1::new(
            0,
            DiagnosticCodeV1::new(4101).unwrap(),
            DiagnosticSeverityV1::Error,
            Some(CompilerStageV1::Kernel),
            None,
            DiagnosticMessageV1::new("unsupported operation").unwrap(),
        );
        let diagnostic = StructuredCompilerDiagnosticV1::new(
            canonical,
            Some(DiagnosticMessageV1::new("vector_add").unwrap()),
            Some(DiagnosticMessageV1::new("helper").unwrap()),
            Some(span.clone()),
            vec![DiagnosticCallFrameV1::new(
                DiagnosticMessageV1::new("vector_add").unwrap(),
                Some(span),
            )],
            vec![DiagnosticMessageV1::new("replace the unsupported operation").unwrap()],
        )
        .unwrap();

        assert_eq!(diagnostic.canonical().code().get(), 4101);
        assert_eq!(diagnostic.kernel().unwrap().as_str(), "vector_add");
        assert_eq!(diagnostic.function().unwrap().as_str(), "helper");
        assert_eq!(
            diagnostic.source_span().unwrap().source_name(),
            "src/kernel.rs"
        );
        assert_eq!(diagnostic.call_chain().len(), 1);
        assert_eq!(diagnostic.notes().len(), 1);
    }

    #[test]
    fn structured_diagnostic_rejects_unbounded_or_invalid_context() {
        let position = DiagnosticSourcePositionV1::new(1, 1).unwrap();
        assert_eq!(
            DiagnosticSourceSpanV1::new("", position, position),
            Err(StructuredDiagnosticErrorV1::EmptySourceName)
        );
        assert_eq!(
            DiagnosticSourceSpanV1::new(
                "src/kernel.rs",
                DiagnosticSourcePositionV1::new(2, 1).unwrap(),
                position,
            ),
            Err(StructuredDiagnosticErrorV1::ReversedSourceSpan)
        );
    }
}
