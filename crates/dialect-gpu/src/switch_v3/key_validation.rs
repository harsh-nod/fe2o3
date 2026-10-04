use super::{MAX_SWITCH_CASES_V3, SwitchErrorV3, SwitchKeyKindAttrV3};

/// Allocation-free legacy uniqueness threshold, independent of kernel identity.
pub const SMALL_LEGACY_SWITCH_KEYS_V3: usize = 16;

/// Inert bounds for the shared key checker, not an admission capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwitchKeyValidationResourcesV3 {
    work: usize,
    scratch: usize,
}

impl SwitchKeyValidationResourcesV3 {
    /// Logical loop/element visits, not machine instructions or allocator work.
    pub const fn work_upper_bound(self) -> usize {
        self.work
    }
    /// Simultaneous scratch word cells, including radix counters and Vec owners.
    /// The caller's key storage is borrowed and is not charged again here.
    pub const fn scratch_storage_upper_bound(self) -> usize {
        self.scratch
    }
}

/// Bounds one key-validation call before its traversal or scratch allocation.
/// Actual selector/key validity is checked independently by the caller and checker.
pub fn switch_key_validation_resources_v3(
    kind: SwitchKeyKindAttrV3,
    count: usize,
) -> Result<SwitchKeyValidationResourcesV3, SwitchErrorV3> {
    if count > MAX_SWITCH_CASES_V3 {
        return Err(SwitchErrorV3::Limit);
    }
    let base = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(32))
        .ok_or(SwitchErrorV3::Limit)?;
    if kind != SwitchKeyKindAttrV3::LegacyU64 {
        return Ok(SwitchKeyValidationResourcesV3 {
            work: base,
            scratch: 0,
        });
    }
    if count <= SMALL_LEGACY_SWITCH_KEYS_V3 {
        let pairs = count
            .checked_mul(count.saturating_sub(1))
            .ok_or(SwitchErrorV3::Limit)?
            / 2;
        return Ok(SwitchKeyValidationResourcesV3 {
            work: base.checked_add(pairs).ok_or(SwitchErrorV3::Limit)?,
            scratch: 0,
        });
    }
    // Two source/output initialization visits, eight passes with histogram and
    // scatter visits plus counter zeroing/prefix scans, and final duplicate scan.
    let work = count
        .checked_mul(29)
        .and_then(|n| n.checked_add(8 * 512 + 32))
        .ok_or(SwitchErrorV3::Limit)?;
    let headers = 2 * std::mem::size_of::<Vec<u64>>().div_ceil(std::mem::size_of::<usize>());
    let key_cells = std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>());
    let scratch = count
        .checked_mul(2)
        .and_then(|n| n.checked_mul(key_cells))
        .and_then(|n| n.checked_add(256 + headers + 16))
        .ok_or(SwitchErrorV3::Limit)?;
    Ok(SwitchKeyValidationResourcesV3 { work, scratch })
}

/// Checks descriptive physical integer shape and exact case bits.
///
/// This does not establish an SSA value's type, provenance or authority. Every
/// caller must derive the shape from its actual typed value and check that join.
pub fn validate_switch_case_keys_v3(
    width: u32,
    signed: bool,
    physical_index: bool,
    kind: SwitchKeyKindAttrV3,
    keys: &[u64],
) -> Result<(), SwitchErrorV3> {
    switch_key_validation_resources_v3(kind, keys.len())?;
    if !matches!(width, 8 | 16 | 32 | 64 | 128) || (physical_index && (width != 64 || signed)) {
        return Err(SwitchErrorV3::SelectorType);
    }
    use SwitchKeyKindAttrV3 as K;
    let expected = match kind {
        K::LegacyU64 => None,
        K::EmptyTyped if keys.is_empty() => None,
        K::EmptyTyped => return Err(SwitchErrorV3::Keys),
        K::I8 => Some((8, true, false)),
        K::I16 => Some((16, true, false)),
        K::I32 => Some((32, true, false)),
        K::I64 => Some((64, true, false)),
        K::U8 => Some((8, false, false)),
        K::U16 => Some((16, false, false)),
        K::U32 => Some((32, false, false)),
        K::U64 => Some((64, false, false)),
        K::Index => Some((64, false, true)),
    };
    if expected.is_some_and(|expected| expected != (width, signed, physical_index)) {
        return Err(SwitchErrorV3::SelectorType);
    }
    if kind == K::EmptyTyped {
        return Ok(());
    }
    if width < 64 && keys.iter().any(|key| *key >= 1_u64 << width) {
        return Err(SwitchErrorV3::Keys);
    }
    if kind == K::LegacyU64 {
        legacy_unique(keys)
    } else {
        let sign = if signed { 1_u64 << (width - 1) } else { 0 };
        if keys
            .windows(2)
            .any(|pair| (pair[0] ^ sign) >= (pair[1] ^ sign))
        {
            return Err(SwitchErrorV3::Keys);
        }
        Ok(())
    }
}

fn legacy_unique(keys: &[u64]) -> Result<(), SwitchErrorV3> {
    if keys.len() <= SMALL_LEGACY_SWITCH_KEYS_V3 {
        for (ordinal, key) in keys.iter().enumerate() {
            if keys[..ordinal].iter().any(|previous| previous == key) {
                return Err(SwitchErrorV3::Keys);
            }
        }
        return Ok(());
    }
    let mut first = Vec::new();
    let mut second = Vec::new();
    first
        .try_reserve_exact(keys.len())
        .map_err(|_| SwitchErrorV3::Allocation)?;
    second
        .try_reserve_exact(keys.len())
        .map_err(|_| SwitchErrorV3::Allocation)?;
    // The pinned RawVec::grow_exact reports requested capacities. Reject an
    // implementation change before copying/traversing the scratch payloads.
    if first.capacity() != keys.len() || second.capacity() != keys.len() {
        return Err(SwitchErrorV3::Allocation);
    }
    first.extend_from_slice(keys);
    second.resize(keys.len(), 0);
    for shift in (0..64).step_by(8) {
        let mut positions = [0usize; 256];
        for key in &first {
            positions[((key >> shift) & 255) as usize] += 1;
        }
        let mut prefix = 0usize;
        for position in &mut positions {
            let frequency = *position;
            *position = prefix;
            prefix += frequency;
        }
        for &key in &first {
            let position = &mut positions[((key >> shift) & 255) as usize];
            second[*position] = key;
            *position += 1;
        }
        std::mem::swap(&mut first, &mut second);
    }
    if first.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SwitchErrorV3::Keys);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
