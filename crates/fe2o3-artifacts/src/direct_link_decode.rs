use std::fmt;

use crate::{
    DIRECT_LINK_EVIDENCE_HEADER_BYTES, DIRECT_LINK_EVIDENCE_MAGIC, DIRECT_LINK_EVIDENCE_VERSION,
    DigestAlgorithm, DigestBytes, DirectLinkBindingExpectationV1, DirectLinkBindingV1,
    DirectLinkBundleEvidenceV1, DirectLinkEvidenceError, DirectLinkToolIdentityV1,
    DirectLinkTransformationIdentityV1, IdentityText, MAX_DIRECT_LINK_BINDINGS,
    MAX_DIRECT_LINK_EVIDENCE_BYTES, MAX_IDENTITY_TEXT_BYTES, PayloadDigest, ValidationError,
};

impl DirectLinkBundleEvidenceV1 {
    /// Decodes canonical, bounded, untrusted direct-link evidence.
    ///
    /// Decoding does not authenticate the record and does not grant load or
    /// launch authority. Call `validate_against` after authenticating the
    /// record through the producer's policy boundary.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DirectLinkDecodeError> {
        if bytes.len() > MAX_DIRECT_LINK_EVIDENCE_BYTES {
            return Err(DirectLinkDecodeError::TooLarge {
                max: MAX_DIRECT_LINK_EVIDENCE_BYTES,
            });
        }
        if bytes.len() < DIRECT_LINK_EVIDENCE_HEADER_BYTES {
            return Err(DirectLinkDecodeError::Truncated);
        }
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != DIRECT_LINK_EVIDENCE_MAGIC {
            return Err(DirectLinkDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != DIRECT_LINK_EVIDENCE_VERSION {
            return Err(DirectLinkDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(DirectLinkDecodeError::UnsupportedFlags(flags));
        }
        let bundle_index_identity = reader.payload_digest()?;
        let count = reader.count("direct-link bindings", 1, MAX_DIRECT_LINK_BINDINGS)?;
        let reserved = reader.u16()?;
        if reserved != 0 {
            return Err(DirectLinkDecodeError::NonZeroReserved(reserved));
        }

        let mut bindings = Vec::with_capacity(count);
        for _ in 0..count {
            let container_identity = reader.payload_digest()?;
            let expectation = DirectLinkBindingExpectationV1::new(
                reader.payload_digest()?,
                reader.tool()?,
                reader.tool()?,
                reader.payload_digest()?,
                DirectLinkTransformationIdentityV1::new(
                    reader.payload_digest()?,
                    reader.payload_digest()?,
                    reader.payload_digest()?,
                ),
                reader.payload_digest()?,
            );
            bindings.push(DirectLinkBindingV1::from_decoded(
                container_identity,
                expectation,
            ));
        }
        if !reader.is_empty() {
            return Err(DirectLinkDecodeError::TrailingBytes);
        }
        Ok(Self::from_decoded(bundle_index_identity, bindings)?)
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], DirectLinkDecodeError> {
        if self.remaining.len() < count {
            return Err(DirectLinkDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DirectLinkDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, DirectLinkDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DirectLinkDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn count(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, DirectLinkDecodeError> {
        let count = usize::from(self.u16()?);
        if !(min..=max).contains(&count) {
            Err(DirectLinkDecodeError::CountOutOfRange {
                field,
                count,
                min,
                max,
            })
        } else {
            Ok(count)
        }
    }

    fn text(&mut self, field: &'static str) -> Result<IdentityText, DirectLinkDecodeError> {
        let count = self.count(field, 1, MAX_IDENTITY_TEXT_BYTES)?;
        let value = std::str::from_utf8(self.take(count)?)
            .map_err(|_| ValidationError::InvalidText { field })?;
        Ok(IdentityText::new(value)?)
    }

    fn payload_digest(&mut self) -> Result<PayloadDigest, DirectLinkDecodeError> {
        let algorithm = match self.u8()? {
            0 => DigestAlgorithm::Sha256,
            tag => {
                return Err(DirectLinkDecodeError::UnknownDigestAlgorithm(tag));
            }
        };
        Ok(PayloadDigest::new(
            algorithm,
            DigestBytes::from_bytes(self.array()?),
        ))
    }

    fn tool(&mut self) -> Result<DirectLinkToolIdentityV1, DirectLinkDecodeError> {
        Ok(DirectLinkToolIdentityV1::new(
            self.text("tool name")?,
            self.text("tool version")?,
            self.payload_digest()?,
            self.payload_digest()?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DirectLinkDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    NonZeroReserved(u16),
    UnknownDigestAlgorithm(u8),
    CountOutOfRange {
        field: &'static str,
        count: usize,
        min: usize,
        max: usize,
    },
    TrailingBytes,
    Model(ValidationError),
    Evidence(DirectLinkEvidenceError),
}

impl fmt::Display for DirectLinkDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "direct-link evidence exceeds {max} bytes"),
            Self::Truncated => write!(formatter, "direct-link evidence is truncated"),
            Self::InvalidMagic => write!(formatter, "direct-link evidence magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported direct-link evidence version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported direct-link evidence flags {flags:#x}"
                )
            }
            Self::NonZeroReserved(value) => {
                write!(
                    formatter,
                    "direct-link reserved field is nonzero: {value:#x}"
                )
            }
            Self::UnknownDigestAlgorithm(tag) => {
                write!(formatter, "unknown direct-link digest algorithm tag {tag}")
            }
            Self::CountOutOfRange {
                field,
                count,
                min,
                max,
            } => write!(formatter, "{field} count {count} is outside {min}..={max}"),
            Self::TrailingBytes => write!(formatter, "direct-link evidence has trailing bytes"),
            Self::Model(error) => error.fmt(formatter),
            Self::Evidence(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DirectLinkDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Evidence(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for DirectLinkDecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Model(value)
    }
}

impl From<DirectLinkEvidenceError> for DirectLinkDecodeError {
    fn from(value: DirectLinkEvidenceError) -> Self {
        Self::Evidence(value)
    }
}
