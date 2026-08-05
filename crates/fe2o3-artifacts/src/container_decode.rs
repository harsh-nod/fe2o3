use std::fmt;

use crate::{
    ArtifactContainerV1, CONTAINER_MAGIC, CONTAINER_VERSION, CodeObjectPayload,
    ContainerValidationError, DecodeError, DigestAlgorithm, DigestBytes, MAX_CODE_OBJECT_BYTES,
    MAX_CODE_OBJECTS, MAX_CONTAINER_BYTES, MAX_EMBEDDED_PAYLOAD_BYTES, MAX_MANIFEST_BYTES,
    ManifestV1, PayloadDigest,
};

impl ArtifactContainerV1 {
    /// Decodes untrusted bytes and verifies the manifest, closure, lengths, and payload digests.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ContainerDecodeError> {
        validate_input_len(bytes.len())?;
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != CONTAINER_MAGIC {
            return Err(ContainerDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != CONTAINER_VERSION {
            return Err(ContainerDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(ContainerDecodeError::UnsupportedFlags(flags));
        }
        let digest_algorithm = digest_algorithm_from_tag(reader.u16()?)?;
        let reserved = reader.u16()?;
        if reserved != 0 {
            return Err(ContainerDecodeError::NonZeroReserved(reserved));
        }
        let manifest_len = reader.length_u32("manifest", 1, MAX_MANIFEST_BYTES)?;
        let payload_count = reader.length_u32("payload count", 1, MAX_CODE_OBJECTS)?;
        let manifest = ManifestV1::from_bytes(reader.take(manifest_len)?)?;

        let mut descriptors: Vec<(DigestBytes, usize)> = Vec::with_capacity(payload_count);
        let mut payload_bytes = 0usize;
        for _ in 0..payload_count {
            let digest = DigestBytes::from_bytes(reader.array()?);
            if let Some((previous, _)) = descriptors.last() {
                match previous.cmp(&digest) {
                    std::cmp::Ordering::Equal => {
                        return Err(ContainerValidationError::DuplicatePayload(digest).into());
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ContainerDecodeError::NonCanonicalPayloadOrder);
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            let byte_len = reader.length_u64("code-object payload", 1, MAX_CODE_OBJECT_BYTES)?;
            payload_bytes = payload_bytes
                .checked_add(byte_len)
                .ok_or(ContainerDecodeError::PayloadBytesOverflow)?;
            if payload_bytes > MAX_EMBEDDED_PAYLOAD_BYTES {
                return Err(ContainerDecodeError::PayloadBytesTooLarge {
                    max: MAX_EMBEDDED_PAYLOAD_BYTES,
                });
            }
            descriptors.push((digest, byte_len));
        }

        match reader.remaining_len().cmp(&payload_bytes) {
            std::cmp::Ordering::Less => return Err(ContainerDecodeError::Truncated),
            std::cmp::Ordering::Greater => return Err(ContainerDecodeError::TrailingBytes),
            std::cmp::Ordering::Equal => {}
        }

        let mut payloads = Vec::with_capacity(payload_count);
        for (digest, byte_len) in descriptors {
            payloads.push(CodeObjectPayload::new(
                PayloadDigest::new(digest_algorithm, digest),
                reader.take(byte_len)?.to_vec(),
            )?);
        }
        debug_assert_eq!(reader.remaining_len(), 0);
        Ok(Self::new(manifest, digest_algorithm, payloads)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    UnknownDigestAlgorithm(u16),
    NonZeroReserved(u16),
    LengthOutOfRange {
        field: &'static str,
        value: u64,
        min: usize,
        max: usize,
    },
    PayloadBytesOverflow,
    PayloadBytesTooLarge {
        max: usize,
    },
    NonCanonicalPayloadOrder,
    TrailingBytes,
    Manifest(DecodeError),
    Validation(ContainerValidationError),
}

impl fmt::Display for ContainerDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(f, "artifact container exceeds {max} bytes"),
            Self::Truncated => write!(f, "artifact container is truncated"),
            Self::InvalidMagic => write!(f, "artifact container magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(f, "unsupported artifact container version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(f, "unsupported artifact container flags {flags:#x}")
            }
            Self::UnknownDigestAlgorithm(tag) => {
                write!(f, "unknown artifact digest algorithm tag {tag}")
            }
            Self::NonZeroReserved(value) => {
                write!(
                    f,
                    "artifact container reserved field is nonzero: {value:#x}"
                )
            }
            Self::LengthOutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} length {value} is outside {min}..={max}"),
            Self::PayloadBytesOverflow => write!(f, "total payload byte length overflows"),
            Self::PayloadBytesTooLarge { max } => {
                write!(f, "artifact payloads exceed {max} total bytes")
            }
            Self::NonCanonicalPayloadOrder => {
                write!(f, "artifact payloads are not in canonical digest order")
            }
            Self::TrailingBytes => write!(f, "artifact container contains trailing bytes"),
            Self::Manifest(error) => write!(f, "invalid embedded manifest: {error}"),
            Self::Validation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ContainerDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DecodeError> for ContainerDecodeError {
    fn from(value: DecodeError) -> Self {
        Self::Manifest(value)
    }
}

impl From<ContainerValidationError> for ContainerDecodeError {
    fn from(value: ContainerValidationError) -> Self {
        Self::Validation(value)
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ContainerDecodeError> {
        if self.remaining.len() < count {
            return Err(ContainerDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ContainerDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, ContainerDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ContainerDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ContainerDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn length_u32(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, ContainerDecodeError> {
        validate_length(field, u64::from(self.u32()?), min, max)
    }

    fn length_u64(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, ContainerDecodeError> {
        validate_length(field, self.u64()?, min, max)
    }
}

fn validate_input_len(len: usize) -> Result<(), ContainerDecodeError> {
    if len > MAX_CONTAINER_BYTES {
        Err(ContainerDecodeError::TooLarge {
            max: MAX_CONTAINER_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_length(
    field: &'static str,
    value: u64,
    min: usize,
    max: usize,
) -> Result<usize, ContainerDecodeError> {
    if value < min as u64 || value > max as u64 {
        Err(ContainerDecodeError::LengthOutOfRange {
            field,
            value,
            min,
            max,
        })
    } else {
        Ok(value as usize)
    }
}

fn digest_algorithm_from_tag(tag: u16) -> Result<DigestAlgorithm, ContainerDecodeError> {
    match tag {
        1 => Ok(DigestAlgorithm::Sha256),
        _ => Err(ContainerDecodeError::UnknownDigestAlgorithm(tag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_input_bound_is_enforced_without_a_large_allocation() {
        assert_eq!(validate_input_len(MAX_CONTAINER_BYTES), Ok(()));
        assert_eq!(
            validate_input_len(MAX_CONTAINER_BYTES + 1),
            Err(ContainerDecodeError::TooLarge {
                max: MAX_CONTAINER_BYTES
            })
        );
    }
}
