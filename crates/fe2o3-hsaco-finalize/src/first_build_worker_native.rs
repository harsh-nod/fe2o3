//! Native source/F custody through the existing reproducible Worker engine.

use std::{fmt, fmt::Write, mem::size_of};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffConsumptionTokenV4, CompilerModuleHandoffReceiptV4,
    ConsumedCompilerModuleHandoffV4,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4;

use crate::{
    ContentIdentityV1, LinkOptionV1, MultiInputLinkPlanV1, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerResponseV2,
    first_build_worker_engine::{
        ReproducibleFirstBuildEngineError as EngineError, ReproducibleFirstBuildEnginePreflight,
        execute_preflighted_reproducible_first_build_engine,
        preflight_reproducible_first_build_engine,
    },
    first_build_worker_native_binding::{
        ProtectedCompilerNativeHandoffBindingErrorV1, ProtectedCompilerNativeHandoffBindingV1,
        native_handoff_storage_floor,
    },
    first_build_worker_native_resources::NativeWorkerResourceQuote,
    first_build_worker_v3::{
        calculate_worker_evidence_identity_parts, enforce_worker_working_set_budget,
        validate_replay_parts,
    },
    request_construction::decode_compiler_module_handoff_v2,
    worker_executor::InertWorkerExecutionV2,
};

// Binding constructors return unreserved headers. Keep their local copies,
// quote and fixed error/comparison state paid until the engine scope returns.
const ENTRY_STORAGE: usize = 2 * size_of::<ProtectedCompilerNativeHandoffBindingV1>()
    + size_of::<NativeWorkerResourceQuote>()
    + 2 * size_of::<CompilerModuleHandoffReceiptV4>()
    + size_of::<NativeFirstBuildWorkerErrorV1>();
const ENTRY_WORK: usize = 2 * ENTRY_STORAGE + 4 * crate::MAX_WORKER_TOOLCHAIN_ID_BYTES + 256;

/// Additional conservative retained charge. Reserve it before retaining the
/// returned owner; release it only after dropping or transferring that owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFirstBuildWorkerStorageV1(usize);

impl NativeFirstBuildWorkerStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Checked native inputs for the shared Worker transaction, not launch authority.
/// The recovered handoff's reservation remains separately paid by its owner.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedNativeFirstBuildWorkerV1 as Preflight;
/// fn duplicate(value: Preflight) { let _ = value.clone(); }
/// ```
pub struct PreparedNativeFirstBuildWorkerV1 {
    binding: ProtectedCompilerNativeHandoffBindingV1,
    worker: WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    quote: NativeWorkerResourceQuote,
    engine: ReproducibleFirstBuildEnginePreflight,
    storage: NativeFirstBuildWorkerStorageV1,
}

impl PreparedNativeFirstBuildWorkerV1 {
    pub const fn binding(&self) -> ProtectedCompilerNativeHandoffBindingV1 {
        self.binding
    }

    pub const fn storage(&self) -> NativeFirstBuildWorkerStorageV1 {
        self.storage
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }
}

/// Identity of the native binding and complete candidate/replay transcripts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFirstBuildWorkerIdentityV1([u8; 32]);

impl NativeFirstBuildWorkerIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Custody origin, independent of the deterministic transcript identity. Neither
/// variant authenticates protected compiler or Worker execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWorkerEvidenceCustodyV1 {
    ConsumedPublication,
    RecoveredTranscript,
}

enum NativeWorkerSource {
    Consumed(ConsumedCompilerModuleHandoffV4<RecoveredCompilerNativeSemanticHandoffV4>),
    Replayed {
        source: RecoveredCompilerNativeSemanticHandoffV4,
        receipt: CompilerModuleHandoffReceiptV4,
    },
}

impl NativeWorkerSource {
    const fn content(&self) -> &RecoveredCompilerNativeSemanticHandoffV4 {
        match self {
            Self::Consumed(value) => value.content(),
            Self::Replayed { source, .. } => source,
        }
    }
}

/// Retains the consumed or independently replayed V4 occurrence, signed-source
/// content and actual final F
/// together with reproducible Worker output. No carrier is replaced by a digest
/// or an embedded V3 owner. This is structural evidence, not protected origin,
/// currentness, semantic-to-machine refinement, publication or GPU authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::InertNativeFirstBuildWorkerEvidenceV1 as Evidence;
/// fn forge() -> Evidence { Evidence::default() }
/// ```
pub struct InertNativeFirstBuildWorkerEvidenceV1 {
    source: NativeWorkerSource,
    binding: ProtectedCompilerNativeHandoffBindingV1,
    identity: NativeFirstBuildWorkerIdentityV1,
    worker: WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    plan: MultiInputLinkPlanV1,
    bootstrap_request_bytes: Vec<u8>,
    bootstrap: InertWorkerExecutionV2,
    replay_request_bytes: Vec<u8>,
    replay: InertWorkerExecutionV2,
    storage: NativeFirstBuildWorkerStorageV1,
    retained_storage: usize,
}

impl InertNativeFirstBuildWorkerEvidenceV1 {
    pub const fn custody(&self) -> NativeWorkerEvidenceCustodyV1 {
        match &self.source {
            NativeWorkerSource::Consumed(_) => NativeWorkerEvidenceCustodyV1::ConsumedPublication,
            NativeWorkerSource::Replayed { .. } => {
                NativeWorkerEvidenceCustodyV1::RecoveredTranscript
            }
        }
    }
    pub const fn recovered_handoff(&self) -> &RecoveredCompilerNativeSemanticHandoffV4 {
        self.source.content()
    }

    pub const fn binding(&self) -> ProtectedCompilerNativeHandoffBindingV1 {
        self.binding
    }

    pub const fn identity(&self) -> NativeFirstBuildWorkerIdentityV1 {
        self.identity
    }

    pub const fn storage(&self) -> NativeFirstBuildWorkerStorageV1 {
        self.storage
    }

    /// Complete source, preflight and Worker reservation that must stay paid
    /// while this owner or a downstream retaining owner is live.
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }

    pub(crate) fn revalidate_for_artifact(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), NativeFirstBuildWorkerErrorV1> {
        budget.with_prepaid_scope(
            self.retained_storage,
            0,
            ENTRY_WORK,
            ENTRY_STORAGE,
            |budget| {
                let actual = match &self.source {
                    NativeWorkerSource::Consumed(source) => {
                        ProtectedCompilerNativeHandoffBindingV1::from_consumed(
                            source,
                            self.binding.receipt(),
                            self.binding.compiler_closure(),
                            budget,
                        )?
                    }
                    NativeWorkerSource::Replayed { source, receipt } => {
                        if *receipt != self.binding.receipt() {
                            return Err(NativeFirstBuildWorkerErrorV1::PreflightMismatch(
                                "replayed native receipt",
                            ));
                        }
                        ProtectedCompilerNativeHandoffBindingV1::from_handoff(
                            source,
                            *receipt,
                            self.binding.compiler_closure(),
                            budget,
                        )?
                    }
                };
                if actual != self.binding {
                    return Err(NativeFirstBuildWorkerErrorV1::PreflightMismatch(
                        "native artifact source",
                    ));
                }
                Ok(())
            },
        )
    }

    pub(crate) fn artifact_lineage(
        &self,
    ) -> crate::worker_hsaco_lineage::WorkerArtifactLineage<'_> {
        use crate::worker_hsaco_lineage::{WorkerArtifactExchange, WorkerArtifactLineage};
        WorkerArtifactLineage {
            module: self.recovered_handoff().handoff().module_handoff(),
            plan: &self.plan,
            measurement: &self.worker,
            exchanges: [
                WorkerArtifactExchange {
                    request: &self.bootstrap_request_bytes,
                    response: self.bootstrap.response(),
                    executable: self.bootstrap.worker_executable(),
                },
                WorkerArtifactExchange {
                    request: &self.replay_request_bytes,
                    response: self.replay.response(),
                    executable: self.replay.worker_executable(),
                },
            ],
            output: self.output_bytes(),
        }
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.limits
    }

    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        &self.plan
    }

    pub fn bootstrap_request_bytes(&self) -> &[u8] {
        &self.bootstrap_request_bytes
    }

    pub fn exact_replay_request_bytes(&self) -> &[u8] {
        &self.replay_request_bytes
    }

    pub const fn bootstrap_response(&self) -> &WorkerResponseV2 {
        self.bootstrap.response()
    }

    pub const fn exact_replay_response(&self) -> &WorkerResponseV2 {
        self.replay.response()
    }

    pub fn output_bytes(&self) -> &[u8] {
        // Shared transcript validation requires this output;
        // neither response nor this evidence has a public mutable constructor.
        self.replay
            .response()
            .output()
            .expect("validated native replay output")
            .bytes()
    }

    pub const fn output_identity(&self) -> ContentIdentityV1 {
        self.plan.output().identity()
    }
}

/// Performs all input-dependent checks before one-shot transaction consumption.
/// The caller retains the locked recovered token and its paid storage. Returned
/// storage is additional and unreserved. Refusal cannot tombstone the token.
///
/// Worker process/LLVM limits remain the existing engine's separate accounting
/// domain; this budget covers the declared native Rust staging and replay work.
/// Provider/options accounting uses logical lengths and internal growth, not
/// arbitrary caller spare capacity or allocator overhead; it is not an RSS cap.
#[allow(clippy::too_many_arguments)]
pub fn preflight_native_reproducible_first_build_worker_v1(
    token: &CompilerModuleHandoffConsumptionTokenV4<RecoveredCompilerNativeSemanticHandoffV4>,
    receipt: CompilerModuleHandoffReceiptV4,
    compiler_closure: CompilerClosureV2,
    worker: &PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    options: Vec<LinkOptionV1>,
    output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedNativeFirstBuildWorkerV1,
        NativeFirstBuildWorkerStorageV1,
    ),
    NativeFirstBuildWorkerErrorV1,
> {
    budget.with_prepaid_scope(
        token.storage().retained_storage(),
        16,
        ENTRY_WORK,
        ENTRY_STORAGE,
        |budget| {
            if token.receipt() != receipt {
                return Err(NativeFirstBuildWorkerErrorV1::PreflightMismatch(
                    "locked V4 receipt",
                ));
            }
            token
                .revalidate_locked_currentness(budget)
                .map_err(currentness_error)?;
            let source = token.content();
            let binding = ProtectedCompilerNativeHandoffBindingV1::from_handoff(
                source,
                receipt,
                compiler_closure,
                budget,
            )?;
            let handoff = source.handoff();
            if providers.len() >= crate::MAX_LINK_INPUTS || options.len() > crate::MAX_LINK_OPTIONS
            {
                return Err(failure(
                    "working set",
                    "provider or option count exceeds the shared bound",
                ));
            }
            // Two bounded length censuses plus the shared aggregate guard, before any
            // sorting, variable-byte hashing, decoding or Worker request construction.
            budget.charge_work(2048)?;
            enforce_worker_working_set_budget(
                handoff.canonical_bytes().len(),
                handoff.module_handoff(),
                &providers,
                &options,
            )
            .map_err(|e| failure("working set", e))?;
            let quote = NativeWorkerResourceQuote::new(
                handoff.module_handoff(),
                &providers,
                &options,
                &output,
                limits,
            )
            .map_err(|e| failure("resource quote", format_args!("{e:?}")))?;
            let storage = NativeFirstBuildWorkerStorageV1(
                quote
                    .preflight_storage
                    .checked_add(size_of::<PreparedNativeFirstBuildWorkerV1>())
                    .ok_or(Resource::Arithmetic)?,
            );
            budget.with_prepaid_scope(
                native_handoff_storage_floor(source)?,
                0,
                quote.preflight_work,
                storage.0,
                |budget| {
                    let decoded = decode_compiler_module_handoff_v2(
                        handoff.module_handoff().canonical_bytes(),
                    )
                    .map_err(|e| failure("module decode", e))?;
                    let engine = preflight_reproducible_first_build_engine(
                        (&binding).into(),
                        decoded,
                        worker,
                        providers,
                        options,
                        output,
                    )
                    .map_err(engine_error)?;
                    token
                        .revalidate_locked_currentness(budget)
                        .map_err(currentness_error)?;
                    Ok((
                        PreparedNativeFirstBuildWorkerV1 {
                            binding,
                            worker: worker.measurement().clone(),
                            limits,
                            quote,
                            engine,
                            storage,
                        },
                        storage,
                    ))
                },
            )
        },
    )
}

/// Consumes the native owner and its exact preflight through the same Worker
/// engine. Receipt, carrier/F binding and measured worker are rechecked before
/// any process starts. The output retains the original source owner unchanged.
///
/// The token and preflight charges remain paid on success or refusal. On success
/// the returned charge is additional; retain all three until the evidence drops.
pub fn execute_preflighted_native_reproducible_first_build_worker_v1(
    consumed: ConsumedCompilerModuleHandoffV4<RecoveredCompilerNativeSemanticHandoffV4>,
    preflight: PreparedNativeFirstBuildWorkerV1,
    worker: &PinnedWorkerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        InertNativeFirstBuildWorkerEvidenceV1,
        NativeFirstBuildWorkerStorageV1,
    ),
    NativeFirstBuildWorkerErrorV1,
> {
    let floor = consumed
        .storage()
        .retained_storage()
        .checked_add(preflight.storage.0)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 8, ENTRY_WORK, ENTRY_STORAGE, |budget| {
        let fresh = ProtectedCompilerNativeHandoffBindingV1::from_consumed(
            &consumed,
            preflight.binding.receipt(),
            preflight.binding.compiler_closure(),
            budget,
        )?;
        if fresh != preflight.binding {
            return Err(NativeFirstBuildWorkerErrorV1::PreflightMismatch(
                "native source/F binding",
            ));
        }
        if worker.measurement() != &preflight.worker {
            return Err(NativeFirstBuildWorkerErrorV1::PreflightMismatch(
                "measured worker",
            ));
        }
        let PreparedNativeFirstBuildWorkerV1 {
            binding,
            worker: measurement,
            limits,
            quote,
            engine,
            ..
        } = preflight;
        let storage = NativeFirstBuildWorkerStorageV1(
            quote
                .returned_retained_storage()
                .checked_add(size_of::<InertNativeFirstBuildWorkerEvidenceV1>())
                .ok_or(Resource::Arithmetic)?,
        );
        let scratch = quote
            .execution_storage
            .checked_add(size_of::<InertNativeFirstBuildWorkerEvidenceV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, quote.execution_work, scratch, |_| {
            let result = execute_preflighted_reproducible_first_build_engine(
                (&binding).into(),
                engine,
                worker,
                limits,
            )
            .map_err(engine_error)?;
            validate_replay_parts(
                (&binding).into(),
                &measurement,
                &result.decoded,
                &result.plan,
                &result.candidate_request_bytes,
                result.candidate.response(),
                &result.authorized_request_bytes,
                result.authorized.response(),
            )
            .map_err(|e| failure("transcript replay", e))?;
            let identity = calculate_worker_evidence_identity_parts(
                (&binding).into(),
                &measurement,
                limits,
                &result.plan,
                &result.candidate_request_bytes,
                result.candidate.response().canonical_bytes(),
                &result.authorized_request_bytes,
                result.authorized.response().canonical_bytes(),
            )
            .map_err(|e| failure("evidence identity", e))?;
            Ok((
                InertNativeFirstBuildWorkerEvidenceV1 {
                    source: NativeWorkerSource::Consumed(consumed),
                    binding,
                    identity: NativeFirstBuildWorkerIdentityV1(identity),
                    worker: measurement,
                    limits,
                    plan: result.plan,
                    bootstrap_request_bytes: result.candidate_request_bytes,
                    bootstrap: result.candidate,
                    replay_request_bytes: result.authorized_request_bytes,
                    replay: result.authorized,
                    storage,
                    retained_storage: floor.checked_add(storage.0).ok_or(Resource::Arithmetic)?,
                },
                storage,
            ))
        })
    })
}

pub(crate) struct NativeWorkerReplaySource {
    pub(crate) source: RecoveredCompilerNativeSemanticHandoffV4,
    pub(crate) binding: ProtectedCompilerNativeHandoffBindingV1,
    pub(crate) worker: WorkerMeasurementV1,
    pub(crate) limits: WorkerExecutionLimitsV1,
}

/// Called only inside the replay adapter's prepaid common-engine schedule.
/// Recovered transcript evidence never manufactures a consumed transaction.
pub(crate) fn recover_prepaid_native_worker_evidence_v1(
    input: NativeWorkerReplaySource,
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    exchanges: crate::worker_finalizer_replay_engine::ReconstructedWorkerExchanges,
    quote: &NativeWorkerResourceQuote,
) -> Result<
    (
        InertNativeFirstBuildWorkerEvidenceV1,
        NativeFirstBuildWorkerStorageV1,
    ),
    NativeFirstBuildWorkerErrorV1,
> {
    let NativeWorkerReplaySource {
        source,
        binding,
        worker,
        limits,
    } = input;
    let crate::worker_finalizer_replay_engine::ReconstructedWorkerExchanges {
        plan,
        bootstrap_request_bytes,
        bootstrap_response,
        replay_request_bytes,
        replay_response,
    } = exchanges;
    validate_replay_parts(
        (&binding).into(),
        &worker,
        decoded,
        &plan,
        &bootstrap_request_bytes,
        &bootstrap_response,
        &replay_request_bytes,
        &replay_response,
    )
    .map_err(|e| failure("recovered transcript replay", e))?;
    let identity = calculate_worker_evidence_identity_parts(
        (&binding).into(),
        &worker,
        limits,
        &plan,
        &bootstrap_request_bytes,
        bootstrap_response.canonical_bytes(),
        &replay_request_bytes,
        replay_response.canonical_bytes(),
    )
    .map_err(|e| failure("recovered evidence identity", e))?;
    let storage = NativeFirstBuildWorkerStorageV1(
        quote
            .returned_retained_storage()
            .checked_add(size_of::<InertNativeFirstBuildWorkerEvidenceV1>())
            .ok_or(Resource::Arithmetic)?,
    );
    let retained_storage = native_handoff_storage_floor(&source)?
        .checked_add(storage.0)
        .ok_or(Resource::Arithmetic)?;
    let executable = worker.executable();
    Ok((
        InertNativeFirstBuildWorkerEvidenceV1 {
            source: NativeWorkerSource::Replayed {
                source,
                receipt: binding.receipt(),
            },
            binding,
            identity: NativeFirstBuildWorkerIdentityV1(identity),
            worker,
            limits,
            plan,
            bootstrap_request_bytes,
            bootstrap: InertWorkerExecutionV2::from_recovered_response(
                executable,
                bootstrap_response,
            ),
            replay_request_bytes,
            replay: InertWorkerExecutionV2::from_recovered_response(executable, replay_response),
            storage,
            retained_storage,
        },
        storage,
    ))
}

/// Bounded diagnostic text; failed process transcripts are dropped before their
/// temporary resource reservation is released rather than escaping in an error.
#[derive(Clone, Eq, PartialEq)]
pub struct NativeWorkerDiagnosticV1 {
    bytes: [u8; 80],
    len: usize,
}

impl NativeWorkerDiagnosticV1 {
    pub(crate) fn from_display(value: impl fmt::Display) -> Self {
        let mut diagnostic = Self {
            bytes: [0; 80],
            len: 0,
        };
        let _ = write!(&mut diagnostic, "{value}");
        diagnostic
    }
}

impl fmt::Write for NativeWorkerDiagnosticV1 {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let mut take = text.len().min(self.bytes.len() - self.len);
        while !text.is_char_boundary(take) {
            take -= 1;
        }
        self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]);
        self.len += take;
        if take == text.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

impl fmt::Display for NativeWorkerDiagnosticV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| fmt::Error)?)
    }
}

impl fmt::Debug for NativeWorkerDiagnosticV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug)]
pub enum NativeFirstBuildWorkerErrorV1 {
    Resource(Resource),
    Binding(ProtectedCompilerNativeHandoffBindingErrorV1),
    PreflightMismatch(&'static str),
    Worker {
        phase: &'static str,
        diagnostic: NativeWorkerDiagnosticV1,
    },
}

impl From<Resource> for NativeFirstBuildWorkerErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl From<ProtectedCompilerNativeHandoffBindingErrorV1> for NativeFirstBuildWorkerErrorV1 {
    fn from(error: ProtectedCompilerNativeHandoffBindingErrorV1) -> Self {
        Self::Binding(error)
    }
}

impl fmt::Display for NativeFirstBuildWorkerErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Binding(e) => e.fmt(f),
            Self::PreflightMismatch(field) => {
                write!(f, "native Worker preflight mismatch: {field}")
            }
            Self::Worker { phase, diagnostic } => write!(f, "native Worker {phase}: {diagnostic}"),
        }
    }
}

impl std::error::Error for NativeFirstBuildWorkerErrorV1 {}

fn failure(phase: &'static str, error: impl fmt::Display) -> NativeFirstBuildWorkerErrorV1 {
    NativeFirstBuildWorkerErrorV1::Worker {
        phase,
        diagnostic: NativeWorkerDiagnosticV1::from_display(error),
    }
}

fn currentness_error(
    error: fe2o3_artifact_transaction::CompilerModuleHandoffErrorV4,
) -> NativeFirstBuildWorkerErrorV1 {
    match error {
        fe2o3_artifact_transaction::CompilerModuleHandoffErrorV4::Resource(error) => error.into(),
        other => failure("currentness", other),
    }
}

fn engine_error(error: EngineError) -> NativeFirstBuildWorkerErrorV1 {
    match error {
        EngineError::LinkPlan(e) => failure("link plan", e),
        EngineError::RequestConstruction(e) => failure("request construction", e),
        EngineError::CandidateRequest(e) => failure("candidate request", e),
        EngineError::CandidateExecution(e) => failure("candidate execution", e),
        EngineError::AuthorizedExecution(e) => failure("replay execution", e),
        EngineError::CandidateDidNotProduceOutput(e) => failure(
            "candidate output",
            format_args!("missing at {:?}", e.response().stage()),
        ),
        EngineError::AuthorizedDidNotProduceOutput { authorized, .. } => failure(
            "replay output",
            format_args!("missing at {:?}", authorized.response().stage()),
        ),
        EngineError::OutputMismatch { .. } => {
            failure("reproducibility", "candidate/replay output differs")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_are_fixed_size_and_truncate_only_at_utf8_boundaries() {
        let mut diagnostic = NativeWorkerDiagnosticV1 {
            bytes: [0; 80],
            len: 0,
        };
        diagnostic.write_str(&"x".repeat(79)).unwrap();
        assert!(diagnostic.write_str("\u{e9}").is_err());
        assert_eq!(diagnostic.to_string(), "x".repeat(79));
        diagnostic.write_str("y").unwrap();
        assert!(diagnostic.write_str("z").is_err());
        assert_eq!(diagnostic.len, diagnostic.bytes.len());
        assert!(size_of::<NativeFirstBuildWorkerErrorV1>() <= 128);
    }

    #[test]
    fn failure_formatting_does_not_retain_unbounded_worker_diagnostics() {
        let error = failure("test", "z".repeat(4096));
        let NativeFirstBuildWorkerErrorV1::Worker { phase, diagnostic } = error else {
            panic!("wrong failure class");
        };
        assert_eq!(phase, "test");
        assert_eq!(diagnostic.to_string(), "z".repeat(80));
    }
}
