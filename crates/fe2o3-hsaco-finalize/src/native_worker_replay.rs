//! Reconstruct native source/Worker/artifact custody without launching a process.

use std::{fmt, mem::size_of};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV4, ProducerIdentity,
    rederive_compiler_module_handoff_receipt_for_replay_v4,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4;

use crate::{
    InertNativeFirstBuildWorkerEvidenceV1, MAX_LINK_INPUTS, MAX_WORKER_OUTPUT_BYTES,
    MAX_WORKER_TOTAL_INPUT_BYTES, NativeFirstBuildWorkerErrorV1,
    NativeWorkerCompactFinalizerReplayV1, NativeWorkerDiagnosticV1,
    NativeWorkerFinalizationErrorV1, PreparedFinalizedNativeWorkerHsacoV1,
    ProtectedCompilerNativeHandoffBindingErrorV1, WorkerInputV1, WorkerOutputConstraintsV1,
    finalize_native_worker_hsaco_v1,
    first_build_worker_native::{
        NativeWorkerReplaySource, recover_prepaid_native_worker_evidence_v1,
    },
    first_build_worker_native_binding::{
        ProtectedCompilerNativeHandoffBindingV1, native_handoff_storage_floor,
    },
    first_build_worker_v3::enforce_worker_working_set_budget,
    native_worker_replay_resources::{
        NativeWorkerReplayResourceQuote, NativeWorkerReplayResponseInputs,
    },
    request_construction::decode_compiler_module_handoff_v2,
    worker_finalizer_replay_engine::{ReconstructedWorkerExchanges, reconstruct_worker_exchanges},
    worker_v3_finalized_schema::DescriptorSchema,
};

const FRAME: usize = 2 * size_of::<NativeWorkerReplaySource>()
    + 2 * size_of::<ProtectedCompilerNativeHandoffBindingV1>()
    + size_of::<ReconstructedWorkerExchanges>()
    + size_of::<InertNativeFirstBuildWorkerEvidenceV1>()
    + size_of::<NativeWorkerReplayResourceQuote>()
    + size_of::<NativeWorkerReplayErrorV1>()
    + 4096;

/// Additional unreserved Worker/finalizer charge. Source and transcript funding
/// remain separately paid; no recovered value represents fresh process custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerReplayStorageV1(usize);
impl NativeWorkerReplayStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum NativeWorkerReplayErrorV1 {
    Resource(Resource),
    Mismatch(&'static str),
    Stage {
        phase: &'static str,
        diagnostic: NativeWorkerDiagnosticV1,
    },
}
impl From<Resource> for NativeWorkerReplayErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for NativeWorkerReplayErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Mismatch(field) => write!(f, "native replay mismatch: {field}"),
            Self::Stage { phase, diagnostic } => write!(f, "native replay {phase}: {diagnostic}"),
        }
    }
}
impl std::error::Error for NativeWorkerReplayErrorV1 {}
type Result<T> = std::result::Result<T, NativeWorkerReplayErrorV1>;
fn failure(phase: &'static str, value: impl fmt::Display) -> NativeWorkerReplayErrorV1 {
    NativeWorkerReplayErrorV1::Stage {
        phase,
        diagnostic: NativeWorkerDiagnosticV1::from_display(value),
    }
}

impl From<ProtectedCompilerNativeHandoffBindingErrorV1> for NativeWorkerReplayErrorV1 {
    fn from(value: ProtectedCompilerNativeHandoffBindingErrorV1) -> Self {
        match value {
            ProtectedCompilerNativeHandoffBindingErrorV1::Resource(e) => Self::Resource(e),
            other => failure("native binding", other),
        }
    }
}

impl From<NativeFirstBuildWorkerErrorV1> for NativeWorkerReplayErrorV1 {
    fn from(value: NativeFirstBuildWorkerErrorV1) -> Self {
        match value {
            NativeFirstBuildWorkerErrorV1::Resource(e) => Self::Resource(e),
            NativeFirstBuildWorkerErrorV1::Binding(e) => e.into(),
            other => failure("Worker validation", other),
        }
    }
}

impl From<NativeWorkerFinalizationErrorV1> for NativeWorkerReplayErrorV1 {
    fn from(value: NativeWorkerFinalizationErrorV1) -> Self {
        match value {
            NativeWorkerFinalizationErrorV1::Resource(e) => Self::Resource(e),
            NativeWorkerFinalizationErrorV1::Source(e) => e.into(),
            other => failure("finalizer", other),
        }
    }
}

/// Retains the independently recovered source/F while reconstructing the exact
/// frozen Worker requests and responses and re-running the shared finalizer.
/// The caller keeps the full source backing/metadata/recovery and transcript
/// reservations paid. Returned storage is additional and unreserved.
///
/// No Worker process is started and no consumption/currentness token is forged.
/// Re-derived receipt coordinates, public signed-source content, deterministic
/// replay and structural finalization do not authenticate protected execution or
/// grant publication/load/launch authority. Artifact parsing and raw-HSACO
/// reconstruction remain in the existing bounded finalizer domain; native
/// source, provider construction and Worker replay use this original ledger.
#[allow(clippy::too_many_arguments)]
pub fn revalidate_native_worker_finalizer_v1(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    source: RecoveredCompilerNativeSemanticHandoffV4,
    transcript: &NativeWorkerCompactFinalizerReplayV1,
    provider_payloads: Vec<Vec<u8>>,
    exact_finalized_hsaco: &[u8],
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedFinalizedNativeWorkerHsacoV1,
    NativeWorkerReplayStorageV1,
)> {
    let source_floor = native_handoff_storage_floor(&source)?;
    let floor = source_floor
        .checked_add(transcript.storage().retained_storage())
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 8, 4096, FRAME, |budget| {
        if transcript.attempt() != attempt {
            return Err(NativeWorkerReplayErrorV1::Mismatch("attempt"));
        }
        transcript
            .verify_outer_identity(source.handoff().identity())
            .map_err(|e| failure("outer identity", e))?;
        let receipt = rederive_compiler_module_handoff_receipt_for_replay_v4(
            producer,
            attempt,
            transcript.handoff_slot(),
            transcript.transaction_identity(),
            source.handoff(),
            budget,
        )
        .map_err(|e| match e {
            CompilerModuleHandoffErrorV4::Resource(e) => NativeWorkerReplayErrorV1::Resource(e),
            other => failure("occurrence", other),
        })?;
        let binding = ProtectedCompilerNativeHandoffBindingV1::from_handoff(
            &source,
            receipt,
            *source.handoff().capsule().base().compiler_closure(),
            budget,
        )?;
        if binding.identity().as_bytes() != transcript.binding_identity() {
            return Err(NativeWorkerReplayErrorV1::Mismatch("Worker binding"));
        }
        let replay = transcript.replay_view();
        let providers = prepare_providers(&replay, provider_payloads, budget)?;
        enforce_worker_working_set_budget(
            source.handoff().canonical_bytes().len(),
            source.handoff().module_handoff(),
            &providers,
            replay.link_options,
        )
        .map_err(|e| failure("working set", e))?;
        if exact_finalized_hsaco.is_empty() || exact_finalized_hsaco.len() > MAX_WORKER_OUTPUT_BYTES
        {
            return Err(NativeWorkerReplayErrorV1::Mismatch("artifact byte bound"));
        }
        let abi = source
            .handoff()
            .capsule()
            .base()
            .receipts()
            .abi()
            .canonical_preimage();
        let raw_hsaco = DescriptorSchema::from_abi(abi)
            .and_then(|schema| schema.derive_raw(exact_finalized_hsaco))
            .map_err(|e| failure("raw artifact reconstruction", e))?;
        let output = WorkerOutputConstraintsV1::new(replay.bootstrap_output_bound)
            .map_err(|e| failure("output bound", e))?;
        let quote = NativeWorkerReplayResourceQuote::new(
            source.handoff().module_handoff(),
            &providers,
            replay.link_options,
            &output,
            replay.execution_limits,
            NativeWorkerReplayResponseInputs {
                raw_output_bytes: raw_hsaco.len(),
                worker_build_identity_bytes: replay.worker.worker_build_identity().len(),
                bootstrap_metadata: replay.bootstrap_metadata,
                replay_metadata: replay.replay_metadata,
            },
        )
        .map_err(|e| failure("resource quote", e))?;
        let entry = budget.storage();
        let (worker_source, worker_storage) = budget.with_prepaid_scope(
            entry,
            0,
            quote.reconstruction_work,
            quote.reconstruction_storage,
            |_| {
                let decoded = decode_compiler_module_handoff_v2(
                    source.handoff().module_handoff().canonical_bytes(),
                )
                .map_err(|e| failure("module decode", e))?;
                let exchanges = reconstruct_worker_exchanges(
                    (&binding).into(),
                    &decoded,
                    providers,
                    &replay,
                    &raw_hsaco,
                )
                .map_err(|e| failure("Worker reconstruction", e))?;
                recover_prepaid_native_worker_evidence_v1(
                    NativeWorkerReplaySource {
                        source,
                        binding,
                        worker: replay.worker.clone(),
                        limits: replay.execution_limits,
                    },
                    &decoded,
                    exchanges,
                    &quote.first_build,
                )
                .map_err(NativeWorkerReplayErrorV1::from)
            },
        )?;
        budget.reserve_storage(worker_storage.retained_storage())?;
        if worker_source.identity().as_bytes() != transcript.source_evidence_identity() {
            return Err(NativeWorkerReplayErrorV1::Mismatch("source evidence"));
        }
        let (finalized, final_storage) = finalize_native_worker_hsaco_v1(worker_source, budget)?;
        budget.reserve_storage(final_storage.retained_storage())?;
        if finalized.identity().as_bytes() != transcript.expected_finalization_identity() {
            return Err(NativeWorkerReplayErrorV1::Mismatch("finalization identity"));
        }
        if finalized.exact_finalized_bytes() != exact_finalized_hsaco {
            return Err(NativeWorkerReplayErrorV1::Mismatch(
                "finalized artifact bytes",
            ));
        }
        let storage = NativeWorkerReplayStorageV1(
            worker_storage
                .retained_storage()
                .checked_add(final_storage.retained_storage())
                .ok_or(Resource::Arithmetic)?,
        );
        Ok((finalized, storage))
    })
}

fn prepare_providers(
    replay: &crate::worker_v3_compact_finalizer_replay::ProtectedWorkerV3CompactFinalizerReplayViewV2<'_>,
    payloads: Vec<Vec<u8>>,
    budget: &mut Budget<'_>,
) -> Result<Vec<WorkerInputV1>> {
    let count = payloads.len();
    if count >= MAX_LINK_INPUTS || count != replay.external_providers.len() {
        return Err(NativeWorkerReplayErrorV1::Mismatch("provider count"));
    }
    budget.charge_work(MAX_LINK_INPUTS * 16)?;
    let total = payloads
        .iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes.len()))
        .ok_or(Resource::Arithmetic)?;
    if total > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(NativeWorkerReplayErrorV1::Mismatch("provider byte bound"));
    }
    let work = total
        .checked_mul(3)
        .and_then(|v| v.checked_add(count * 256))
        .ok_or(Resource::Arithmetic)?;
    let storage = count
        .checked_mul(size_of::<WorkerInputV1>() + size_of::<Vec<u8>>())
        .and_then(|v| v.checked_add(total))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(storage)?;
    budget.charge_work(work)?;
    let mut providers = Vec::new();
    providers
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    for (reference, payload) in replay.external_providers.iter().zip(payloads) {
        if reference.identity.byte_len() != payload.len() as u64 {
            return Err(NativeWorkerReplayErrorV1::Mismatch("provider length"));
        }
        let input =
            WorkerInputV1::new(reference.kind, payload).map_err(|e| failure("provider", e))?;
        if input.identity() != reference.identity {
            return Err(NativeWorkerReplayErrorV1::Mismatch("provider identity"));
        }
        providers.push(input);
    }
    // This temporary reservation is retired by the enclosing original-ledger
    // scope after provider/request overlap has ended, including on refusal.
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_resource_refusals_remain_typed() {
        let binding =
            || ProtectedCompilerNativeHandoffBindingErrorV1::Resource(Resource::Arithmetic);
        let worker = || NativeFirstBuildWorkerErrorV1::Binding(binding());
        let cases: [NativeWorkerReplayErrorV1; 5] = [
            binding().into(),
            worker().into(),
            NativeFirstBuildWorkerErrorV1::Resource(Resource::Arithmetic).into(),
            NativeWorkerFinalizationErrorV1::Source(worker()).into(),
            NativeWorkerFinalizationErrorV1::Resource(Resource::Arithmetic).into(),
        ];
        for error in cases {
            assert!(matches!(
                error,
                NativeWorkerReplayErrorV1::Resource(Resource::Arithmetic)
            ));
        }
    }
}
