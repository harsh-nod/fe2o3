//! Version-neutral, metered content identity mechanics. No admission policy.
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
