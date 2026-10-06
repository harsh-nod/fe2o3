//! Closed test-only synchronous Worker loan. No ordinary/public admission.
//! The intact source/O/formal/LLVM/descriptor owner stays live for both existing
//! engineering executions. Only a paid fixed row is emitted; no engine owner,
//! byte getter, callback, or generic result crosses the owner boundary.
//! Engine/protocol/ELF Vec/String trees, sealed image and subprocess allocations
//! remain explicitly outside this selected logical account and require separate
//! root runtime-input/containment review. This file issues no execution lease.
use super::*;
use fe2o3_compiler_ffi::{CompilerModuleHandoffErrorV1, CompilerModuleHandoffErrorV2};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerOutputConstraintsV1,
    derive_unfinalized_hsaco_from_finalized_v1, inspect_finalized, inspect_unfinalized,
    observe_engineering_hsaco_v1,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const ENGINE: &str =
    "private BF16 engineering Worker failed; retained diagnostic is not a typed source refusal";
const INSPECTION: &str = "private BF16 engineering HSACO inspection failed";
const JOIN: &str = "private BF16 engineering HSACO full descriptor or owner binding differs";
const UNWIND: &str = "private BF16 synchronous Worker loan unwound";
const LIMIT: &str = "private BF16 synchronous Worker loan exceeds its bounded child allowance";
const ENTRY_GUARD: &str = "private BF16 negative control entered the engineering boundary";
const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;
const STREAM_LIMIT: usize = 8 * 1024 * 1024;
const PER_EXECUTION_LIMIT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Engine,
    Inspection,
    Join,
    Unwind,
    Limit,
    EntryGuard,
}
impl Failure {
    fn reason(self) -> &'static str {
        match self {
            Self::Engine => ENGINE,
            Self::Inspection => INSPECTION,
            Self::Join => JOIN,
            Self::Unwind => UNWIND,
            Self::Limit => LIMIT,
            Self::EntryGuard => ENTRY_GUARD,
        }
    }
}

// Harness-only state in this cfg(test)-only module. A negative case must never
// report engine success: an unexpected actual engineering entry is counted and
// refused before the external API. No global flag, fresh Budget or worker stub.
std::thread_local! {
    static ENTRY_CONTROL: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}
struct EntryScope {
    // The active TLS scope cannot be sent to another thread before Drop.
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl EntryScope {
    fn enter() -> Result<Self, PrivateBf16LlvmErrorV1> {
        ENTRY_CONTROL.with(|cell| {
            if cell.get().is_some() {
                return Err(mismatch("nested private Worker entry control"));
            }
            cell.set(Some(0));
            Ok(Self {
                _thread_bound: std::marker::PhantomData,
            })
        })
    }
    fn entries(&self) -> Option<usize> {
        ENTRY_CONTROL.with(|cell| cell.get())
    }
}
impl Drop for EntryScope {
    fn drop(&mut self) {
        ENTRY_CONTROL.with(|cell| cell.set(None));
    }
}
fn entry_control() -> Result<(), Failure> {
    ENTRY_CONTROL.with(|cell| match cell.get() {
        None => Ok(()),
        Some(count) => {
            cell.set(Some(count.saturating_add(1)));
            Err(Failure::EntryGuard)
        }
    })
}

// A closed value, not a generic callback result or detached HSACO owner.
#[derive(Clone, Copy)]
struct Row {
    handoff: ContentIdentityV1,
    hsaco: ContentIdentityV1,
    worker: ContentIdentityV1,
    descriptor_sha: [u8; 32],
    descriptor_bytes: usize,
    bootstrap_request: ContentIdentityV1,
    bootstrap_response: ContentIdentityV1,
    replay_request: ContentIdentityV1,
    replay_response: ContentIdentityV1,
}
struct Hex<'a>(&'a [u8; 32]);
impl std::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}
impl Row {
    fn emit(self, requested: [u8; 4]) -> Result<(), Failure> {
        // This is explicitly pre-postflight. Only the later postchecked marker
        // can indicate that the full original owner replay also succeeded.
        eprintln!(
            "fe2o3-bf16-private-worker-collected-v1 requested={},{},{},{} handoff_sha256={} handoff_bytes={} hsaco_sha256={} hsaco_bytes={} worker_sha256={} worker_bytes={} descriptor_sha256={} descriptor_bytes={} bootstrap_request_sha256={} bootstrap_response_sha256={} replay_request_sha256={} replay_response_sha256={} worker_invocations=2 full_descriptor_replayed=true engine_owners_dropped=true cleanup_pending=true worker_invoked=true normal_admission=false publication_authority=false load_authority=false launch_authority=false",
            requested[0],
            requested[1],
            requested[2],
            requested[3],
            Hex(self.handoff.sha256()),
            self.handoff.byte_len(),
            Hex(self.hsaco.sha256()),
            self.hsaco.byte_len(),
            Hex(self.worker.sha256()),
            self.worker.byte_len(),
            Hex(&self.descriptor_sha),
            self.descriptor_bytes,
            Hex(self.bootstrap_request.sha256()),
            Hex(self.bootstrap_response.sha256()),
            Hex(self.replay_request.sha256()),
            Hex(self.replay_response.sha256())
        );
        Ok(())
    }
}

fn check_limits(
    output: &WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    deadline: Instant,
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
            .checked_duration_since(Instant::now())
            .is_none_or(|left| left < pair)
    {
        return Err(Failure::Limit);
    }
    Ok(())
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
    eprintln!("fe2o3-bf16-private-worker-inspection-error-v1 {error:?}");
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
    entry_control()?;
    // No provider inference or hidden file loading: this initial closed domain
    // is exactly the retained FFI-free handoff and an empty external roster.
    let observation =
        match observe_engineering_hsaco_v1(handoff, worker, Vec::new(), output, limits) {
            Ok(value) => value,
            Err(error) => {
                // Preserve the existing bounded String-backed engine diagnostic, but
                // never pretend it is the precise typed source/guard refusal enum.
                eprintln!("fe2o3-bf16-private-worker-engine-error-v1 {error:?}");
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

impl PrivateBf16WorkerHandoffV1 {
    /// An explicit test-only call. It is not selected by the old63 endpoint or
    /// any environment variable, and it never returns an engine/HSACO owner.
    #[allow(dead_code)]
    pub(crate) fn observe_private_bf16_worker_for_test_v1(
        &mut self,
        requested: [u8; 4],
        roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        worker: &PinnedWorkerV1,
        output: WorkerOutputConstraintsV1,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        // This exact source/Return/O/guard/descriptor/full-handoff replay wins
        // before limits, inspection, wire decoding or either Worker invocation.
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
                    || engineering(handoff, descriptor, worker, output, limits),
                    |row| row.emit(requested),
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
        // Evaluate the full replay even when execution/refusal/unwind failed.
        // A poisoned original phase stays poisoned; no fresh Budget is created.
        after_replay(
            result
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)
                .and_then(|value| value),
            clean.map_err(PrivateBf16LlvmErrorV1::RankedVerification),
            || self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile),
        )?;
        eprintln!(
            "fe2o3-bf16-private-worker-postchecked-v1 requested={},{},{},{} source_descriptor_handoff_replayed=true selected_storage_restored=true worker_invoked=true cleanup_pending=true normal_admission=false publication_authority=false load_authority=false launch_authority=false",
            requested[0], requested[1], requested[2], requested[3]
        );
        Ok(())
    }
}

// Original-owner negative controls, not a detached admission constructor. The
// existing public-codec/engine allocations are not newly brought into Budget.
impl PrivateBf16WorkerHandoffV1 {
    fn worker_entry_checkpoint_v1(
        &mut self,
    ) -> Result<
        (
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
        ),
        PrivateBf16LlvmErrorV1,
    > {
        self.descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                budget
                    .check_prior_denials_v1()
                    .map_err(E::ConditionalResource)?;
                Ok((
                    budget.work_ledger_identity_v1(),
                    budget.storage(),
                    budget.work(),
                    budget.peak_storage(),
                ))
            })
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)
    }

    #[allow(dead_code)]
    pub(crate) fn exercise_private_bf16_worker_entry_refusals_for_test_v1(
        &mut self,
        requested: [u8; 4],
        roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        worker: &PinnedWorkerV1,
        output_bytes: u64,
        limits: WorkerExecutionLimitsV1,
        deadline: Instant,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        if profile != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 {
            return Err(mismatch("entry controls require the genuine Gfx942 owner"));
        }
        let scope = EntryScope::enter()?;
        self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
        let wrong = if requested == [0, 1, 2, 3] {
            [1, 0, 2, 3]
        } else {
            [0, 1, 2, 3]
        };
        let descriptor_last = self
            .descriptor
            .descriptor
            .canonical_descriptor
            .len()
            .checked_sub(1)
            .ok_or_else(|| mismatch("empty genuine descriptor"))?;
        let text_at = self
            .descriptor
            .descriptor
            .final_llvm
            .find("gfx942")
            .ok_or_else(|| mismatch("genuine final target absent"))?;
        let handoff_last = self
            .handoff
            .canonical
            .len()
            .checked_sub(1)
            .ok_or_else(|| mismatch("empty genuine handoff"))?;
        for fault in 0..6 {
            let before = self.worker_entry_checkpoint_v1()?;
            let output =
                WorkerOutputConstraintsV1::new(output_bytes).map_err(|_| mismatch(LIMIT))?;
            if fault == 3 {
                self.descriptor.descriptor.canonical_descriptor[descriptor_last] ^= 1;
            }
            if fault == 4 {
                self.descriptor
                    .descriptor
                    .final_llvm
                    .get_mut(text_at..text_at + 6)
                    .unwrap()
                    .make_ascii_uppercase();
            }
            if fault == 5 {
                self.handoff.canonical[handoff_last] ^= 1;
            }
            let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.observe_private_bf16_worker_for_test_v1(
                    if fault == 0 { wrong } else { requested },
                    if fault == 2 { &[] } else { roots },
                    if fault == 1 {
                        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950
                    } else {
                        profile
                    },
                    worker,
                    output,
                    limits,
                    deadline,
                )
            }));
            // Restore the same retained allocations before inspecting either the
            // Result or an unwind payload. No replacement box/source owner.
            if fault == 3 {
                self.descriptor.descriptor.canonical_descriptor[descriptor_last] ^= 1;
            }
            if fault == 4 {
                self.descriptor
                    .descriptor
                    .final_llvm
                    .get_mut(text_at..text_at + 6)
                    .unwrap()
                    .make_ascii_lowercase();
            }
            if fault == 5 {
                self.handoff.canonical[handoff_last] ^= 1;
            }
            let result = match attempted {
                Ok(value) => value,
                Err(payload) => {
                    drop(payload);
                    Err(mismatch(
                        "private Worker negative control unwound after restoration",
                    ))
                }
            };
            if scope.entries() != Some(0) {
                drop(result);
                return Err(mismatch(ENTRY_GUARD));
            }
            // Sticky denial wins and unknown credit is never released here.
            let after = self.worker_entry_checkpoint_v1()?;
            if before.0 != after.0
                || before.1 != after.1
                || before.2 >= after.2
                || before.3 > after.3
            {
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            }
            let exact = match (&result, fault) {
                (Err(PrivateBf16LlvmErrorV1::RankedVerification(E::FormalMemory(
                    fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                        fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch)))), 0) => true,
                (Err(PrivateBf16LlvmErrorV1::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure)), 1 | 2) => true,
                (Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                    crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                        "private BF16 descriptor/final LLVM bytes differ from owned replay"))), 3 | 4) => true,
                (Err(PrivateBf16LlvmErrorV1::WorkerHandoff(HandoffError::Handoff(
                    CompilerModuleHandoffErrorV2::Handoff(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch)))), 5) => true,
                _ => false,
            };
            if !exact {
                return match result {
                    Err(error) => Err(error),
                    Ok(()) => Err(mismatch(
                        "private Worker negative control unexpectedly succeeded",
                    )),
                };
            }
            self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
        }
        if scope.entries() != Some(0) {
            return Err(mismatch(ENTRY_GUARD));
        }
        drop(scope);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::cell::Cell;
    fn row() -> Row {
        let id = ContentIdentityV1::from_parts([7; 32], 19);
        Row {
            handoff: id,
            hsaco: id,
            worker: id,
            descriptor_sha: [8; 32],
            descriptor_bytes: 3,
            bootstrap_request: id,
            bootstrap_response: id,
            replay_request: id,
            replay_response: id,
        }
    }
    struct DropCount<'a>(&'a Cell<usize>);
    impl Drop for DropCount<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    fn is_reason(result: &Result<(), PrivateBf16LlvmErrorV1>, reason: &str) -> bool {
        matches!(result, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
            crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(r))) if *r == reason)
    }
    #[test]
    fn closed_loan_success_drops_captures_and_restores_original_floor() {
        let dropped = Cell::new(0);
        let emit_count = Cell::new(0);
        let mut work = Work::new(1000);
        let mut b = Budget::new(&mut work, 4096);
        b.reserve_storage(7).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let capture = DropCount(&dropped);
        run_closed(
            &mut b,
            37,
            move || {
                drop(capture);
                Ok(row())
            },
            |_| {
                emit_count.set(1);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            (dropped.get(), emit_count.get(), b.storage(), b.work()),
            (1, 1, 7, 37)
        );
        assert!(b.work_ledger_identity_v1() == ledger);
        assert!(b.peak_storage() > 7);
    }
    #[test]
    fn closed_loan_refusal_drops_capture_without_emitting_or_refunding_work() {
        let dropped = Cell::new(0);
        let mut work = Work::new(1000);
        let mut b = Budget::new(&mut work, 4096);
        b.reserve_storage(9).unwrap();
        let capture = DropCount(&dropped);
        let got = run_closed(
            &mut b,
            41,
            move || {
                drop(capture);
                Err(Failure::Engine)
            },
            |_| panic!("must not emit"),
        );
        assert!(is_reason(&got, ENGINE));
        assert_eq!((dropped.get(), b.storage(), b.work()), (1, 9, 41));
    }
    #[test]
    fn closed_loan_caught_unwind_drops_payload_and_capture_before_refund() {
        let dropped = Cell::new(0);
        let mut work = Work::new(1000);
        let mut b = Budget::new(&mut work, 4096);
        b.reserve_storage(11).unwrap();
        let capture = DropCount(&dropped);
        let got = run_closed(
            &mut b,
            43,
            move || {
                let _held = capture;
                std::panic::resume_unwind(Box::new("inert loan control"))
            },
            |_| Ok(()),
        );
        assert!(is_reason(&got, UNWIND));
        assert_eq!((dropped.get(), b.storage(), b.work()), (1, 11, 43));
    }
    #[test]
    fn closed_loan_emitter_refusal_or_unwind_restores_known_floor() {
        for unwind in [false, true] {
            let mut work = Work::new(1000);
            let mut b = Budget::new(&mut work, 4096);
            b.reserve_storage(13).unwrap();
            let got = run_closed(
                &mut b,
                47,
                || Ok(row()),
                |_| {
                    if unwind {
                        std::panic::resume_unwind(Box::new("inert emitter control"));
                    }
                    Err(Failure::Join)
                },
            );
            assert!(is_reason(&got, if unwind { UNWIND } else { JOIN }));
            assert_eq!(b.storage(), 13);
        }
    }
    #[test]
    fn closed_loan_work_and_storage_denials_never_enter_callback() {
        for storage_denial in [false, true] {
            let called = Cell::new(false);
            let dropped = Cell::new(0);
            let capture = DropCount(&dropped);
            let mut work = Work::new(if storage_denial { 1000 } else { 0 });
            let mut b = Budget::new(&mut work, if storage_denial { 0 } else { 4096 });
            let got = run_closed(
                &mut b,
                1,
                || {
                    let _held = capture;
                    called.set(true);
                    Ok(row())
                },
                |_| Ok(()),
            );
            assert!(matches!(
                got,
                Err(PrivateBf16LlvmErrorV1::RankedVerification(
                    E::ConditionalResource(Resource::Storage(_) | Resource::Work(_))
                ))
            ));
            assert!(!called.get());
            assert_eq!(dropped.get(), 1);
            assert_eq!(b.storage(), 0);
            assert!(b.check_prior_denials_v1().is_err());
        }
    }
    #[test]
    fn closed_loan_exact_and_one_short_selected_reservation() {
        let construct = || Ok(row());
        let emit = |_| Ok(());
        fn amount<F, G>(_: &F, _: &G) -> usize {
            scratch::<F, G>().unwrap()
        }
        let exact = amount(&construct, &emit);
        let mut w = Work::new(1000);
        let mut b = Budget::new(&mut w, exact);
        run_closed(&mut b, 1, construct, emit).unwrap();
        assert_eq!(b.peak_storage(), exact);
        assert_eq!(b.storage(), 0);
        let mut w = Work::new(1000);
        let mut b = Budget::new(&mut w, exact - 1);
        let got = run_closed(&mut b, 1, construct, emit);
        assert!(matches!(
            got,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Storage(_))
            ))
        ));
        assert_eq!(b.storage(), 0);
    }
    #[test]
    fn foreign_slot_ledger_and_unknown_surplus_never_guess_a_refund() {
        let mut other_work = Work::new(1000);
        let other = Budget::new(&mut other_work, 1000);
        for fault in 0..3 {
            let mut work = Work::new(1000);
            let mut b = Budget::new(&mut work, 1000);
            b.reserve_storage(17).unwrap();
            let ledger = if fault == 0 {
                other.work_ledger_identity_v1()
            } else {
                b.work_ledger_identity_v1()
            };
            let slot = (&b as *const Budget<'_> as usize) + usize::from(fault == 1);
            let prepaid = if fault == 2 { 9 } else { 10 };
            let got = finish_closed(Ok(()), 7, ledger, slot, prepaid, &mut b);
            assert!(matches!(
                got,
                Err(PrivateBf16LlvmErrorV1::RankedVerification(
                    E::ConditionalResource(Resource::Accounting)
                ))
            ));
            assert_eq!(b.storage(), 17);
        }
    }
    #[test]
    fn descriptor_oracle_is_complete_bytes_not_recalculated_identity() {
        let expected = b"zero-digest exact original descriptor";
        assert_eq!(equal_descriptor(expected, expected), Ok(()));
        let mut foreign = expected.to_vec();
        let n = foreign.len();
        foreign[n - 1] ^= 1;
        assert_ne!(Sha256::digest(&foreign), Sha256::digest(expected));
        assert_eq!(equal_descriptor(&foreign, expected), Err(Failure::Join));
        assert_eq!(
            equal_descriptor(&expected[..n - 1], expected),
            Err(Failure::Join)
        );
    }
    #[test]
    fn worker_limits_reserve_both_executions_and_refuse_expired_or_wide_inputs() {
        let limits = WorkerExecutionLimitsV1::new(Duration::from_secs(1), 1024, 1024).unwrap();
        let output = WorkerOutputConstraintsV1::new(1024).unwrap();
        assert_eq!(
            check_limits(&output, limits, Instant::now() + Duration::from_secs(5)),
            Ok(())
        );
        assert_eq!(
            check_limits(&output, limits, Instant::now()),
            Err(Failure::Limit)
        );
        assert_eq!(
            check_limits(
                &WorkerOutputConstraintsV1::new(OUTPUT_LIMIT + 1).unwrap(),
                limits,
                Instant::now() + Duration::from_secs(5)
            ),
            Err(Failure::Limit)
        );
        let wide = WorkerExecutionLimitsV1::new(Duration::from_secs(121), 1024, 1024).unwrap();
        assert_eq!(
            check_limits(&output, wide, Instant::now() + Duration::from_secs(600)),
            Err(Failure::Limit)
        );
    }
    #[test]
    fn replay_runs_after_success_error_and_unwind_and_cannot_promote_failure() {
        for prior in [None, Some(ENGINE), Some(UNWIND)] {
            let calls = Cell::new(0);
            let attempted = prior.map_or(Ok(()), |reason| Err(mismatch(reason)));
            let got = after_replay(attempted, Ok(()), || {
                calls.set(calls.get() + 1);
                Ok(())
            });
            assert_eq!(calls.get(), 1);
            if let Some(reason) = prior {
                assert!(is_reason(&got, reason));
            } else {
                assert!(got.is_ok());
            }
        }
    }
    #[test]
    fn replay_source_or_original_account_refusal_dominates_engine_failure() {
        let calls = Cell::new(0);
        let got = after_replay(Err(mismatch(ENGINE)), Ok(()), || {
            calls.set(1);
            Err(mismatch(JOIN))
        });
        assert!(is_reason(&got, JOIN));
        assert_eq!(calls.get(), 1);
        let mut denied_work = Work::new(0);
        let mut denied_budget = Budget::new(&mut denied_work, 0);
        let denied = denied_budget.charge_work(1).unwrap_err();
        assert!(matches!(denied, Resource::Work(_)));
        let got = after_replay(
            Err(mismatch(ENGINE)),
            Err(private_bf16_target_resource_v1(denied)),
            || {
                calls.set(2);
                Err(mismatch(JOIN))
            },
        );
        assert!(matches!(
            got,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Work(_))
            ))
        ));
        assert_eq!(calls.get(), 2);
    }
    #[test]
    fn owner_loan_source_guard_and_old_endpoint_have_no_escaping_engine_owner() {
        let src = include_str!("bf16_private_worker_observation_v1_tests.rs");
        let owner = src
            .split("impl PrivateBf16WorkerHandoffV1 {")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(
            owner
                .find("self.revalidate_private_bf16_worker_handoff_v1")
                .unwrap()
                < owner.find("check_limits(").unwrap()
        );
        assert!(owner.contains("after_replay("));
        assert!(owner.contains("|| self.revalidate_private_bf16_worker_handoff_v1"));
        assert!(!owner.contains("Budget::new"));
        assert!(!owner.contains("-> Result<Row"));
        assert!(!owner.contains("pub(crate) fn canonical_bytes"));
        let production = include_str!("bf16_private_worker_handoff_v1.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(!production.contains("observe_engineering_hsaco_v1"));
    }

    #[test]
    fn entry_guard_counts_and_refuses_without_fabricating_engine_success() {
        assert_eq!(entry_control(), Ok(()));
        let scope = EntryScope::enter().unwrap();
        assert_eq!(scope.entries(), Some(0));
        assert_eq!(entry_control(), Err(Failure::EntryGuard));
        assert_eq!(scope.entries(), Some(1));
        assert_eq!(entry_control(), Err(Failure::EntryGuard));
        assert_eq!(scope.entries(), Some(2));
        drop(scope);
        assert_eq!(entry_control(), Ok(()));
    }
    #[test]
    fn entry_guard_nested_refusal_does_not_clear_outer_scope() {
        let scope = EntryScope::enter().unwrap();
        assert!(EntryScope::enter().is_err());
        assert_eq!(scope.entries(), Some(0));
        assert_eq!(entry_control(), Err(Failure::EntryGuard));
        assert_eq!(scope.entries(), Some(1));
        drop(scope);
        assert_eq!(entry_control(), Ok(()));
    }
    #[test]
    fn entry_guard_resets_on_unwind_without_global_panic_hook() {
        let result = std::panic::catch_unwind(|| {
            let _scope = EntryScope::enter().unwrap();
            assert_eq!(entry_control(), Err(Failure::EntryGuard));
            std::panic::resume_unwind(Box::new("inert entry scope unwind"));
        });
        assert!(result.is_err());
        drop(result);
        assert_eq!(entry_control(), Ok(()));
        let scope = EntryScope::enter().unwrap();
        assert_eq!(scope.entries(), Some(0));
    }
    #[test]
    fn entry_guard_is_thread_local() {
        let scope = EntryScope::enter().unwrap();
        std::thread::spawn(|| {
            assert_eq!(entry_control(), Ok(()));
            let local = EntryScope::enter().unwrap();
            assert_eq!(entry_control(), Err(Failure::EntryGuard));
            assert_eq!(local.entries(), Some(1));
        })
        .join()
        .unwrap();
        assert_eq!(scope.entries(), Some(0));
        assert_eq!(entry_control(), Err(Failure::EntryGuard));
        assert_eq!(scope.entries(), Some(1));
    }
    #[test]
    fn entry_guard_is_first_at_actual_engine_boundary_and_owner_controls_are_closed() {
        let source = include_str!("bf16_private_worker_observation_v1_tests.rs");
        let engineering = source
            .split("fn engineering(")
            .nth(1)
            .unwrap()
            .split("impl PrivateBf16WorkerHandoffV1 {")
            .next()
            .unwrap();
        assert!(engineering.contains(") -> Result<Row, Failure> {\n    entry_control()?;"));
        assert!(
            engineering.find("entry_control()?").unwrap()
                < engineering.find("observe_engineering_hsaco_v1(").unwrap()
        );
        let controls = source
            .split("pub(crate) fn exercise_private_bf16_worker_entry_refusals_for_test_v1(")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(controls.contains("for fault in 0..6"));
        assert!(controls.contains("scope.entries() != Some(0)"));
        assert!(controls.contains("self.observe_private_bf16_worker_for_test_v1("));
        assert!(!controls.contains("Budget::new"));
        assert!(!controls.contains("observe_engineering_hsaco_v1("));
    }
}
