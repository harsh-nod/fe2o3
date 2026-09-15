//! Hashes the existing canonical byte stream; does not admit or verify a module.
use super::*;
use crate::CanonicalKernelIrVersionV1;
use sha2::{Digest, Sha256};

pub(super) enum Output {
    Bytes(Vec<u8>),
    Count,
    Digest(Sha256),
}
impl Output {
    pub(super) fn write(&mut self, bytes: &[u8]) {
        match self {
            Self::Bytes(output) => output.extend_from_slice(bytes),
            Self::Count => {}
            Self::Digest(hash) => hash.update(bytes),
        }
    }
}

pub(super) struct Work {
    initial: usize,
    remaining: usize,
}

/// Digest of exact encoded bytes, not a verified canonical owner or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalModuleDigestV1 {
    digest: [u8; 32],
    length: u64,
}
impl CanonicalModuleDigestV1 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn canonical_length(&self) -> u64 {
        self.length
    }
}

/// Logical fixed digest workspace: one live writer and its result. Excludes
/// existing encoder validation/subpayload allocations, allocator and stack RSS.
pub const KERNEL_IR_DIGEST_WORKSPACE_BYTES_V1: usize =
    std::mem::size_of::<Writer>() + std::mem::size_of::<CanonicalModuleDigestV1>();

/// Computes SHA-256 and length without retaining the module's encoded bytes.
///
/// The header commits the final length, so the same encoder runs once into a
/// counting sink and once into SHA-256. Both passes debit `remaining_work`;
/// failed passes keep their debit. Each emitted slice costs its byte length
/// plus one, with an additional conservative role-validation charge. The byte
/// limit is clamped to the existing codec ceiling. All codec predicates remain
/// shared with `encode_module_v13`/`encode_module_v14`.
pub fn canonical_module_digest_v1(
    module: &Module,
    version: CanonicalKernelIrVersionV1,
    maximum_bytes: usize,
    remaining_work: &mut usize,
) -> Result<CanonicalModuleDigestV1, KernelIrEncodeError> {
    let version = match version {
        CanonicalKernelIrVersionV1::V13 => KERNEL_IR_VERSION_V13,
        CanonicalKernelIrVersionV1::V14 => KERNEL_IR_VERSION_V14,
    };
    let limit = maximum_bytes.min(MAX_MODULE_BYTES_V1);
    // Drop the counting writer before constructing the hashing writer.
    let length = {
        let mut writer = Writer::streaming(version, limit, *remaining_work, Output::Count);
        let result = encode_module_into(&mut writer, module, 0);
        *remaining_work = writer.work.as_ref().unwrap().remaining;
        result?;
        u32::try_from(writer.length).map_err(|_| KernelIrEncodeError::Overflow {
            field: "module length",
        })?
    };
    let mut writer = Writer::streaming(version, limit, *remaining_work, Output::Digest(Sha256::new()));
    let result = encode_module_into(&mut writer, module, length);
    *remaining_work = writer.work.as_ref().unwrap().remaining;
    result?;
    if writer.length != length as usize {
        return Err(KernelIrEncodeError::NonCanonical { field: "canonical digest length" });
    }
    let Output::Digest(hash) = writer.output else { unreachable!("hashing writer") };
    Ok(CanonicalModuleDigestV1 { digest: hash.finalize().into(), length: u64::from(length) })
}

impl Writer {
    fn streaming(version: u16, byte_limit: usize, work: usize, output: Output) -> Self {
        Self { output, version, length: 0, byte_limit, work: Some(Work { initial: work, remaining: work }) }
    }
    pub(super) fn charge(&mut self, count: usize) -> Result<(), KernelIrEncodeError> {
        let Some(work) = &mut self.work else { return Ok(()) };
        match work.remaining.checked_sub(count) {
            Some(next) => { work.remaining = next; Ok(()) }
            None => {
                let actual = (work.initial - work.remaining).saturating_add(count);
                work.remaining = 0;
                Err(KernelIrEncodeError::LimitExceeded {
                    field: "canonical digest work", actual, max: work.initial,
                })
            }
        }
    }
    pub(super) fn charge_role_validation(&mut self, module: &Module) -> Result<(), KernelIrEncodeError> {
        if self.work.is_none() { return Ok(()) }
        // The shared legacy validator builds an ordered entry set. Precharge
        // its roster scan, name copies and worst-case name comparisons, not a
        // second format-specific size calculation.
        let mut max_name = 0usize;
        for id in module.kernels.iter().map(|k| &k.entry).chain(module.functions.iter().map(|f| &f.id)) {
            self.charge(1)?;
            max_name = max_name.max(id.as_str().len());
        }
        let rows = module.kernels.len().checked_add(module.functions.len());
        // BTreeSet's exact balancing is not a wire contract. A per-operation
        // roster bound also covers an unbalanced search and duplicate entries.
        let count = rows.and_then(|n| n.checked_mul(module.kernels.len().saturating_add(1)))
            .and_then(|n| n.checked_mul(max_name.saturating_add(1)))
            .ok_or(KernelIrEncodeError::Overflow { field: "canonical digest role work" })?;
        self.charge(count)
    }
}

#[cfg(test)]
#[path = "canonical_digest/tests.rs"]
mod tests;
