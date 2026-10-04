use std::fmt;

use crate::{
    ConfigurationEntry, DigestAlgorithm, DigestBytes, IdentityText, MAX_CONFIGURATION_ENTRIES,
    MAX_IDENTITY_TEXT_BYTES, MAX_NAME_BYTES, MAX_PROOF_PROPERTIES, MAX_PROOF_RECORD_BYTES,
    MAX_TRUSTED_ITEMS, MeasuredToolIdentity, Name, PROOF_RECORD_MAGIC, PROOF_RECORD_VERSION,
    PayloadDigest, ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome, ProofProperty,
    ProofRecordV1, ProofTargetIdentity, SourceContractIdentity, TrustedItem, ValidationError,
    VerificationModelIdentity,
};

impl ProofRecordV1 {
    /// Decodes untrusted canonical v1 bytes and applies all model validation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProofDecodeError> {
        if bytes.len() > MAX_PROOF_RECORD_BYTES {
            return Err(ProofDecodeError::TooLarge {
                max: MAX_PROOF_RECORD_BYTES,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != PROOF_RECORD_MAGIC {
            return Err(ProofDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != PROOF_RECORD_VERSION {
            return Err(ProofDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(ProofDecodeError::UnsupportedFlags(flags));
        }

        let target = ProofTargetIdentity::new(
            ProofArtifactIdentity::new(
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
            ),
            SourceContractIdentity::new(
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
                reader.payload_digest()?,
            ),
        );

        let configuration_count =
            reader.count("proof configuration", 0, MAX_CONFIGURATION_ENTRIES)?;
        let mut configuration = Vec::with_capacity(configuration_count);
        for _ in 0..configuration_count {
            configuration.push(ConfigurationEntry::new(
                reader.name()?,
                reader.identity_text()?,
            ));
        }
        ensure_strict_order(&configuration, "proof configuration")?;

        let execution = ProofExecutionIdentity::new(
            VerificationModelIdentity::new(reader.identity_text()?, reader.payload_digest()?),
            reader.measured_tool()?,
            reader.measured_tool()?,
            reader.measured_tool()?,
            reader.payload_digest()?,
        );
        let outcome = reader.outcome()?;

        let property_count = reader.count("proved properties", 0, MAX_PROOF_PROPERTIES)?;
        let mut proved_properties = Vec::with_capacity(property_count);
        for _ in 0..property_count {
            proved_properties.push(reader.property()?);
        }
        ensure_strict_order(&proved_properties, "proved properties")?;

        let trusted_count = reader.count("trusted items", 0, MAX_TRUSTED_ITEMS)?;
        let mut trusted_items = Vec::with_capacity(trusted_count);
        for _ in 0..trusted_count {
            trusted_items.push(TrustedItem::new(reader.name()?, reader.payload_digest()?));
        }
        ensure_strict_order(&trusted_items, "trusted items")?;

        if !reader.is_empty() {
            return Err(ProofDecodeError::TrailingBytes);
        }
        Ok(Self::new(
            target,
            configuration,
            execution,
            outcome,
            proved_properties,
            trusted_items,
        )?)
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProofDecodeError> {
        if self.remaining.len() < count {
            return Err(ProofDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProofDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProofDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProofDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn count(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, ProofDecodeError> {
        let count = usize::from(self.u16()?);
        if !(min..=max).contains(&count) {
            return Err(ProofDecodeError::CountOutOfRange {
                field,
                count,
                min,
                max,
            });
        }
        Ok(count)
    }

    fn text(&mut self, field: &'static str, max: usize) -> Result<&'a str, ProofDecodeError> {
        let count = self.count(field, 1, max)?;
        std::str::from_utf8(self.take(count)?)
            .map_err(|_| ValidationError::InvalidText { field }.into())
    }

    fn name(&mut self) -> Result<Name, ProofDecodeError> {
        Ok(Name::new(self.text("name", MAX_NAME_BYTES)?)?)
    }

    fn identity_text(&mut self) -> Result<IdentityText, ProofDecodeError> {
        Ok(IdentityText::new(
            self.text("identity text", MAX_IDENTITY_TEXT_BYTES)?,
        )?)
    }

    fn payload_digest(&mut self) -> Result<PayloadDigest, ProofDecodeError> {
        let algorithm = match self.u8()? {
            0 => DigestAlgorithm::Sha256,
            tag => {
                return Err(ProofDecodeError::UnknownTag {
                    kind: "digest algorithm",
                    tag,
                });
            }
        };
        Ok(PayloadDigest::new(
            algorithm,
            DigestBytes::from_bytes(self.array()?),
        ))
    }

    fn measured_tool(&mut self) -> Result<MeasuredToolIdentity, ProofDecodeError> {
        Ok(MeasuredToolIdentity::new(
            self.identity_text()?,
            self.identity_text()?,
            self.payload_digest()?,
            self.payload_digest()?,
        ))
    }

    fn outcome(&mut self) -> Result<ProofOutcome, ProofDecodeError> {
        match self.u8()? {
            0 => Ok(ProofOutcome::Proved),
            1 => Ok(ProofOutcome::Failed),
            2 => Ok(ProofOutcome::TimedOut),
            tag => Err(ProofDecodeError::UnknownTag {
                kind: "proof outcome",
                tag,
            }),
        }
    }

    fn property(&mut self) -> Result<ProofProperty, ProofDecodeError> {
        match self.u8()? {
            0 => Ok(ProofProperty::Bounds),
            1 => Ok(ProofProperty::AddressOverflowFreedom),
            2 => Ok(ProofProperty::MemorySafety),
            3 => Ok(ProofProperty::Initialization),
            4 => Ok(ProofProperty::RaceFreedom),
            5 => Ok(ProofProperty::LaunchValidity),
            6 => Ok(ProofProperty::FunctionalCorrectness),
            tag => Err(ProofDecodeError::UnknownTag {
                kind: "proof property",
                tag,
            }),
        }
    }
}

fn ensure_strict_order<T: Ord>(values: &[T], field: &'static str) -> Result<(), ProofDecodeError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(ProofDecodeError::NonCanonicalOrder { field })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    UnknownTag {
        kind: &'static str,
        tag: u8,
    },
    CountOutOfRange {
        field: &'static str,
        count: usize,
        min: usize,
        max: usize,
    },
    NonCanonicalOrder {
        field: &'static str,
    },
    TrailingBytes,
    Validation(ValidationError),
}

impl fmt::Display for ProofDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(f, "proof record exceeds {max} bytes"),
            Self::Truncated => write!(f, "proof record is truncated"),
            Self::InvalidMagic => write!(f, "proof record magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(f, "unsupported proof record version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(f, "unsupported proof record flags {flags:#x}")
            }
            Self::UnknownTag { kind, tag } => write!(f, "unknown {kind} tag {tag}"),
            Self::CountOutOfRange {
                field,
                count,
                min,
                max,
            } => write!(f, "{field} count {count} is outside {min}..={max}"),
            Self::NonCanonicalOrder { field } => {
                write!(f, "{field} entries are not in canonical order")
            }
            Self::TrailingBytes => write!(f, "proof record contains trailing bytes"),
            Self::Validation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProofDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for ProofDecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
