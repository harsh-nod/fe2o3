use std::fmt;

use crate::model::MAX_CANONICAL_KERNEL_IR_BYTES_V4;

/// Exact domain bytes for the verified canonical Kernel IR V6 identity.
pub const VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V6\0";
/// Frozen policy version in the verified canonical Kernel IR V6 identity.
pub const VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1: u16 = 1;
/// Fixed magic required at the start of a Kernel IR V6 identity preimage.
pub const CANONICAL_KERNEL_IR_MAGIC_V6: [u8; 8] = *b"FE2O3KI\0";
/// Exact Kernel IR wire version required by this identity construction.
pub const CANONICAL_KERNEL_IR_VERSION_V6: u16 = 6;
/// Fixed Kernel IR V6 envelope header length checked by this crate.
pub const CANONICAL_KERNEL_IR_V6_HEADER_BYTES: usize = 20;

const VERSION_OFFSET: usize = 8;
const FLAGS_OFFSET: usize = 10;
const LENGTH_OFFSET: usize = 12;
const RESERVED_OFFSET: usize = 16;

/// Recomputed digest for the exact canonical KIR V6 SHA-256 policy V1 scheme.
///
/// Construction requires the complete claimed V6 preimage. This remains inert:
/// only the external Kernel IR owner can establish typed canonicality and
/// semantic validity for those bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity {
    digest: [u8; 32],
    canonical_length: u64,
}

impl RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity {
    /// Returns the exact SHA-256 digest bytes.
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Returns the exact byte length committed by the identity preimage.
    pub const fn canonical_length(self) -> u64 {
        self.canonical_length
    }
}

/// Recomputes the exact verified-canonical Kernel IR V6 policy V1 identity.
///
/// The SHA-256 preimage is, in exact order:
///
/// 1. the domain length as one little-endian `u32`;
/// 2. [`VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1`] verbatim;
/// 3. [`VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1`] as little-endian `u16`;
/// 4. the complete canonical V6 byte length as little-endian `u64`; and
/// 5. the complete canonical V6 bytes verbatim.
///
/// The domain and the embedded artifact header both commit wire version 6. This
/// helper checks the fixed artifact envelope but cannot perform the authoritative
/// typed KIR decode, canonical re-encoding, or semantic verification. A caller
/// must supply bytes already owned by that downstream move-only validator. The
/// returned value is an inert recomputation and grants no authority.
pub fn recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(
    canonical_v6_bytes: &[u8],
) -> Result<RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity, KernelIrV6IdentityPreimageError> {
    let length = u64::try_from(canonical_v6_bytes.len())
        .map_err(|_| KernelIrV6IdentityPreimageError::LengthOverflow)?;
    if length > MAX_CANONICAL_KERNEL_IR_BYTES_V4 {
        return Err(KernelIrV6IdentityPreimageError::TooLarge {
            actual: length,
            max: MAX_CANONICAL_KERNEL_IR_BYTES_V4,
        });
    }
    let header = canonical_v6_bytes
        .get(..CANONICAL_KERNEL_IR_V6_HEADER_BYTES)
        .ok_or(KernelIrV6IdentityPreimageError::TruncatedHeader {
            actual: canonical_v6_bytes.len(),
        })?;
    if header[..8] != CANONICAL_KERNEL_IR_MAGIC_V6 {
        return Err(KernelIrV6IdentityPreimageError::InvalidMagic);
    }
    let version = u16::from_le_bytes(
        header[VERSION_OFFSET..FLAGS_OFFSET]
            .try_into()
            .expect("fixed header version field"),
    );
    if version != CANONICAL_KERNEL_IR_VERSION_V6 {
        return Err(KernelIrV6IdentityPreimageError::NotVersion6 { actual: version });
    }
    let flags = u16::from_le_bytes(
        header[FLAGS_OFFSET..LENGTH_OFFSET]
            .try_into()
            .expect("fixed header flags field"),
    );
    if flags != 0 {
        return Err(KernelIrV6IdentityPreimageError::UnsupportedFlags { actual: flags });
    }
    let declared = u32::from_le_bytes(
        header[LENGTH_OFFSET..RESERVED_OFFSET]
            .try_into()
            .expect("fixed header length field"),
    );
    if u64::from(declared) != length {
        return Err(KernelIrV6IdentityPreimageError::DeclaredLengthMismatch {
            declared,
            actual: length,
        });
    }
    let reserved = u32::from_le_bytes(
        header[RESERVED_OFFSET..CANONICAL_KERNEL_IR_V6_HEADER_BYTES]
            .try_into()
            .expect("fixed header reserved field"),
    );
    if reserved != 0 {
        return Err(KernelIrV6IdentityPreimageError::NonzeroReserved { actual: reserved });
    }

    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1.len())
        .expect("frozen identity domain length fits u32");
    let mut digest = Sha256::new();
    digest.update(&domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1);
    digest.update(&VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(&length.to_le_bytes());
    digest.update(canonical_v6_bytes);
    Ok(RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity {
        digest: digest.finalize(),
        canonical_length: length,
    })
}

/// Invalid preimage for the exact Kernel IR V6 identity recomputation helper.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum KernelIrV6IdentityPreimageError {
    /// The host slice length could not fit canonical `u64` framing.
    LengthOverflow,
    /// The preimage exceeded the hard canonical Kernel IR length cap.
    TooLarge {
        /// Supplied byte length.
        actual: u64,
        /// Hard maximum byte length.
        max: u64,
    },
    /// The fixed 20-byte Kernel IR envelope was incomplete.
    TruncatedHeader {
        /// Supplied byte length.
        actual: usize,
    },
    /// The fixed Kernel IR magic did not match.
    InvalidMagic,
    /// The artifact header was not exact Kernel IR V6.
    NotVersion6 {
        /// Supplied artifact header version.
        actual: u16,
    },
    /// The artifact header carried nonzero flags.
    UnsupportedFlags {
        /// Supplied flags.
        actual: u16,
    },
    /// The artifact header length did not equal the complete preimage length.
    DeclaredLengthMismatch {
        /// Header-declared length.
        declared: u32,
        /// Complete supplied length.
        actual: u64,
    },
    /// The artifact header's reserved field was nonzero.
    NonzeroReserved {
        /// Supplied reserved value.
        actual: u32,
    },
}

impl fmt::Display for KernelIrV6IdentityPreimageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOverflow => formatter.write_str("Kernel IR preimage length overflows u64"),
            Self::TooLarge { actual, max } => {
                write!(
                    formatter,
                    "Kernel IR preimage uses {actual} bytes, exceeding {max}"
                )
            }
            Self::TruncatedHeader { actual } => write!(
                formatter,
                "Kernel IR preimage has {actual} bytes, fewer than the 20-byte V6 header"
            ),
            Self::InvalidMagic => formatter.write_str("invalid Kernel IR V6 preimage magic"),
            Self::NotVersion6 { actual } => {
                write!(
                    formatter,
                    "Kernel IR preimage version is {actual}, expected 6"
                )
            }
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "Kernel IR V6 preimage flags are {actual:#x}")
            }
            Self::DeclaredLengthMismatch { declared, actual } => write!(
                formatter,
                "Kernel IR V6 preimage declares {declared} bytes but supplies {actual}"
            ),
            Self::NonzeroReserved { actual } => write!(
                formatter,
                "Kernel IR V6 preimage reserved field is {actual:#x}"
            ),
        }
    }
}

impl std::error::Error for KernelIrV6IdentityPreimageError {}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    message_len: u64,
}

impl Sha256 {
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            message_len: 0,
        }
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.message_len = self
            .message_len
            .checked_add(bytes.len() as u64)
            .expect("hard-bounded Kernel IR identity preimage length");
        if self.buffer_len != 0 {
            let count = (64 - self.buffer_len).min(bytes.len());
            self.buffer[self.buffer_len..self.buffer_len + count].copy_from_slice(&bytes[..count]);
            self.buffer_len += count;
            bytes = &bytes[count..];
            if self.buffer_len != 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        while bytes.len() >= 64 {
            let block: &[u8; 64] = bytes[..64].try_into().expect("exact SHA-256 block");
            self.compress(block);
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.buffer_len = bytes.len();
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self
            .message_len
            .checked_mul(8)
            .expect("hard-bounded Kernel IR identity bit length");
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer = [0; 64];
        } else {
            self.buffer[self.buffer_len..56].fill(0);
        }
        self.buffer[56..].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut output = [0_u8; 32];
        for (chunk, word) in output.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte SHA-256 word"));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (word, constant) in words.into_iter().zip(Self::ROUND_CONSTANTS) {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(constant)
                .wrapping_add(word);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
mod tests {
    use super::Sha256;

    fn digest(bytes: &[u8]) -> [u8; 32] {
        let mut sha = Sha256::new();
        sha.update(bytes);
        sha.finalize()
    }

    #[test]
    fn sha256_matches_fips_vectors() {
        assert_eq!(
            digest(b""),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ]
        );
        assert_eq!(
            digest(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }
}
