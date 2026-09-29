//! Inert, conservative V5 operation schedules, charged on the original ledger.
use super::*;
use crate::attempt::{
    AttemptRecord, MAX_ATTEMPT_BYTES, MAX_ATTEMPT_RECORDS, MAX_CRATE_NAME_BYTES,
    MAX_STABLE_SOURCE_BYTES,
};
use fe2o3_compiler_ffi::inert_semantic_compiler_module_handoff_decode_work_v5;

/// Additional work and peak logical scratch above already-reserved input owners.
/// Includes returned-owner construction overlap, not allocator RSS or an I/O
/// deadline. A quote grants no authority and reserves or resets no resources.
/// Process-global runtime/lock-table bookkeeping is not per-request ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffOperationQuotaV5 {
    work: usize,
    scratch: usize,
}

impl CompilerModuleHandoffOperationQuotaV5 {
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn scratch(self) -> usize {
        self.scratch
    }

    pub(super) fn prepay(self, resources: &mut Resources<'_, '_>) -> Result<()> {
        resources.reserve(self.scratch)?;
        resources.work(self.work)?;
        Ok(())
    }
}

fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |a, b| {
        a.checked_add(*b).ok_or(Resource::Arithmetic.into())
    })
}

fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}

pub(super) fn validate_length(length: usize) -> Result<()> {
    if length == 0 || length > MAX_COMPILER_MODULE_HANDOFF_BYTES_V5 {
        return Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::InvalidHandoffSize {
                actual: length,
                maximum: MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
            },
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum Operation {
    Recovery,
    Lease,
    Token,
    Currentness,
}

/// The shared filesystem engine predates resource ledgers. Prepay its bounded
/// control/metadata work here, in addition to its existing payload/record debits.
/// One authorization may replay a recovery registry then read the live registry:
/// two decodes, each validating and canonically re-encoding, but never concurrent.
/// The BTree allowance includes spare key/value slots, edges, generation sets,
/// and transient node splitting (4 slots per record plus a spare root).
pub(super) fn filesystem(
    dynamic: usize,
    op: Operation,
) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    let node_slots = sum(&[mul(4, MAX_ATTEMPT_RECORDS)?, 16])?;
    let node_storage = mul(
        node_slots,
        sum(&[
            size_of::<AttemptRecord>(),
            size_of::<String>(),
            16 * size_of::<usize>(),
        ])?,
    )?;
    // read_to_end may double through maximum+1; re-encoding and decoded text
    // coexist. Include fixed receipt codecs/registry headers in a separate frame.
    let registry_storage = sum(&[mul(4, MAX_ATTEMPT_BYTES + 1)?, node_storage])?;
    let levels = (usize::BITS - MAX_ATTEMPT_RECORDS.leading_zeros()) as usize;
    let comparison_work = mul(
        mul(16, levels)?,
        mul(MAX_ATTEMPT_RECORDS, MAX_STABLE_SOURCE_BYTES)?,
    )?;
    let registry_work = sum(&[
        comparison_work,
        mul(128, MAX_ATTEMPT_BYTES)?,
        mul(64, node_storage)?,
    ])?;
    let (authorizations, stale) = match op {
        Operation::Recovery => (2, true),
        Operation::Lease => (3, true),
        Operation::Token => (2, false),
        Operation::Currentness => (1, false),
    };
    // Linux directory entries are at most 255 bytes. Budget doubled Vec slots,
    // two nested directory scans, and names retained while stale slots drain.
    let entries = if stale {
        MAX_STALE_SLOTS + MAX_SLOT_ENTRIES + 2
    } else {
        MAX_SLOT_ENTRIES + 2
    };
    let directory_storage = sum(&[2 * 64 * 1024, mul(2 * entries, 256 + size_of::<PathBuf>())?])?;
    let fixed = sum(&[
        size_of::<currentness::Current<Schema>>(),
        8 * size_of::<HandoffRecord<Schema>>(),
        8 * size_of::<AttemptRecord>(),
        8 * resources::fixed_scope_overhead::<CompilerModuleHandoffReceiptV5>(),
        8 * size_of::<Sha256>(),
    ])?;
    let visits = if stale {
        (MAX_STALE_SLOTS + 1) * (MAX_SLOT_ENTRIES + 1)
    } else {
        MAX_SLOT_ENTRIES + 1
    };
    Ok(CompilerModuleHandoffOperationQuotaV5 {
        work: sum(&[
            mul(2 * authorizations, registry_work)?,
            mul(64 * visits, sum(&[dynamic, 256])?)?,
            mul(8, fixed)?,
        ])?,
        scratch: sum(&[registry_storage, directory_storage, mul(8, dynamic)?, fixed])?,
    })
}

fn operation(
    dynamic: usize,
    op: Operation,
    work: usize,
    scratch: usize,
) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    let filesystem = filesystem(dynamic, op)?;
    Ok(CompilerModuleHandoffOperationQuotaV5 {
        work: sum(&[8, filesystem.work, work])?,
        scratch: sum(&[FRAME_STORAGE, filesystem.scratch, scratch])?,
    })
}

fn recovery(dynamic: usize, length: usize) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    validate_length(length)?;
    let decode = inert_semantic_compiler_module_handoff_decode_work_v5(length)
        .map_err(Error::NonCanonicalHandoff)?;
    operation(
        dynamic,
        Operation::Recovery,
        sum(&[12 * schema::RECORD_BYTES, mul(2, length)?, decode, 260])?,
        sum(&[
            size_of::<Sha256>(),
            2 * schema::RECORD_BYTES,
            length,
            METADATA,
        ])?,
    )
}

/// Full limited-recovery schedule for these input capacities and an inert ceiling.
/// The ceiling can exceed the actual receipt, but must fit the canonical schema.
/// The returned scratch may exceed the entry cap; callers must reject such plans.
pub fn compiler_module_handoff_try_recovery_quota_v5(
    output: &Path,
    producer: &ProducerIdentity,
    maximum_handoff_bytes: usize,
) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    recovery(dynamic_bound(output, producer)?, maximum_handoff_bytes)
}

pub(super) fn lease_storage(dynamic: usize) -> Result<usize> {
    sum(&[
        CURRENT_STORAGE,
        size_of::<CompilerModuleHandoffCurrentnessLeaseV5>(),
        dynamic,
    ])
}

pub(super) fn token_storage(dynamic: usize, length: usize) -> Result<usize> {
    sum(&[token_headers::<Handoff>(), dynamic, length, METADATA])
}

fn lease(dynamic: usize, length: usize) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    operation(
        dynamic,
        Operation::Lease,
        sum(&[
            mul(4, dynamic)?,
            3 * currentness::record_work::<Schema>(),
            currentness::stream_work(length)?,
        ])?,
        sum(&[
            lease_storage(dynamic)?,
            currentness::STREAM_STORAGE,
            currentness::record_scratch::<Schema>(),
        ])?,
    )
}

fn token(dynamic: usize, length: usize) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    let decode = inert_semantic_compiler_module_handoff_decode_work_v5(length)
        .map_err(Error::NonCanonicalHandoff)?;
    operation(
        dynamic,
        Operation::Token,
        sum(&[
            mul(3, length)?,
            257,
            2 * currentness::record_work::<Schema>(),
            decode,
        ])?,
        sum(&[
            token_storage(dynamic, length)?,
            size_of::<Sha256>(),
            currentness::record_scratch::<Schema>(),
        ])?,
    )
}

fn currentness(dynamic: usize) -> Result<CompilerModuleHandoffOperationQuotaV5> {
    operation(
        dynamic,
        Operation::Currentness,
        currentness::record_work::<Schema>(),
        currentness::record_scratch::<Schema>(),
    )
}

impl CompilerModuleHandoffCurrentnessCustodyQuoteV5 {
    pub fn lease_acquisition_quota(&self) -> Result<CompilerModuleHandoffOperationQuotaV5> {
        lease(self.dynamic, self.receipt.length)
    }
    pub fn token_acquisition_quota(&self) -> Result<CompilerModuleHandoffOperationQuotaV5> {
        token(self.dynamic, self.receipt.length)
    }
    pub fn currentness_revalidation_quota(&self) -> Result<CompilerModuleHandoffOperationQuotaV5> {
        currentness(self.dynamic)
    }
}

fn current_dynamic(binding: &currentness::Current<Schema>) -> Result<usize> {
    // Ordinary tokens need not meet quoted constructors' path-guard ceiling.
    Ok(currentness::location_storage(
        &binding.output,
        &binding.producer,
        &binding.parent,
        &binding.slot_directory,
    )?)
}

impl<T> CompilerModuleHandoffConsumptionTokenV5<T> {
    /// Metadata/currentness schedule only; never decodes or calls the payload.
    pub fn currentness_revalidation_quota(&self) -> Result<CompilerModuleHandoffOperationQuotaV5> {
        currentness(current_dynamic(&self.binding)?)
    }
}

pub(in super::super) fn prepay_currentness(
    binding: &currentness::Current<Schema>,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    filesystem(current_dynamic(binding)?, Operation::Currentness)?.prepay(resources)
}

/// Receipt-free maximum plan for an explicitly bounded input shape. Call
/// `validate_inputs` before applying this schedule; exact receipt quotes remain
/// available for larger shapes and do not silently inherit these restrictions.
/// Neither this record nor its size ceiling is publication/currentness authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffCustodyQuotaV5 {
    maximum_handoff_bytes: usize,
    retained_storage: usize,
    recovery: CompilerModuleHandoffOperationQuotaV5,
    lease: CompilerModuleHandoffOperationQuotaV5,
    token: CompilerModuleHandoffOperationQuotaV5,
    currentness: CompilerModuleHandoffOperationQuotaV5,
}

impl CompilerModuleHandoffCustodyQuotaV5 {
    pub const fn maximum_handoff_bytes(self) -> usize {
        self.maximum_handoff_bytes
    }
    pub const fn retained_storage(self) -> usize {
        self.retained_storage
    }
    pub const fn try_recovery_quota(self) -> CompilerModuleHandoffOperationQuotaV5 {
        self.recovery
    }
    pub const fn lease_acquisition_quota(self) -> CompilerModuleHandoffOperationQuotaV5 {
        self.lease
    }
    pub const fn token_acquisition_quota(self) -> CompilerModuleHandoffOperationQuotaV5 {
        self.token
    }
    pub const fn currentness_revalidation_quota(self) -> CompilerModuleHandoffOperationQuotaV5 {
        self.currentness
    }

    /// Output bytes <=16 KiB, stable-source capacity <=4096, crate capacity <=128.
    /// The existing quoted constructor separately checks actual retained paths,
    /// including a <=16 KiB guard. Capacities, not just string lengths, matter.
    pub fn validate_inputs(&self, output: &Path, producer: &ProducerIdentity) -> Result<()> {
        if output.as_os_str().len() > PATH_GUARD_CAPACITY
            || producer.stable_source.capacity() > MAX_STABLE_SOURCE_BYTES
            || producer.crate_name.capacity() > MAX_CRATE_NAME_BYTES
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    /// Bind the inert plan to exact constructor inputs without fabricating a receipt.
    pub fn quote_currentness(
        &self,
        output: &Path,
        producer: &ProducerIdentity,
        receipt: CompilerModuleHandoffReceiptV5,
    ) -> Result<CompilerModuleHandoffCurrentnessCustodyQuoteV5> {
        self.validate_inputs(output, producer)?;
        if receipt.length > self.maximum_handoff_bytes {
            return Err(Error::Coordination(
                CompilerModuleHandoffErrorV1::InvalidHandoffSize {
                    actual: receipt.length,
                    maximum: self.maximum_handoff_bytes,
                },
            ));
        }
        quote_compiler_module_handoff_currentness_custody_v5(output, producer, receipt)
    }
}

/// Receipt-free complete operation/retention maxima. Add work across calls; add
/// each operation's scratch to whatever owners the caller retains at that point.
/// In particular, late custody's prepaid maximum is NOT included in scratch.
/// The enclosing caller also accounts for its own quota/holder record headers.
pub fn compiler_module_handoff_custody_quota_for_limit_v5(
    maximum_handoff_bytes: usize,
) -> Result<CompilerModuleHandoffCustodyQuotaV5> {
    validate_length(maximum_handoff_bytes)?;
    let dynamic = dynamic_bound_for_lengths(
        PATH_GUARD_CAPACITY,
        MAX_STABLE_SOURCE_BYTES,
        MAX_CRATE_NAME_BYTES,
    )?;
    Ok(CompilerModuleHandoffCustodyQuotaV5 {
        maximum_handoff_bytes,
        retained_storage: sum(&[
            lease_storage(dynamic)?,
            token_storage(dynamic, maximum_handoff_bytes)?,
        ])?,
        recovery: recovery(dynamic, maximum_handoff_bytes)?,
        lease: lease(dynamic, maximum_handoff_bytes)?,
        token: token(dynamic, maximum_handoff_bytes)?,
        currentness: currentness(dynamic)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceiling_schedule_fits_small_publications_but_not_canonical_maximum() {
        let small = compiler_module_handoff_custody_quota_for_limit_v5(1024 * 1024).unwrap();
        let large = compiler_module_handoff_custody_quota_for_limit_v5(
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
        )
        .unwrap();
        assert!(
            small.retained_storage() + small.token_acquisition_quota().scratch()
                < 3 * MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 / 4
        );
        assert!(small.try_recovery_quota().scratch() < MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 / 2);
        assert!(large.try_recovery_quota().scratch() > MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5);
        assert!(large.try_recovery_quota().work() > 11_092_469_018);
        eprintln!("V5 1MiB plan {small:?}; canonical-max plan {large:?}");
    }

    #[test]
    fn quota_arithmetic_and_invalid_ceiling_refuse_without_an_account() {
        for n in [0, MAX_COMPILER_MODULE_HANDOFF_BYTES_V5 + 1, usize::MAX] {
            assert!(matches!(
                compiler_module_handoff_custody_quota_for_limit_v5(n),
                Err(Error::Coordination(
                    CompilerModuleHandoffErrorV1::InvalidHandoffSize { .. }
                ))
            ));
        }
        assert!(matches!(
            sum(&[usize::MAX, 1]),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert!(matches!(
            mul(usize::MAX, 2),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert!(matches!(
            dynamic_bound_for_lengths(usize::MAX, 0, 0),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert!(matches!(
            filesystem(usize::MAX, Operation::Recovery),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }
}
