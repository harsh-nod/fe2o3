//! Actual exec-derived kernel image metadata, never pathname-based authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
use sha2::{Digest, Sha256};

/// Private metadata captured from an actual held exec, not caller-supplied rows.
/// The same account and live stopped observations are required for every check.
/// Fork descendants may inherit it; a different actual exec must capture anew.
/// This does not establish a compiler guard, syscall policy or immutable source.
pub(crate) struct NativeKernelImage {
    interval: (u64, u64),
    digest: [u8; 32],
    ledger: Ledger,
    address: usize,
}

impl NativeKernelImage {
    pub(crate) const STORAGE: usize = size_of::<Self>();
    pub(crate) const FRAME: usize = super::FRAME
        + View::AUXV_BYTES
        + View::MEMORY_BYTES
        + size_of::<policy::elf::ExecutableFileRanges>()
        + 4 * size_of::<Self>();
    const PARSE_WORK: usize = 8
        + policy::MAX_MAP_BYTES * 64
        + View::MEMORY_BYTES * 64
        + policy::elf::MAX_PROGRAM_HEADERS * (4096 + policy::elf::PROGRAM_HEADER_BYTES * 64);
    pub(crate) const CAPTURE_WORK: usize =
        Self::PARSE_WORK + View::AUXV_WORK + View::MAPS_WORK + View::MEMORY_WORK;
    pub(crate) const VALIDATE_WORK: usize = Self::PARSE_WORK + View::AUXV_WORK + View::MEMORY_WORK;
    pub(crate) const CAPTURE_SCRATCH: usize = Self::FRAME
        + MAPS_STORAGE
        + View::FRAME
        + View::AUXV_BYTES
        + 16
        + View::MAPS_SCRATCH
        + MAPS_STORAGE;
    pub(crate) const VALIDATE_SCRATCH: usize = Self::FRAME + View::FRAME + View::AUXV_BYTES + 16;

    pub(crate) const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }

    /// Returned storage is FULL and unreserved. The owner retains it with the
    /// same attempt and discards/replaces it only on a newly observed exec.
    pub(crate) fn capture(view: &View<'_, '_>, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(0, 8, Self::PARSE_WORK, Self::FRAME + MAPS_STORAGE, |b| {
            if !view.is_exec_boundary() {
                return Err(Error::Invalid(
                    "kernel image capture requires an actual held exec",
                ));
            }
            let (auxv, count) = view.read_auxv(b)?;
            let address = sysinfo_ehdr(&auxv[..count])?;
            with_maps(view, b, |maps, b| {
                let interval = kernel_interval(maps, address)?;
                let mut bytes = [0; View::MEMORY_BYTES];
                let image = &mut bytes[..(interval.1 - interval.0) as usize];
                view.read_memory(interval.0, image, b)?;
                require_complete_elf(image)?;
                Ok(Self {
                    interval,
                    digest: Sha256::digest(image).into(),
                    ledger: b.work_ledger_identity_v1(),
                    address: b as *const Budget<'_> as usize,
                })
            })
        })
    }

    /// Compare exact actual auxv/layout/bytes with the retained exec observation.
    /// No supplied map label or fixed userspace address can authorize execution.
    pub(super) fn validate(
        &self,
        view: &View<'_, '_>,
        maps: &str,
        b: &mut Budget<'_>,
    ) -> Result<(u64, u64)> {
        b.with_prepaid_scope(Self::STORAGE, 8, Self::PARSE_WORK, Self::FRAME, |b| {
            if self.ledger != b.work_ledger_identity_v1()
                || self.address != b as *const Budget<'_> as usize
            {
                return Err(Resource::Accounting.into());
            }
            let (auxv, count) = view.read_auxv(b)?;
            if sysinfo_ehdr(&auxv[..count])? != self.interval.0
                || kernel_interval(maps, self.interval.0)? != self.interval
            {
                return Err(Error::Invalid(
                    "kernel image layout differs from its original exec",
                ));
            }
            let mut bytes = [0; View::MEMORY_BYTES];
            let image = &mut bytes[..(self.interval.1 - self.interval.0) as usize];
            view.read_memory(self.interval.0, image, b)?;
            let digest: [u8; 32] = Sha256::digest(image).into();
            if digest != self.digest {
                return Err(Error::Invalid("kernel image bytes changed after exec"));
            }
            Ok(self.interval)
        })
    }
}

fn sysinfo_ehdr(auxv: &[u8]) -> Result<u64> {
    if auxv.is_empty() || auxv.len() > View::AUXV_BYTES || !auxv.len().is_multiple_of(16) {
        return Err(Error::Invalid(
            "kernel auxiliary vector has invalid native record bounds",
        ));
    }
    let mut sysinfo = None;
    let mut terminal = false;
    for pair in auxv.chunks_exact(16) {
        let kind = u64::from_ne_bytes(pair[..8].try_into().expect("fixed auxv pair"));
        let value = u64::from_ne_bytes(pair[8..].try_into().expect("fixed auxv pair"));
        if terminal {
            return Err(Error::Invalid(
                "kernel auxiliary vector has trailing records",
            ));
        }
        if kind == 0 {
            if value != 0 {
                return Err(Error::Invalid(
                    "kernel auxiliary vector terminator is not zero",
                ));
            }
            terminal = true;
        } else if kind == 33
            && (sysinfo.replace(value).is_some() || value == 0 || value & 4095 != 0)
        {
            return Err(Error::Invalid(
                "kernel auxiliary vector has invalid SYSINFO_EHDR",
            ));
        }
    }
    if !terminal {
        return Err(Error::Invalid("kernel auxiliary vector is not terminated"));
    }
    sysinfo.ok_or(Error::Invalid("kernel auxiliary vector lacks SYSINFO_EHDR"))
}

fn kernel_interval(maps: &str, address: u64) -> Result<(u64, u64)> {
    if maps.len() > policy::MAX_MAP_BYTES {
        return Err(Error::Invalid(
            "kernel mapping observation exceeds byte bound",
        ));
    }
    let mut found = None;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let (start, end) = policy::parse_mapping_range(
            fields
                .next()
                .ok_or(Error::Invalid("kernel map lacks range"))?,
        )?;
        if start != address {
            continue;
        }
        if found.is_some()
            || fields.next() != Some("r-xp")
            || policy::parse_mapping_file_offset(
                fields
                    .next()
                    .ok_or(Error::Invalid("kernel map lacks offset"))?,
            )? != 0
            || fields.next() != Some("00:00")
            || fields.next() != Some("0")
            || start & 4095 != 0
            || end & 4095 != 0
            || end - start > View::MEMORY_BYTES as u64
        {
            return Err(Error::Invalid(
                "actual SYSINFO_EHDR mapping is not a bounded private RX kernel image",
            ));
        }
        found = Some((start, end));
    }
    found.ok_or(Error::Invalid(
        "actual SYSINFO_EHDR has no complete kernel mapping",
    ))
}

fn require_complete_elf(image: &[u8]) -> Result<()> {
    if image.len() < policy::elf::HEADER_BYTES || image.len() > View::MEMORY_BYTES {
        return Err(Error::Invalid(
            "kernel ELF image exceeds captured byte bounds",
        ));
    }
    let ranges = policy::elf::executable_file_ranges::<Error>(
        image.len() as u64,
        &image[..policy::elf::HEADER_BYTES],
        |offset, program| {
            let start = usize::try_from(offset)
                .map_err(|_| Error::Invalid("kernel ELF table offset overflow"))?;
            let end = start
                .checked_add(program.len())
                .ok_or(Error::Invalid("kernel ELF table range overflow"))?;
            let bytes = image
                .get(start..end)
                .ok_or(Error::Invalid("kernel ELF program table exceeds image"))?;
            program.copy_from_slice(bytes);
            Ok(program.len())
        },
    )?
    .ok_or(Error::Invalid("actual kernel image is not native ELF64"))?;
    if !policy::contains_file_range(0, image.len() as u64, ranges.as_slice())? {
        return Err(Error::Invalid(
            "actual kernel executable mapping exceeds its ELF load ranges",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_runtime_kernel_image_tests.rs"]
mod tests;
