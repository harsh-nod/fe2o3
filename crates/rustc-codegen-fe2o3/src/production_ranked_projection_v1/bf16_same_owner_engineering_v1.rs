//! Closed non-test engineering loan of the actual BF16 source/descriptor owner.
//! No ordinary admission, default dispatch, raw extraction or artifact authority.
//! The original projection account meters only this adapter's selected fixed
//! scratch and bounded byte work. Codec/LLVM/finalizer trees, sealed images,
//! child RSS and test-only observation state remain explicit external domains.
use super::*;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerOutputConstraintsV1,
    derive_unfinalized_hsaco_from_finalized_v1, inspect_finalized, inspect_unfinalized,
    observe_engineering_hsaco_v1,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const ENGINE: &str = "BF16 same-owner engineering Worker failed; not a typed source refusal";
const INSPECTION: &str = "BF16 same-owner engineering HSACO inspection failed";
const JOIN: &str = "BF16 same-owner engineering descriptor or owner binding differs";
const UNWIND: &str = "BF16 same-owner engineering loan unwound";
const LIMIT: &str = "BF16 same-owner engineering loan exceeds its bounded child allowance";
#[cfg(test)]
const ENTRY_GUARD: &str = "BF16 same-owner negative control entered the engineering boundary";
const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;
const STREAM_LIMIT: usize = 8 * 1024 * 1024;
const PER_EXECUTION_LIMIT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Engine,
    Inspection,
    Join,
    Limit,
    #[cfg(test)]
    EntryGuard,
}
impl Failure {
    fn reason(self) -> &'static str {
        match self {
            Self::Engine => ENGINE,
            Self::Inspection => INSPECTION,
            Self::Join => JOIN,
            Self::Limit => LIMIT,
            #[cfg(test)]
            Self::EntryGuard => ENTRY_GUARD,
        }
    }
}

// Fixed metadata only. Production consumes it inside the paid closure; no
// production method returns this row. Tests may copy it as harness evidence.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct Row {
    pub(crate) handoff: ContentIdentityV1,
    pub(crate) hsaco: ContentIdentityV1,
    pub(crate) worker: ContentIdentityV1,
    pub(crate) descriptor_sha: [u8; 32],
    pub(crate) descriptor_bytes: usize,
    pub(crate) bootstrap_request: ContentIdentityV1,
    pub(crate) bootstrap_response: ContentIdentityV1,
    pub(crate) replay_request: ContentIdentityV1,
    pub(crate) replay_response: ContentIdentityV1,
}

fn check_limits_at(
    output: &WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    deadline: Instant,
    now: Instant,
) -> Result<(), Failure> {
    let pair = limits
        .timeout()
        .checked_mul(2)
        .and_then(|n| n.checked_add(Duration::from_secs(1)))
        .ok_or(Failure::Limit)?;
    if output.max_bytes() == 0
        || output.max_bytes() > OUTPUT_LIMIT
        || limits.timeout() > PER_EXECUTION_LIMIT
        || limits.stdout_bytes() > STREAM_LIMIT
        || limits.stderr_bytes() > 64 * 1024
        || deadline
            .checked_duration_since(now)
            .is_none_or(|left| left < pair)
    {
        return Err(Failure::Limit);
    }
    Ok(())
}
fn check_limits(
    output: &WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    deadline: Instant,
) -> Result<(), Failure> {
    check_limits_at(output, limits, deadline, Instant::now())
}

fn selected_work(
    handoff: usize,
    descriptor: usize,
    output_bound: u64,
) -> Result<usize, PrivateBf16LlvmErrorV1> {
    // Covers this adapter's bounded byte scans/hashes/comparisons, not external
    // LLVM work or the legacy codec/finalizer engines' internal allocations.
    let output = usize::try_from(output_bound).map_err(|_| arithmetic())?;
    handoff
        .checked_add(descriptor)
        .and_then(|n| n.checked_add(output))
        .and_then(|n| n.checked_mul(32))
        .and_then(|n| n.checked_add(1024))
        .ok_or_else(arithmetic)
}
fn scratch<F, G>() -> Result<usize, PrivateBf16LlvmErrorV1> {
    size_of::<Row>()
        .checked_add(size_of::<F>())
        .and_then(|n| n.checked_add(size_of::<G>()))
        .and_then(|n| n.checked_add(size_of::<Result<Row, Failure>>()))
        .and_then(|n| n.checked_add(size_of::<Result<(), PrivateBf16LlvmErrorV1>>()))
        .ok_or_else(arithmetic)
}

// Captures are moved inside the unwind boundary. An uncalled FnOnce, returned
// row, engine/error payload or emitter all die before known credit is released.
fn run_closed<'work, F, G>(
    budget: &mut Budget<'work>,
    work: usize,
    construct: F,
    emit: G,
) -> Result<(), PrivateBf16LlvmErrorV1>
where
    F: FnOnce() -> Result<Row, Failure>,
    G: FnOnce(Row) -> Result<(), Failure>,
{
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let mut prepaid = 0usize;
    let credit = &mut prepaid;
    let loan = &mut *budget;
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        loan.check_prior_denials_v1()
            .map_err(private_bf16_target_resource_v1)?;
        loan.charge_work(work)
            .map_err(private_bf16_target_resource_v1)?;
        let amount = scratch::<F, G>()?;
        loan.reserve_storage(amount)
            .map_err(private_bf16_target_resource_v1)?;
        *credit = amount;
        let row = construct().map_err(|e| mismatch(e.reason()))?;
        emit(row).map_err(|e| mismatch(e.reason()))
    }));
    let result = match attempted {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(mismatch(UNWIND))
        }
    };
    finish_closed(result, floor, ledger, slot, prepaid, budget)
}

fn finish_closed(
    result: Result<(), PrivateBf16LlvmErrorV1>,
    floor: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    prepaid: usize,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    let prior = budget.check_prior_denials_v1();
    if budget.storage().checked_sub(floor) != Some(prepaid)
        || budget.work_ledger_identity_v1() != ledger
        || budget as *const Budget<'_> as usize != slot
    {
        drop(result);
        prior.map_err(private_bf16_target_resource_v1)?;
        return Err(private_bf16_target_resource_v1(Resource::Accounting));
    }
    // Result owns only a source error or unit. All callback/engine/row payloads
    // have already ended above, including on reserve failure or caught unwind.
    #[cfg(test)]
    tests::before_refund();
    budget
        .release_storage(prepaid)
        .map_err(private_bf16_target_resource_v1)?;
    prior.map_err(private_bf16_target_resource_v1)?;
    result
}

// The closed postflight helper deliberately evaluates replay even on an engine
// error, caught unwind or poisoned original phase. Original-account/source
// refusal has priority; no failed attempt becomes success after replay.
fn after_replay(
    attempted: Result<(), PrivateBf16LlvmErrorV1>,
    clean: Result<(), PrivateBf16LlvmErrorV1>,
    replay: impl FnOnce() -> Result<(), PrivateBf16LlvmErrorV1>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    let replayed = replay();
    clean?;
    replayed?;
    attempted
}

fn equal_descriptor(actual: &[u8], retained: &[u8]) -> Result<(), Failure> {
    if actual != retained {
        return Err(Failure::Join);
    }
    Ok(())
}
fn inspection_error(error: fe2o3_hsaco_finalize::FinalizationError) -> Failure {
    #[cfg(test)]
    eprintln!("fe2o3-bf16-same-owner-engineering-inspection-error-v1 {INSPECTION}");
    drop(error);
    Failure::Inspection
}
fn engineering(
    handoff: &[u8],
    descriptor: &[u8],
    worker: &PinnedWorkerV1,
    output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<Row, Failure> {
    #[cfg(test)]
    tests::before_engine()?;
    // No provider inference or hidden file loading: this initial closed domain
    // is exactly the retained FFI-free handoff and an empty external roster.
    let observation =
        match observe_engineering_hsaco_v1(handoff, worker, Vec::new(), output, limits) {
            Ok(value) => value,
            Err(error) => {
                // A fixed label only: Debug formatting an engine error could expand
                // captured stream bytes. Typed source refusals stay distinct.
                #[cfg(test)]
                eprintln!("fe2o3-bf16-same-owner-engineering-error-v1 {ENGINE}");
                drop(error);
                return Err(Failure::Engine);
            }
        };
    let decoded = CompilerModuleHandoffV2::decode(handoff).map_err(|_| Failure::Join)?;
    if observation.target_profile() != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
        || observation.handoff_identity() != ContentIdentityV1::calculate(handoff)
        || observation.worker_measurement() != worker.measurement()
        || !observation.providers().is_empty()
        || observation.grants_publication_authority()
        || observation.grants_load_authority()
        || observation.grants_launch_authority()
        || observation.authority() != "none"
    {
        return Err(Failure::Join);
    }
    let entry_count = decoded
        .symbol_manifest()
        .entries()
        .filter(|(role, _)| *role == fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1::KernelEntry)
        .count();
    if entry_count != observation.kernel_names().len()
        || !decoded
            .symbol_manifest()
            .entries()
            .filter(|(role, _)| {
                *role == fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1::KernelEntry
            })
            .all(|(_, name)| {
                observation
                    .kernel_names()
                    .iter()
                    .any(|actual| actual.as_str() == name)
            })
    {
        return Err(Failure::Join);
    }
    let inspected = inspect_finalized(observation.hsaco_bytes()).map_err(inspection_error)?;
    if inspected.hsaco().target().to_string() != "gfx942:xnack-"
        || inspected.hsaco().code_object_version().number() != 6
        || inspected.hsaco().kernels().len() != entry_count
        || inspected.digest().as_bytes() != observation.canonical_descriptor_digest()
    {
        return Err(Failure::Join);
    }
    // Existing inspection cross-checks kernel ABI and resource metadata. Full
    // zero-digest canonical bytes must additionally equal the original owner;
    // a self-consistent different object/descriptor cannot pass via hashes.
    let raw = derive_unfinalized_hsaco_from_finalized_v1(observation.hsaco_bytes())
        .map_err(inspection_error)?;
    let unfinalized = inspect_unfinalized(&raw).map_err(inspection_error)?;
    let loc = unfinalized.location();
    let end = loc.offset().checked_add(loc.size()).ok_or(Failure::Join)?;
    equal_descriptor(raw.get(loc.offset()..end).ok_or(Failure::Join)?, descriptor)?;
    let row = Row {
        handoff: observation.handoff_identity(),
        hsaco: observation.finalized_hsaco_identity(),
        worker: observation.worker_measurement().executable(),
        descriptor_sha: Sha256::digest(descriptor).into(),
        descriptor_bytes: descriptor.len(),
        bootstrap_request: observation.bootstrap_request_identity(),
        bootstrap_response: observation.bootstrap_response_identity(),
        replay_request: observation.replay_request_identity(),
        replay_response: observation.replay_response_identity(),
    };
    drop(unfinalized);
    drop(raw);
    drop(inspected);
    drop(decoded);
    drop(observation);
    Ok(row)
}

fn consume_row(row: Row) -> Result<(), Failure> {
    // In non-test builds this fixed inert row dies here. It is never returned
    // to a caller or converted into an artifact/launch capability.
    #[cfg(test)]
    tests::record_row(row);
    let _ = row;
    Ok(())
}

impl PrivateBf16WorkerHandoffV1 {
    /// Fixed synchronous engineering loan. No caller callback, byte getter,
    /// extracted owner, environment selector or ordinary admission is exposed.
    #[allow(dead_code)]
    pub(crate) fn observe_same_owner_engineering_v1(
        &mut self,
        requested: [u8; 4],
        roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        worker: &PinnedWorkerV1,
        output: WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        // Exact typed source refusal wins before limits, decoding or entry.
        self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
        check_limits(&output, limits, deadline).map_err(|e| mismatch(e.reason()))?;
        let work = selected_work(
            self.handoff.canonical.len(),
            self.descriptor.descriptor.canonical_descriptor.len(),
            output.max_bytes(),
        )?;
        let handoff = &self.handoff.canonical;
        let descriptor = &self.descriptor.descriptor.canonical_descriptor;
        let result = self
            .descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                Ok(run_closed(
                    budget,
                    work,
                    || {
                        // Recheck the same deadline after prepayment, directly
                        // before either existing Worker execution can start.
                        check_limits(&output, limits, deadline)?;
                        engineering(handoff, descriptor, worker, output, limits)
                    },
                    consume_row,
                ))
            });
        let clean = self
            .descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1();
        // Fresh replay is evaluated even after an error, unwind or denial.
        // No failed attempt can be upgraded by a successful postflight.
        after_replay(
            result
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)
                .and_then(|value| value),
            clean.map_err(PrivateBf16LlvmErrorV1::RankedVerification),
            || self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile),
        )?;
        require_before_deadline(deadline, Instant::now()).map_err(|e| mismatch(e.reason()))?;
        #[cfg(test)]
        tests::record_postflight();
        Ok(())
    }
}

fn require_before_deadline(deadline: Instant, now: Instant) -> Result<(), Failure> {
    if now >= deadline {
        Err(Failure::Limit)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "bf16_same_owner_engineering_v1_tests.rs"]
mod tests;
