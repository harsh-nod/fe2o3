//! Canonical source declaration for per-workgroup memory resources.

use std::fmt;

pub const KERNEL_RESOURCE_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3KR\0";
pub const KERNEL_RESOURCE_CONTRACT_VERSION_V1: u16 = 1;
pub const MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1: usize = 28;

pub const KERNEL_RESOURCE_REGISTRATION_PREFIX_V1: &str = "__fe2o3_kernel_resource_contract_v1_";
pub const KERNEL_RESOURCE_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3KRA");
pub const KERNEL_RESOURCE_REGISTRATION_VERSION_V1: u16 = 1;
pub const KERNEL_RESOURCE_REGISTRATION_KIND_V1: u16 = 1;

const HEADER_BYTES_V1: usize = 20;
const FLAG_STATIC_SHARED_MEMORY: u16 = 0x0001;
const FLAG_MAX_DYNAMIC_SHARED_MEMORY: u16 = 0x0002;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelResourceContractV1 {
    static_shared_memory_bytes: u32,
    max_dynamic_shared_memory_bytes: u32,
}

impl KernelResourceContractV1 {
    pub fn new(
        static_shared_memory_bytes: u32,
        max_dynamic_shared_memory_bytes: u32,
    ) -> Result<Self, KernelResourceContractValidationErrorV1> {
        if static_shared_memory_bytes == 0 && max_dynamic_shared_memory_bytes == 0 {
            return Err(KernelResourceContractValidationErrorV1::Empty);
        }
        static_shared_memory_bytes
            .checked_add(max_dynamic_shared_memory_bytes)
            .ok_or(KernelResourceContractValidationErrorV1::SharedMemoryOverflow)?;
        Ok(Self {
            static_shared_memory_bytes,
            max_dynamic_shared_memory_bytes,
        })
    }

    pub const fn static_shared_memory_bytes(self) -> u32 {
        self.static_shared_memory_bytes
    }

    pub const fn max_dynamic_shared_memory_bytes(self) -> u32 {
        self.max_dynamic_shared_memory_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelResourceContractValidationErrorV1 {
    Empty,
    SharedMemoryOverflow,
}

impl fmt::Display for KernelResourceContractValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("kernel resource contract is empty"),
            Self::SharedMemoryOverflow => {
                formatter.write_str("static and dynamic shared-memory bytes overflow u32")
            }
        }
    }
}

impl std::error::Error for KernelResourceContractValidationErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelResourceContractDecodeErrorV1 {
    TooLarge,
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength(u32),
    TrailingBytes,
    NonzeroReserved,
    NonCanonical,
    Validation(KernelResourceContractValidationErrorV1),
}

impl fmt::Display for KernelResourceContractDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(
                formatter,
                "kernel resource contract exceeds {MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1} bytes"
            ),
            Self::Truncated => formatter.write_str("kernel resource contract is truncated"),
            Self::InvalidMagic => formatter.write_str("kernel resource contract magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported kernel resource contract version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported kernel resource contract flags {flags:#x}"
                )
            }
            Self::InvalidLength(length) => {
                write!(
                    formatter,
                    "invalid kernel resource contract length {length}"
                )
            }
            Self::TrailingBytes => {
                formatter.write_str("kernel resource contract contains trailing bytes")
            }
            Self::NonzeroReserved => {
                formatter.write_str("kernel resource contract reserved field is nonzero")
            }
            Self::NonCanonical => formatter.write_str("kernel resource contract is not canonical"),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for KernelResourceContractDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<KernelResourceContractValidationErrorV1> for KernelResourceContractDecodeErrorV1 {
    fn from(value: KernelResourceContractValidationErrorV1) -> Self {
        Self::Validation(value)
    }
}

pub fn encode_kernel_resource_contract_v1(contract: KernelResourceContractV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1);
    bytes.extend_from_slice(&KERNEL_RESOURCE_CONTRACT_MAGIC_V1);
    push_u16(&mut bytes, KERNEL_RESOURCE_CONTRACT_VERSION_V1);
    let flags = (u16::from(contract.static_shared_memory_bytes != 0) * FLAG_STATIC_SHARED_MEMORY)
        | (u16::from(contract.max_dynamic_shared_memory_bytes != 0)
            * FLAG_MAX_DYNAMIC_SHARED_MEMORY);
    push_u16(&mut bytes, flags);
    push_u32(&mut bytes, MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1 as u32);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, contract.static_shared_memory_bytes);
    push_u32(&mut bytes, contract.max_dynamic_shared_memory_bytes);
    bytes
}

pub fn decode_kernel_resource_contract_v1(
    bytes: &[u8],
) -> Result<KernelResourceContractV1, KernelResourceContractDecodeErrorV1> {
    if bytes.len() > MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1 {
        return Err(KernelResourceContractDecodeErrorV1::TooLarge);
    }
    if bytes.len() < HEADER_BYTES_V1 {
        return Err(KernelResourceContractDecodeErrorV1::Truncated);
    }
    if bytes.get(..8) != Some(KERNEL_RESOURCE_CONTRACT_MAGIC_V1.as_slice()) {
        return Err(KernelResourceContractDecodeErrorV1::InvalidMagic);
    }
    let version = read_u16(bytes, 8)?;
    if version != KERNEL_RESOURCE_CONTRACT_VERSION_V1 {
        return Err(KernelResourceContractDecodeErrorV1::UnknownVersion(version));
    }
    let flags = read_u16(bytes, 10)?;
    if flags == 0 || flags & !(FLAG_STATIC_SHARED_MEMORY | FLAG_MAX_DYNAMIC_SHARED_MEMORY) != 0 {
        return Err(KernelResourceContractDecodeErrorV1::UnsupportedFlags(flags));
    }
    let length = read_u32(bytes, 12)?;
    if length != MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1 as u32 {
        return Err(KernelResourceContractDecodeErrorV1::InvalidLength(length));
    }
    if bytes.len() < MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1 {
        return Err(KernelResourceContractDecodeErrorV1::Truncated);
    }
    if bytes.len() > MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1 {
        return Err(KernelResourceContractDecodeErrorV1::TrailingBytes);
    }
    if read_u32(bytes, 16)? != 0 {
        return Err(KernelResourceContractDecodeErrorV1::NonzeroReserved);
    }
    let static_bytes = read_u32(bytes, 20)?;
    let dynamic_bytes = read_u32(bytes, 24)?;
    if flags & FLAG_STATIC_SHARED_MEMORY == 0 && static_bytes != 0
        || flags & FLAG_MAX_DYNAMIC_SHARED_MEMORY == 0 && dynamic_bytes != 0
    {
        return Err(KernelResourceContractDecodeErrorV1::NonCanonical);
    }
    let contract = KernelResourceContractV1::new(static_bytes, dynamic_bytes)?;
    if encode_kernel_resource_contract_v1(contract) != bytes {
        return Err(KernelResourceContractDecodeErrorV1::NonCanonical);
    }
    Ok(contract)
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, KernelResourceContractDecodeErrorV1> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or(KernelResourceContractDecodeErrorV1::Truncated)?;
    Ok(u16::from_le_bytes(bytes.try_into().map_err(|_| {
        KernelResourceContractDecodeErrorV1::Truncated
    })?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, KernelResourceContractDecodeErrorV1> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or(KernelResourceContractDecodeErrorV1::Truncated)?;
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        KernelResourceContractDecodeErrorV1::Truncated
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact() -> KernelResourceContractV1 {
        KernelResourceContractV1::new(256, 1_024).unwrap()
    }

    #[test]
    fn canonical_round_trip_is_exact() {
        let bytes = encode_kernel_resource_contract_v1(exact());
        assert_eq!(bytes.len(), MAX_KERNEL_RESOURCE_CONTRACT_BYTES_V1);
        assert_eq!(decode_kernel_resource_contract_v1(&bytes).unwrap(), exact());
    }

    #[test]
    fn empty_and_overflowing_contracts_are_rejected() {
        assert_eq!(
            KernelResourceContractV1::new(0, 0),
            Err(KernelResourceContractValidationErrorV1::Empty)
        );
        assert_eq!(
            KernelResourceContractV1::new(u32::MAX, 1),
            Err(KernelResourceContractValidationErrorV1::SharedMemoryOverflow)
        );
    }

    #[test]
    fn malformed_wire_values_fail_closed() {
        let canonical = encode_kernel_resource_contract_v1(exact());
        for length in 0..canonical.len() {
            assert!(decode_kernel_resource_contract_v1(&canonical[..length]).is_err());
        }

        let mut bad = canonical.clone();
        bad[0] ^= 1;
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::InvalidMagic)
        );

        let mut bad = canonical.clone();
        bad[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::UnknownVersion(2))
        );

        let mut bad = canonical.clone();
        bad[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::UnsupportedFlags(
                0x8000
            ))
        );

        let mut bad = canonical.clone();
        bad[12..16].copy_from_slice(&27_u32.to_le_bytes());
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::InvalidLength(27))
        );

        let mut bad = canonical.clone();
        bad[16] = 1;
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::NonzeroReserved)
        );

        let mut bad = canonical.clone();
        bad[10..12].copy_from_slice(&FLAG_STATIC_SHARED_MEMORY.to_le_bytes());
        assert_eq!(
            decode_kernel_resource_contract_v1(&bad),
            Err(KernelResourceContractDecodeErrorV1::NonCanonical)
        );
    }
}
