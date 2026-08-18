//! Stable, bounded compiler diagnostics.

use core::fmt;

use crate::{CompilerStageV1, DiagnosticSubjectIdentityV1};

/// Maximum UTF-8 byte length of a V1 diagnostic message.
pub const MAX_DIAGNOSTIC_MESSAGE_BYTES_V1: usize = 4 * 1024;

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
}
