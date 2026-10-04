use std::fmt;

use crate::{
    DigestAlgorithm, DigestBytes, DigestMismatch, MAX_CODE_OBJECTS, ManifestV1, PayloadDigest,
};

pub const MAX_CODE_OBJECT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_EMBEDDED_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;

#[derive(Eq, PartialEq)]
pub struct CodeObjectPayload {
    digest: PayloadDigest,
    bytes: Vec<u8>,
}

impl CodeObjectPayload {
    pub fn new(digest: PayloadDigest, bytes: Vec<u8>) -> Result<Self, ContainerValidationError> {
        validate_payload_len(bytes.len())?;
        digest.verify(&bytes)?;
        Ok(Self { digest, bytes })
    }

    pub fn from_bytes(
        algorithm: DigestAlgorithm,
        bytes: Vec<u8>,
    ) -> Result<Self, ContainerValidationError> {
        validate_payload_len(bytes.len())?;
        let digest = algorithm.calculate(&bytes);
        Ok(Self { digest, bytes })
    }

    pub const fn digest(&self) -> PayloadDigest {
        self.digest
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl fmt::Debug for CodeObjectPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeObjectPayload")
            .field("digest", &self.digest)
            .field("byte_len", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArtifactContainerV1 {
    manifest: ManifestV1,
    digest_algorithm: DigestAlgorithm,
    payloads: Vec<CodeObjectPayload>,
}

impl ArtifactContainerV1 {
    pub fn new(
        manifest: ManifestV1,
        digest_algorithm: DigestAlgorithm,
        mut payloads: Vec<CodeObjectPayload>,
    ) -> Result<Self, ContainerValidationError> {
        if payloads.len() > MAX_CODE_OBJECTS {
            return Err(ContainerValidationError::TooManyPayloads {
                max: MAX_CODE_OBJECTS,
            });
        }
        for payload in &payloads {
            if payload.digest().algorithm() != digest_algorithm {
                return Err(ContainerValidationError::DigestAlgorithmMismatch);
            }
            validate_payload_len(payload.bytes().len())?;
        }
        validate_total_payload_bytes(payloads.iter().map(|payload| payload.bytes().len()))?;

        payloads.sort_unstable_by_key(|payload| payload.digest().bytes());
        if let Some(pair) = payloads
            .windows(2)
            .find(|pair| pair[0].digest().bytes() == pair[1].digest().bytes())
        {
            return Err(ContainerValidationError::DuplicatePayload(
                pair[0].digest().bytes(),
            ));
        }
        validate_closure(&manifest, &payloads)?;

        Ok(Self {
            manifest,
            digest_algorithm,
            payloads,
        })
    }

    pub const fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    pub const fn digest_algorithm(&self) -> DigestAlgorithm {
        self.digest_algorithm
    }

    pub fn payloads(&self) -> &[CodeObjectPayload] {
        &self.payloads
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerValidationError {
    EmptyPayload,
    PayloadTooLarge {
        max: usize,
    },
    TooManyPayloads {
        max: usize,
    },
    PayloadBytesOverflow,
    PayloadBytesTooLarge {
        max: usize,
    },
    DigestMismatch(DigestMismatch),
    DigestAlgorithmMismatch,
    DuplicatePayload(DigestBytes),
    MissingPayload(DigestBytes),
    ExtraPayload(DigestBytes),
    PayloadLengthMismatch {
        digest: DigestBytes,
        expected: u64,
        actual: u64,
    },
}

impl fmt::Display for ContainerValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPayload => write!(f, "code-object payload must not be empty"),
            Self::PayloadTooLarge { max } => {
                write!(f, "code-object payload exceeds {max} bytes")
            }
            Self::TooManyPayloads { max } => {
                write!(f, "artifact container exceeds {max} payloads")
            }
            Self::PayloadBytesOverflow => write!(f, "total payload byte length overflows"),
            Self::PayloadBytesTooLarge { max } => {
                write!(f, "artifact payloads exceed {max} total bytes")
            }
            Self::DigestMismatch(error) => error.fmt(f),
            Self::DigestAlgorithmMismatch => {
                write!(f, "payload digest algorithm does not match the container")
            }
            Self::DuplicatePayload(digest) => {
                write!(f, "duplicate payload for digest {digest:?}")
            }
            Self::MissingPayload(digest) => {
                write!(f, "manifest code object {digest:?} has no payload")
            }
            Self::ExtraPayload(digest) => {
                write!(f, "payload {digest:?} is not listed in the manifest")
            }
            Self::PayloadLengthMismatch {
                digest,
                expected,
                actual,
            } => write!(
                f,
                "payload {digest:?} has {actual} bytes, but the manifest declares {expected}"
            ),
        }
    }
}

impl std::error::Error for ContainerValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DigestMismatch(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DigestMismatch> for ContainerValidationError {
    fn from(value: DigestMismatch) -> Self {
        Self::DigestMismatch(value)
    }
}

fn validate_payload_len(len: usize) -> Result<(), ContainerValidationError> {
    if len == 0 {
        Err(ContainerValidationError::EmptyPayload)
    } else if len > MAX_CODE_OBJECT_BYTES {
        Err(ContainerValidationError::PayloadTooLarge {
            max: MAX_CODE_OBJECT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_total_payload_bytes(
    lengths: impl IntoIterator<Item = usize>,
) -> Result<(), ContainerValidationError> {
    let total = lengths.into_iter().try_fold(0usize, |total, len| {
        total
            .checked_add(len)
            .ok_or(ContainerValidationError::PayloadBytesOverflow)
    })?;
    if total > MAX_EMBEDDED_PAYLOAD_BYTES {
        Err(ContainerValidationError::PayloadBytesTooLarge {
            max: MAX_EMBEDDED_PAYLOAD_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_closure(
    manifest: &ManifestV1,
    payloads: &[CodeObjectPayload],
) -> Result<(), ContainerValidationError> {
    let mut expected = manifest.code_objects().iter().peekable();
    let mut actual = payloads.iter().peekable();
    loop {
        match (expected.peek(), actual.peek()) {
            (Some(code_object), Some(payload)) => {
                let expected_digest = code_object.digest();
                let actual_digest = payload.digest().bytes();
                match expected_digest.cmp(&actual_digest) {
                    std::cmp::Ordering::Less => {
                        return Err(ContainerValidationError::MissingPayload(expected_digest));
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ContainerValidationError::ExtraPayload(actual_digest));
                    }
                    std::cmp::Ordering::Equal => {
                        let actual_len = payload.bytes().len() as u64;
                        if code_object.byte_len() != actual_len {
                            return Err(ContainerValidationError::PayloadLengthMismatch {
                                digest: expected_digest,
                                expected: code_object.byte_len(),
                                actual: actual_len,
                            });
                        }
                        expected.next();
                        actual.next();
                    }
                }
            }
            (Some(code_object), None) => {
                return Err(ContainerValidationError::MissingPayload(
                    code_object.digest(),
                ));
            }
            (None, Some(payload)) => {
                return Err(ContainerValidationError::ExtraPayload(
                    payload.digest().bytes(),
                ));
            }
            (None, None) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_payload_bounds_are_enforced_without_large_allocations() {
        assert_eq!(
            validate_payload_len(0),
            Err(ContainerValidationError::EmptyPayload)
        );
        assert_eq!(validate_payload_len(MAX_CODE_OBJECT_BYTES), Ok(()));
        assert_eq!(
            validate_payload_len(MAX_CODE_OBJECT_BYTES + 1),
            Err(ContainerValidationError::PayloadTooLarge {
                max: MAX_CODE_OBJECT_BYTES
            })
        );
        assert_eq!(
            validate_total_payload_bytes([MAX_EMBEDDED_PAYLOAD_BYTES]),
            Ok(())
        );
        assert_eq!(
            validate_total_payload_bytes([MAX_EMBEDDED_PAYLOAD_BYTES, 1]),
            Err(ContainerValidationError::PayloadBytesTooLarge {
                max: MAX_EMBEDDED_PAYLOAD_BYTES
            })
        );
        assert_eq!(
            validate_total_payload_bytes([usize::MAX, 1]),
            Err(ContainerValidationError::PayloadBytesOverflow)
        );
    }
}
