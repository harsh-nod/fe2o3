//! Shared metered source validation and identity mechanics. No admission policy.
use sha2::{Digest, Sha256};

pub(crate) enum HashError<E> {
    Arithmetic,
    Work(E),
}

pub(crate) fn identity<E>(
    domain: &[u8],
    bytes: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<([u8; 32], u64), HashError<E>> {
    let byte_len = u64::try_from(bytes.len()).map_err(|_| HashError::Arithmetic)?;
    let work = domain
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(bytes.len()))
        .and_then(|n| n.checked_add(128))
        .ok_or(HashError::Arithmetic)?;
    charge(work).map_err(HashError::Work)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(byte_len.to_le_bytes());
    hash.update(bytes);
    Ok((hash.finalize().into(), byte_len))
}

pub(crate) enum SourceError<W, E> {
    Wire(W),
    Work(E),
    Storage { required: usize, prepaid: usize },
    Arithmetic,
    FinalizedDigest,
    IdentityMismatch,
}

pub(crate) trait ConditionalTable<'wire>: Sized {
    type WireError<E>;
    const DOMAIN: &'static [u8];
    fn decode<E>(
        bytes: &'wire [u8],
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Self::WireError<E>>;
    fn has_zero_digest(&self) -> bool;
}

macro_rules! conditional_table {
    ($view:ident, $error:ident, $decode:ident, $domain:ident) => {
        impl<'wire> ConditionalTable<'wire> for fe2o3_kernel_descriptor::$view<'wire> {
            type WireError<E> = fe2o3_kernel_descriptor::$error<E>;
            const DOMAIN: &'static [u8] = crate::$domain;
            fn decode<E>(
                bytes: &'wire [u8],
                charge: &mut impl FnMut(usize) -> Result<(), E>,
            ) -> Result<Self, Self::WireError<E>> {
                fe2o3_kernel_descriptor::$decode(bytes, charge)
            }
            fn has_zero_digest(&self) -> bool {
                self.canonical_code_object_digest().as_bytes() == &[0; 32]
            }
        }
    };
}
conditional_table!(
    DeviceDescriptorTableV4,
    DescriptorWireErrorV4,
    decode_device_descriptor_table_v4,
    COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V4
);
conditional_table!(
    DeviceDescriptorTableV5,
    DescriptorWireErrorV5,
    decode_device_descriptor_table_v5,
    COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V5
);

pub(crate) fn table<'wire, T: ConditionalTable<'wire>, E>(
    bytes: &'wire [u8],
    required: Option<usize>,
    prepaid: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<T, SourceError<T::WireError<E>, E>> {
    let required = required.ok_or(SourceError::Arithmetic)?;
    if prepaid < required {
        return Err(SourceError::Storage { required, prepaid });
    }
    charge(1).map_err(SourceError::Work)?;
    let table = T::decode(bytes, charge).map_err(SourceError::Wire)?;
    charge(32).map_err(SourceError::Work)?;
    if !table.has_zero_digest() {
        return Err(SourceError::FinalizedDigest);
    }
    Ok(table)
}

pub(crate) fn validate<'wire, T: ConditionalTable<'wire>, E>(
    bytes: &'wire [u8],
    required: Option<usize>,
    prepaid: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<([u8; 32], u64), SourceError<T::WireError<E>, E>> {
    {
        let _table = table::<T, E>(bytes, required, prepaid, charge)?;
    }
    identity(T::DOMAIN, bytes, charge).map_err(|e| match e {
        HashError::Arithmetic => SourceError::Arithmetic,
        HashError::Work(e) => SourceError::Work(e),
    })
}

pub(crate) fn revalidate<'wire, T: ConditionalTable<'wire>, E>(
    bytes: &'wire [u8],
    required: Option<usize>,
    prepaid: usize,
    expected: (&[u8; 32], u64),
    comparison_work: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), SourceError<T::WireError<E>, E>> {
    let (sha256, byte_len) = validate::<T, E>(bytes, required, prepaid, charge)?;
    charge(comparison_work).map_err(SourceError::Work)?;
    if sha256 != *expected.0 || byte_len != expected.1 {
        return Err(SourceError::IdentityMismatch);
    }
    Ok(())
}
