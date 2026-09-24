//! Shared framing mechanics; each caller owns its schema and independent limits.
use sha2::{Digest, Sha256};
use std::ops::Range;

pub(crate) const HEADER: usize = 48;
pub(crate) const OVERHEAD: usize = HEADER + 32;

pub(crate) struct Policy {
    pub magic: [u8; 8],
    pub version: u16,
    pub domain: &'static [u8],
    pub first_max: usize,
    pub second_max: usize,
    pub total_max: usize,
    pub storage_max: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Error<E> {
    Charge(E),
    FirstLength,
    SecondLength,
    Length,
    Arithmetic,
    Header,
    Reserved,
    Identity,
    StorageLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Layout {
    first_end: usize,
    payload_end: usize,
}
impl Layout {
    pub fn new<E>(policy: &Policy, first: usize, second: usize) -> Result<Self, Error<E>> {
        if first == 0 || first > policy.first_max {
            return Err(Error::FirstLength);
        }
        if second == 0 || second > policy.second_max {
            return Err(Error::SecondLength);
        }
        let first_end = HEADER.checked_add(first).ok_or(Error::Arithmetic)?;
        let payload_end = first_end.checked_add(second).ok_or(Error::Arithmetic)?;
        let total = payload_end.checked_add(32).ok_or(Error::Arithmetic)?;
        if total > policy.total_max {
            return Err(Error::Length);
        }
        Ok(Self {
            first_end,
            payload_end,
        })
    }
    pub const fn encoded_len(self) -> usize {
        self.payload_end + 32
    }
    pub fn first_range(self) -> Range<usize> {
        HEADER..self.first_end
    }
    pub fn second_range(self) -> Range<usize> {
        self.first_end..self.payload_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    pub sha256: [u8; 32],
    pub byte_len: u64,
}

fn ceiling<E>(policy: &Policy, storage_limit: usize) -> Result<(), Error<E>> {
    if storage_limit > policy.storage_max {
        return Err(Error::StorageLimit);
    }
    Ok(())
}
fn hash(policy: &Policy, bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(policy.domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn hash_work<E>(policy: &Policy, length: usize) -> Result<usize, Error<E>> {
    length
        .checked_add(policy.domain.len() + 8 + 128)
        .ok_or(Error::Arithmetic)
}

pub(crate) fn seal<E>(
    policy: &Policy,
    layout: Layout,
    bytes: &mut [u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<Identity, Error<E>> {
    ceiling(policy, storage_limit)?;
    if bytes.len() != layout.encoded_len() {
        return Err(Error::Length);
    }
    let work = hash_work::<E>(policy, layout.payload_end)?
        .checked_add(2 * HEADER + 32)
        .ok_or(Error::Arithmetic)?;
    charge(work).map_err(Error::Charge)?;
    // A captured value's destructor is user code too; run it before mutation.
    drop(charge);
    bytes[..HEADER].fill(0);
    bytes[..8].copy_from_slice(&policy.magic);
    bytes[8..10].copy_from_slice(&policy.version.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&(layout.encoded_len() as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(layout.first_range().len() as u64).to_le_bytes());
    bytes[32..40].copy_from_slice(&(layout.second_range().len() as u64).to_le_bytes());
    let sha256 = hash(policy, &bytes[..layout.payload_end]);
    bytes[layout.payload_end..].copy_from_slice(&sha256);
    Ok(Identity {
        sha256,
        byte_len: bytes.len() as u64,
    })
}

pub(crate) fn read<E>(
    policy: &Policy,
    bytes: &[u8],
    storage_limit: usize,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<(Layout, Identity), Error<E>> {
    ceiling(policy, storage_limit)?;
    if bytes.len() < OVERHEAD + 2 || bytes.len() > policy.total_max {
        return Err(Error::Length);
    }
    charge(HEADER).map_err(Error::Charge)?;
    if bytes[..8] != policy.magic
        || bytes[8..10] != policy.version.to_le_bytes()
        || bytes[10..12] != 1_u16.to_le_bytes()
        || bytes[12..16] != (HEADER as u32).to_le_bytes()
    {
        return Err(Error::Header);
    }
    if bytes[40..HEADER] != [0; 8] {
        return Err(Error::Reserved);
    }
    let word = |offset: usize| -> Result<usize, Error<E>> {
        let array = bytes[offset..offset + 8]
            .try_into()
            .map_err(|_| Error::Length)?;
        usize::try_from(u64::from_le_bytes(array)).map_err(|_| Error::Arithmetic)
    };
    let layout = Layout::new(policy, word(24)?, word(32)?)?;
    if word(16)? != bytes.len() || layout.encoded_len() != bytes.len() {
        return Err(Error::Length);
    }
    charge(hash_work::<E>(policy, layout.payload_end)? + 32).map_err(Error::Charge)?;
    let sha256 = hash(policy, &bytes[..layout.payload_end]);
    if bytes[layout.payload_end..] != sha256 {
        return Err(Error::Identity);
    }
    Ok((
        layout,
        Identity {
            sha256,
            byte_len: bytes.len() as u64,
        },
    ))
}
