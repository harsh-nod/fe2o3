use std::fmt;

use sha2::{Digest, Sha256};

use crate::DigestBytes;

/// A cryptographic digest algorithm supported by artifact containers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum DigestAlgorithm {
    Sha256,
}

impl DigestAlgorithm {
    pub const fn output_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
        }
    }

    pub fn calculate(self, payload: &[u8]) -> PayloadDigest {
        let bytes = match self {
            Self::Sha256 => Sha256::digest(payload).into(),
        };
        PayloadDigest {
            algorithm: self,
            bytes: DigestBytes::from_bytes(bytes),
        }
    }
}

/// An explicitly identified cryptographic digest of payload bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PayloadDigest {
    algorithm: DigestAlgorithm,
    bytes: DigestBytes,
}

impl PayloadDigest {
    pub const fn new(algorithm: DigestAlgorithm, bytes: DigestBytes) -> Self {
        Self { algorithm, bytes }
    }

    pub const fn algorithm(self) -> DigestAlgorithm {
        self.algorithm
    }

    pub const fn bytes(self) -> DigestBytes {
        self.bytes
    }

    pub fn verify(self, payload: &[u8]) -> Result<(), DigestMismatch> {
        if self.algorithm.calculate(payload) == self {
            Ok(())
        } else {
            Err(DigestMismatch {
                algorithm: self.algorithm,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DigestMismatch {
    algorithm: DigestAlgorithm,
}

impl DigestMismatch {
    pub const fn algorithm(self) -> DigestAlgorithm {
        self.algorithm
    }
}

impl fmt::Display for DigestMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "payload does not match its {:?} digest", self.algorithm)
    }
}

impl std::error::Error for DigestMismatch {}
