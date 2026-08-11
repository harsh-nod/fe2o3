use std::fmt;

use crate::{
    EXECUTABLE_MIR_VERSION, MirExecutableModule, MirExecutableValidationError,
    MirExecutableVersion, MirExternalCallRegistry, ValidatedMirExecutableModule,
};

const MAGIC: &[u8; 8] = b"F2MEXE01";
const FLAGS: u16 = 0;
const HEADER_BYTES: usize = 16;

/// Hard pre-parse limit for an executable MIR module. Structural limits are
/// checked after decoding and canonical re-encoding is required.
pub const MAX_EXECUTABLE_WIRE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum MirExecutableDecodeError {
    InputTooLarge,
    UnexpectedEnd,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    LengthMismatch,
    InvalidPayload(serde_json::Error),
    NonCanonical,
    Validation(MirExecutableValidationError),
}

impl fmt::Display for MirExecutableDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => {
                formatter.write_str("executable MIR wire input exceeds its bound")
            }
            Self::UnexpectedEnd => {
                formatter.write_str("executable MIR wire input ended unexpectedly")
            }
            Self::InvalidMagic => formatter.write_str("invalid executable MIR wire magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown executable MIR wire version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported executable MIR wire flags {flags:#06x}"
                )
            }
            Self::LengthMismatch => {
                formatter.write_str("executable MIR payload length does not match the envelope")
            }
            Self::InvalidPayload(error) => {
                write!(formatter, "invalid executable MIR payload: {error}")
            }
            Self::NonCanonical => formatter.write_str("executable MIR wire input is not canonical"),
            Self::Validation(error) => write!(formatter, "invalid executable MIR module: {error}"),
        }
    }
}

impl std::error::Error for MirExecutableDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPayload(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MirExecutableValidationError> for MirExecutableDecodeError {
    fn from(value: MirExecutableValidationError) -> Self {
        Self::Validation(value)
    }
}

impl MirExecutableModule {
    /// Prints the exact canonical textual form used inside the wire envelope.
    pub fn to_canonical_text(&self) -> Result<String, MirExecutableValidationError> {
        self.to_canonical_text_with_registry(&MirExternalCallRegistry::default())
    }

    /// Prints canonical text after resolving external call authority.
    pub fn to_canonical_text_with_registry(
        &self,
        registry: &MirExternalCallRegistry,
    ) -> Result<String, MirExecutableValidationError> {
        self.validate_with_registry(registry)?.to_canonical_text()
    }

    /// Parses only the exact canonical textual form.
    pub fn from_canonical_text(
        text: &str,
    ) -> Result<ValidatedMirExecutableModule, MirExecutableDecodeError> {
        Self::from_canonical_text_with_registry(text, &MirExternalCallRegistry::default())
    }

    /// Parses canonical text while resolving external call authority.
    pub fn from_canonical_text_with_registry(
        text: &str,
        registry: &MirExternalCallRegistry,
    ) -> Result<ValidatedMirExecutableModule, MirExecutableDecodeError> {
        if text.len() > MAX_EXECUTABLE_WIRE_BYTES - HEADER_BYTES {
            return Err(MirExecutableDecodeError::InputTooLarge);
        }
        let module: Self =
            serde_json::from_str(text).map_err(MirExecutableDecodeError::InvalidPayload)?;
        if module.version != MirExecutableVersion::V1 {
            return Err(MirExecutableDecodeError::Validation(
                MirExecutableValidationError::new(
                    "module.version",
                    "text version is not executable MIR V1",
                ),
            ));
        }
        let validated = module.validate_with_registry(registry)?;
        if validated
            .to_canonical_text()
            .map_err(MirExecutableDecodeError::Validation)?
            != text
        {
            return Err(MirExecutableDecodeError::NonCanonical);
        }
        Ok(validated)
    }

    /// Encodes the validated module into the V1 canonical wire envelope.
    ///
    /// The payload is deterministic JSON over structs, enums, vectors and
    /// integers only. No map iteration or floating-point text formatting is
    /// involved; floating constants are represented by their raw bits.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MirExecutableValidationError> {
        self.to_bytes_with_registry(&MirExternalCallRegistry::default())
    }

    /// Encodes after validating device imports against an external trust root.
    pub fn to_bytes_with_registry(
        &self,
        registry: &MirExternalCallRegistry,
    ) -> Result<Vec<u8>, MirExecutableValidationError> {
        self.validate_with_registry(registry)?.to_bytes()
    }

    /// Decodes only the exact canonical V1 representation. The envelope and
    /// byte bound are checked before invoking the payload parser.
    pub fn from_bytes(
        bytes: &[u8],
    ) -> Result<ValidatedMirExecutableModule, MirExecutableDecodeError> {
        Self::from_bytes_with_registry(bytes, &MirExternalCallRegistry::default())
    }

    /// Decodes while resolving device import declarations exclusively through
    /// a process-supplied external trust root.
    pub fn from_bytes_with_registry(
        bytes: &[u8],
        registry: &MirExternalCallRegistry,
    ) -> Result<ValidatedMirExecutableModule, MirExecutableDecodeError> {
        if bytes.len() > MAX_EXECUTABLE_WIRE_BYTES {
            return Err(MirExecutableDecodeError::InputTooLarge);
        }
        if bytes.len() < HEADER_BYTES {
            return Err(MirExecutableDecodeError::UnexpectedEnd);
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(MirExecutableDecodeError::InvalidMagic);
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != EXECUTABLE_MIR_VERSION {
            return Err(MirExecutableDecodeError::UnknownVersion(version));
        }
        let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
        if flags != FLAGS {
            return Err(MirExecutableDecodeError::UnsupportedFlags(flags));
        }
        let payload_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        if payload_len != bytes.len() - HEADER_BYTES {
            return Err(MirExecutableDecodeError::LengthMismatch);
        }

        let module: Self = serde_json::from_slice(&bytes[HEADER_BYTES..])
            .map_err(MirExecutableDecodeError::InvalidPayload)?;
        if module.version != MirExecutableVersion::V1 {
            return Err(MirExecutableDecodeError::Validation(
                MirExecutableValidationError::new(
                    "module.version",
                    "payload version does not match its wire envelope",
                ),
            ));
        }
        let validated = module.validate_with_registry(registry)?;
        if validated
            .to_bytes()
            .map_err(MirExecutableDecodeError::Validation)?
            != bytes
        {
            return Err(MirExecutableDecodeError::NonCanonical);
        }
        Ok(validated)
    }
}

impl ValidatedMirExecutableModule {
    /// Prints deterministic, compact JSON with no ambient formatting state.
    pub fn to_canonical_text(&self) -> Result<String, MirExecutableValidationError> {
        let text = serde_json::to_string(self.as_module())
            .expect("validated executable MIR contains only serializable values");
        if text.len() > MAX_EXECUTABLE_WIRE_BYTES - HEADER_BYTES {
            return Err(MirExecutableValidationError::new(
                "module",
                "canonical text representation exceeds its byte bound",
            ));
        }
        Ok(text)
    }

    /// Encodes without accepting a mutable or deserializable authority token.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MirExecutableValidationError> {
        let payload = serde_json::to_vec(self.as_module())
            .expect("validated executable MIR consists only of serializable in-memory values");
        let total = HEADER_BYTES
            .checked_add(payload.len())
            .ok_or_else(|| MirExecutableValidationError::new("module", "wire size overflow"))?;
        if total > MAX_EXECUTABLE_WIRE_BYTES || payload.len() > u32::MAX as usize {
            return Err(MirExecutableValidationError::new(
                "module",
                "canonical wire representation exceeds its byte bound",
            ));
        }

        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&EXECUTABLE_MIR_VERSION.to_le_bytes());
        bytes.extend_from_slice(&FLAGS.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }
}
