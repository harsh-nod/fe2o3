//! Opt-in fixed-buffer I/O. Legacy open/recheck/publish keep their old semantics.
use super::*;
use crate::source_edit_v1::MAX_SOURCE_EDIT_PATH_BYTES_V1;
use std::mem::size_of;

/// Maximum simultaneously live streaming comparison buffer.
pub const BOUNDED_SOURCE_IO_CHUNK_BYTES_V1: usize = 1024;

fn limit(limit: usize) -> Result<(), String> {
    if limit == 0 || limit > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 {
        return Err("bounded source limit must be 1..=1 MiB".into());
    }
    Ok(())
}

fn payload_storage(
    length: usize,
    capacity: usize,
    name_capacity: usize,
    maximum: usize,
) -> Result<usize, String> {
    limit(maximum)?;
    if length > capacity
        || length > maximum
        || capacity > maximum + 1
        || name_capacity > MAX_SOURCE_EDIT_PATH_BYTES_V1
    {
        return Err("bounded retained source capacity exceeds caller profile".into());
    }
    size_of::<RetainedSource>()
        .checked_add(capacity)
        .and_then(|n| n.checked_add(name_capacity))
        .ok_or_else(|| "bounded retained source storage overflow".into())
}

// Exact original length plus one EOF/refusal byte; no read_to_end growth.
fn read_original(
    input: &mut impl Read,
    declared: usize,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    limit(maximum)?;
    if declared > maximum {
        return Err("bounded source length exceeds caller limit".into());
    }
    let allocation = declared
        .checked_add(1)
        .ok_or("bounded source length overflow")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(allocation)
        .map_err(|_| "bounded source allocation refused")?;
    if bytes.capacity() != allocation {
        return Err("bounded source allocation capacity differs".into());
    }
    bytes.resize(allocation, 0);
    let mut filled = 0;
    while filled < allocation {
        let n = input
            .read(&mut bytes[filled..])
            .map_err(|_| "bounded source read refused")?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    if filled != declared {
        return Err("bounded source changed length or ended early".into());
    }
    bytes.truncate(declared);
    Ok(bytes)
}

pub(super) fn compare_stream(input: &mut impl Read, expected: &[u8]) -> Result<(), String> {
    let mut scratch = [0; BOUNDED_SOURCE_IO_CHUNK_BYTES_V1];
    let mut offset = 0usize;
    loop {
        let take = expected
            .len()
            .saturating_sub(offset)
            .saturating_add(1)
            .min(scratch.len());
        let n = input
            .read(&mut scratch[..take])
            .map_err(|_| "bounded source comparison read refused")?;
        if n == 0 {
            break;
        }
        let end = offset
            .checked_add(n)
            .ok_or("bounded comparison length overflow")?;
        if expected.get(offset..end) != Some(&scratch[..n]) {
            return Err("bounded comparison bytes differ".into());
        }
        offset = end;
    }
    if offset != expected.len() {
        return Err("bounded comparison ended early".into());
    }
    Ok(())
}

impl RetainedSource {
    /// Same retained-descriptor contract as open, with caller bound and an
    /// exact-capacity original buffer. Allocation refusal never authenticates
    /// source. Caller prepays maximum+1 bytes, header and bounded basename.
    pub fn open_bounded_streaming_v1(path: &str, maximum: usize) -> Result<Self, String> {
        limit(maximum)?;
        let (parent, name) = parent(path)?;
        if name.capacity() > MAX_SOURCE_EDIT_PATH_BYTES_V1 {
            return Err("bounded source basename capacity exceeds path bound".into());
        }
        let mut file = source_file(&parent, &name)?;
        let observed = regular_metadata(&file)?;
        let declared =
            usize::try_from(observed.len()).map_err(|_| "bounded source length overflow")?;
        let original = read_original(&mut file, declared, maximum)?;
        if !same_snapshot(&observed, &regular_metadata(&file)?) {
            return Err("bounded source changed during read".into());
        }
        let source = Self {
            parent,
            name,
            file,
            observed,
            original,
        };
        source.bounded_retained_storage_v1(maximum)?;
        Ok(source)
    }

    /// Actual logical owned capacities, not allocator overhead or an I/O peak.
    /// The same bound is checked before either new streaming operation.
    pub fn bounded_retained_storage_v1(&self, maximum: usize) -> Result<usize, String> {
        payload_storage(
            self.original.len(),
            self.original.capacity(),
            self.name.capacity(),
            maximum,
        )
    }

    /// Inert original snapshot; a caller must independently join compiler custody.
    pub fn bounded_original_metadata_v1(&self, maximum: usize) -> Result<&Metadata, String> {
        self.bounded_retained_storage_v1(maximum)?;
        Ok(&self.observed)
    }

    /// Streaming same-descriptor/current-basename comparison, no second Vec.
    pub fn recheck_bounded_streaming_v1(&mut self, maximum: usize) -> Result<(), String> {
        self.bounded_retained_storage_v1(maximum)?;
        if !same_snapshot(&self.observed, &regular_metadata(&self.file)?) {
            return Err("bounded source changed before publication".into());
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| "bounded source seek refused")?;
        compare_stream(&mut self.file, &self.original)?;
        if !same_snapshot(&self.observed, &regular_metadata(&self.file)?) {
            return Err("bounded source changed during comparison".into());
        }
        let named = source_file(&self.parent, &self.name)?;
        if !same_snapshot(&self.observed, &regular_metadata(&named)?) {
            return Err("bounded source retained basename changed".into());
        }
        Ok(())
    }
}

/// Uses the original anonymous staging, atomic no-replace link and durability
/// reporting. Only source/staged readbacks use the bounded streaming profile.
/// A failed call can have linked; the original effect-bearing error text stays.
pub fn publish_bounded_streaming_v1(
    source: &mut RetainedSource,
    candidate: &str,
    bytes: &[u8],
    source_limit: usize,
) -> Result<Published, String> {
    publish_profile_inner(
        source,
        candidate,
        bytes,
        Some(source_limit),
        #[cfg(test)]
        None,
    )
}

pub(super) fn verify_staged(
    temporary: &mut File,
    observed: &Metadata,
    expected: &[u8],
) -> Result<(), String> {
    temporary
        .seek(SeekFrom::Start(0))
        .map_err(|_| "bounded staging seek refused")?;
    compare_stream(temporary, expected)?;
    let after = temporary
        .metadata()
        .map_err(|_| "bounded staging metadata refused")?;
    if !after.is_file() || after.nlink() != 0 || !same_snapshot(observed, &after) {
        return Err("bounded staged file identity changed during readback".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_candidate_io_bounded_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_candidate_io_bounded_files_v1_tests.rs"]
mod file_tests;
