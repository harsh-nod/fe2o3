//! Closed in-process orchestration for the production symbolic general-GEMM route.
//!
//! Cargo establishes the managed attempt and supplies a canonical measured-worker
//! manifest. rustc independently re-pins that manifest and executes lowering,
//! Worker V2, and post-link inspection before its private frontend correspondence
//! can leave the process. This module deliberately exposes no publication or load
//! boundary and cannot construct frontend or verifier authority.

#![allow(
    dead_code,
    reason = "the inert orchestration shell is consumed only after the private frontend join lands"
)]

use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildSession, CompilerModuleHandoffIdentityV1, CompilerModuleHandoffReceiptV1,
    CompilerModuleHandoffSlotV1, ProducerIdentity, consume_compiler_module_handoff_in_slot_v1,
    publish_compiler_module_handoff_in_slot_v1,
};
use fe2o3_general_gemm_compiler::{
    GeneralGemmScheduleV1, GeneralGemmSymbolicArtifactIdentityV1,
    GeneralGemmSymbolicCompilationUnitV1, lower_general_gemm_symbolic_structural_machine_v1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, OpaqueGeneralGemmPostLinkMachineObservationV1, PinnedWorkerV1,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, execute_symbolic_general_gemm_worker_v2_v1,
    finalize_symbolic_general_gemm_worker_v2_v1,
};
use fe2o3_verifier::{
    GeneralGemmNumericalComparisonPolicyV1, GeneralGemmNumericalPolicyRequestV1,
    GeneralGemmPropertyClosureEvaluationV1, evaluate_general_gemm_property_closure_v1,
    execute_general_gemm_numerical_policy_v1, execute_general_gemm_schedule_proof_v1,
    join_general_gemm_proof_and_numerical_evidence_v1,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(crate) const GENERAL_GEMM_PIPELINE_V1: &str = "collected-general-gemm-v1";
const CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
const WORKER_CONFIG_ENV: &str = "FE2O3_WORKER_V2_CONFIG_V2";
const EXPECTED_CONFIG_ID_ENV: &str = "FE2O3_WORKER_V2_EXPECTED_ID_V1";
const CONFIG_FORMAT: &str = "fe2o3-worker-v2-config-v2";
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_CONFIG_PATH_BYTES: usize = 4096;
const ROOT_KEYS: &[&str] = &[
    "candidate_output_max_bytes",
    "format",
    "general_gemm_v1",
    "limits",
    "link_options",
    "providers",
    "units",
    "worker",
];
const WORKER_KEYS: &[&str] = &[
    "byte_len",
    "llvm_build_identity",
    "path",
    "sha256",
    "worker_build_identity",
];
const LIMIT_KEYS: &[&str] = &["stderr_bytes", "stdout_bytes", "timeout_ms"];
const UNIT_KEYS: &[&str] = &["crate_name", "source", "working_directory"];
const OPTION_KEYS: &[&str] = &["name", "value"];
const GENERAL_GEMM_KEYS: &[&str] = &["profile", "proof_timeout_seconds", "verus_path"];
const GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1: &str = "qualification-pair-v1";
const GENERAL_GEMM_QUALIFICATION_SCHEDULES_V1: [GeneralGemmScheduleV1; 2] = [
    GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
];
const FIXED_OPTIONS: &[(&str, &str)] = &[
    ("code-object-version", "6"),
    ("opt-level", "2"),
    ("strip-debug", "true"),
    ("verify-each", "true"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmPipelineConfigIdentityV1([u8; 32]);

impl GeneralGemmPipelineConfigIdentityV1 {
    pub(crate) const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Independently pinned, closed configuration retained by synchronous rustc execution.
pub(crate) struct PreparedGeneralGemmPipelineV1 {
    identity: GeneralGemmPipelineConfigIdentityV1,
    verus_path: PathBuf,
    proof_timeout_seconds: u32,
    worker: PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
}

impl PreparedGeneralGemmPipelineV1 {
    pub(crate) fn from_environment(
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> Result<Self, GeneralGemmPipelineErrorV1> {
        if env::var_os(CODEGEN_PIPELINE_ENV).as_deref() != Some(GENERAL_GEMM_PIPELINE_V1.as_ref()) {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "the in-process general-GEMM parser requires the exact pipeline selector"
                    .to_owned(),
            ));
        }
        let path = env::var_os(WORKER_CONFIG_ENV).ok_or_else(|| {
            GeneralGemmPipelineErrorV1::Configuration(format!(
                "{WORKER_CONFIG_ENV} is required for in-process general GEMM"
            ))
        })?;
        let path = PathBuf::from(path);
        let expected = env::var(EXPECTED_CONFIG_ID_ENV).map_err(|_| {
            GeneralGemmPipelineErrorV1::Configuration(format!(
                "{EXPECTED_CONFIG_ID_ENV} is required for in-process general GEMM"
            ))
        })?;
        Self::from_manifest(&path, &expected, crate_name, source, working_directory)
    }

    fn from_manifest(
        path: &Path,
        expected_identity: &str,
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> Result<Self, GeneralGemmPipelineErrorV1> {
        require_absolute_path(path, "configuration")?;
        let bytes = read_bounded(path, MAX_CONFIG_BYTES, "configuration")?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            GeneralGemmPipelineErrorV1::Configuration(format!("invalid JSON: {error}"))
        })?;
        if serde_json::to_vec(&value).map_err(|error| {
            GeneralGemmPipelineErrorV1::Configuration(format!("cannot canonicalize JSON: {error}"))
        })? != bytes
        {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "configuration is not compact canonical JSON".to_owned(),
            ));
        }
        let root = exact_object(&value, ROOT_KEYS, "configuration")?;
        if required_string(root, "format", "configuration")? != CONFIG_FORMAT {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "configuration format differs".to_owned(),
            ));
        }
        if required_u64(root, "candidate_output_max_bytes", "configuration")?
            != fe2o3_hsaco::MAX_HSACO_BYTES as u64
        {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "candidate output bound differs from the closed general-GEMM profile".to_owned(),
            ));
        }
        let providers = required_value(root, "providers", "configuration")?
            .as_array()
            .ok_or_else(|| {
                GeneralGemmPipelineErrorV1::Configuration("providers must be an array".to_owned())
            })?;
        if !providers.is_empty() {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "general GEMM rejects request-side providers".to_owned(),
            ));
        }
        parse_fixed_options(required_value(root, "link_options", "configuration")?)?;
        require_selected_unit(
            required_value(root, "units", "configuration")?,
            crate_name,
            source,
            working_directory,
        )?;
        let schedule_object = exact_object(
            required_value(root, "general_gemm_v1", "configuration")?,
            GENERAL_GEMM_KEYS,
            "general_gemm_v1",
        )?;
        let profile = required_string(schedule_object, "profile", "general_gemm_v1")?;
        if profile != GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1 {
            return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
                "unsupported general_gemm_v1.profile {profile:?}"
            )));
        }
        let proof_timeout_seconds =
            required_u64(schedule_object, "proof_timeout_seconds", "general_gemm_v1")?;
        if proof_timeout_seconds == 0
            || proof_timeout_seconds
                > u64::from(fe2o3_verifier::MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1)
        {
            return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
                "general_gemm_v1.proof_timeout_seconds must be in 1..={}",
                fe2o3_verifier::MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1
            )));
        }
        let verus_path = PathBuf::from(required_string(
            schedule_object,
            "verus_path",
            "general_gemm_v1",
        )?);
        require_absolute_path(&verus_path, "general_gemm_v1.verus_path")?;
        let limits = parse_limits(required_value(root, "limits", "configuration")?)?;
        let worker = parse_worker(required_value(root, "worker", "configuration")?)?;
        let identity = calculate_config_identity(&bytes, &worker);
        if decode_sha256(expected_identity, EXPECTED_CONFIG_ID_ENV)? != identity.0 {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "managed expected configuration identity differs from the independently pinned manifest"
                    .to_owned(),
            ));
        }
        Ok(Self {
            identity,
            verus_path,
            proof_timeout_seconds: proof_timeout_seconds as u32,
            worker,
            limits,
        })
    }

    pub(crate) const fn identity(&self) -> GeneralGemmPipelineConfigIdentityV1 {
        self.identity
    }

    pub(crate) fn verus_path(&self) -> &Path {
        &self.verus_path
    }

    pub(crate) const fn proof_timeout_seconds(&self) -> u32 {
        self.proof_timeout_seconds
    }
}

/// One schedule-local executable proof/numerical evaluation retained for the final owner join.
pub(crate) struct InertGeneralGemmVerifierClosureV1 {
    schedule: GeneralGemmScheduleV1,
    closure: GeneralGemmPropertyClosureEvaluationV1,
}

impl InertGeneralGemmVerifierClosureV1 {
    pub(crate) const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    pub(crate) const fn closure(&self) -> &GeneralGemmPropertyClosureEvaluationV1 {
        &self.closure
    }

    pub(crate) fn into_closure(self) -> GeneralGemmPropertyClosureEvaluationV1 {
        self.closure
    }
}

fn execute_general_gemm_verifier_closure_v1(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
    configuration: &PreparedGeneralGemmPipelineV1,
) -> Result<InertGeneralGemmVerifierClosureV1, GeneralGemmPipelineErrorV1> {
    let request = unit
        .symbolic_schedule_proof_request()
        .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    let numerical_request = GeneralGemmNumericalPolicyRequestV1::checked(
        request.symbolic_compilation_identity(),
        request.symbolic_plan_identity(),
        request.symbolic_kir_identity(),
        request.numerical_policy_identity(),
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    let proof = execute_general_gemm_schedule_proof_v1(
        request,
        configuration.verus_path(),
        configuration.proof_timeout_seconds(),
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    let numerical = execute_general_gemm_numerical_policy_v1(
        numerical_request,
        GeneralGemmNumericalComparisonPolicyV1::ExactBits,
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    let evidence = join_general_gemm_proof_and_numerical_evidence_v1(proof, numerical)
        .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    let closure = evaluate_general_gemm_property_closure_v1(evidence)
        .map_err(|error| GeneralGemmPipelineErrorV1::Verifier(error.to_string()))?;
    if closure.proof_request() != request
        || closure.can_enter_compiler_proof_gate()
        || closure.grants_artifact_or_runtime_authority()
    {
        return Err(GeneralGemmPipelineErrorV1::Verifier(
            "verifier closure did not retain the exact non-admitting request".to_owned(),
        ));
    }
    Ok(InertGeneralGemmVerifierClosureV1 {
        schedule: unit.schedule(),
        closure,
    })
}

/// Exact live transaction values used by the synchronous compiler/worker path.
#[derive(Clone, Debug)]
pub(crate) struct GeneralGemmManagedPipelineBindingsV1 {
    output_directory: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
    handoff_receipt: CompilerModuleHandoffReceiptV1,
    consumed_handoff: CompilerModuleHandoffIdentityV1,
}

impl GeneralGemmManagedPipelineBindingsV1 {
    pub(crate) fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub(crate) const fn producer(&self) -> &ProducerIdentity {
        &self.producer
    }

    pub(crate) const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub(crate) const fn slot(&self) -> CompilerModuleHandoffSlotV1 {
        self.slot
    }

    pub(crate) const fn handoff_receipt(&self) -> &CompilerModuleHandoffReceiptV1 {
        &self.handoff_receipt
    }

    pub(crate) const fn consumed_handoff(&self) -> CompilerModuleHandoffIdentityV1 {
        self.consumed_handoff
    }
}

pub(crate) struct InertGeneralGemmScheduleQualificationV1 {
    unit: GeneralGemmSymbolicCompilationUnitV1,
    verifier: InertGeneralGemmVerifierClosureV1,
    managed: GeneralGemmManagedPipelineBindingsV1,
    observation: OpaqueGeneralGemmPostLinkMachineObservationV1,
}

impl InertGeneralGemmScheduleQualificationV1 {
    pub(crate) fn into_join_inputs(
        self,
    ) -> (
        GeneralGemmSymbolicCompilationUnitV1,
        InertGeneralGemmVerifierClosureV1,
        GeneralGemmManagedPipelineBindingsV1,
        OpaqueGeneralGemmPostLinkMachineObservationV1,
    ) {
        (self.unit, self.verifier, self.managed, self.observation)
    }
}

/// Inert owning result retained until rustc's private final join is connected.
///
/// There is deliberately no constructor other than [`execute_general_gemm_pipeline_v1`].
#[allow(dead_code)]
pub(crate) struct InertSynchronousGeneralGemmPipelineV1<FrontendCorrespondence> {
    frontend_correspondence: FrontendCorrespondence,
    configuration: PreparedGeneralGemmPipelineV1,
    qualifications: [InertGeneralGemmScheduleQualificationV1; 2],
}

#[allow(dead_code)]
impl<FrontendCorrespondence> InertSynchronousGeneralGemmPipelineV1<FrontendCorrespondence> {
    pub(crate) fn into_join_inputs(
        self,
    ) -> (
        FrontendCorrespondence,
        PreparedGeneralGemmPipelineV1,
        [InertGeneralGemmScheduleQualificationV1; 2],
    ) {
        (
            self.frontend_correspondence,
            self.configuration,
            self.qualifications,
        )
    }
}

/// Synchronously proves, lowers, executes, and inspects the exact ordered schedule pair while
/// retaining the caller's opaque frontend correspondence in this rustc process.
#[allow(dead_code)]
pub(crate) fn execute_general_gemm_pipeline_v1<FrontendCorrespondence>(
    frontend_correspondence: FrontendCorrespondence,
    units: [GeneralGemmSymbolicCompilationUnitV1; 2],
    configuration: PreparedGeneralGemmPipelineV1,
    output_directory: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<InertSynchronousGeneralGemmPipelineV1<FrontendCorrespondence>, GeneralGemmPipelineErrorV1>
{
    if attempt.session() == BuildSession::DIRECT {
        return Err(GeneralGemmPipelineErrorV1::ManagedBinding(
            "general GEMM requires a non-direct managed build attempt".to_owned(),
        ));
    }
    if output_directory.as_os_str().is_empty() {
        return Err(GeneralGemmPipelineErrorV1::ManagedBinding(
            "general GEMM requires the managed artifact output".to_owned(),
        ));
    }
    if units[0].schedule() != GENERAL_GEMM_QUALIFICATION_SCHEDULES_V1[0]
        || units[1].schedule() != GENERAL_GEMM_QUALIFICATION_SCHEDULES_V1[1]
    {
        return Err(GeneralGemmPipelineErrorV1::ScheduleSubstitution);
    }
    if units[0].frontend_semantics() != units[1].frontend_semantics() {
        return Err(GeneralGemmPipelineErrorV1::FrontendBindingSubstitution);
    }

    let [reference, vectorized_a] = units;
    let reference_verifier = execute_general_gemm_verifier_closure_v1(&reference, &configuration)?;
    let vectorized_a_verifier =
        execute_general_gemm_verifier_closure_v1(&vectorized_a, &configuration)?;
    let reference = execute_general_gemm_schedule_machine_v1(
        reference,
        reference_verifier,
        &configuration,
        output_directory,
        producer,
        attempt,
        CompilerModuleHandoffSlotV1::GeneralGemmReference,
    )?;
    let vectorized_a = execute_general_gemm_schedule_machine_v1(
        vectorized_a,
        vectorized_a_verifier,
        &configuration,
        output_directory,
        producer,
        attempt,
        CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
    )?;

    Ok(InertSynchronousGeneralGemmPipelineV1 {
        frontend_correspondence,
        configuration,
        qualifications: [reference, vectorized_a],
    })
}

fn execute_general_gemm_schedule_machine_v1(
    unit: GeneralGemmSymbolicCompilationUnitV1,
    verifier: InertGeneralGemmVerifierClosureV1,
    configuration: &PreparedGeneralGemmPipelineV1,
    output_directory: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
) -> Result<InertGeneralGemmScheduleQualificationV1, GeneralGemmPipelineErrorV1> {
    if verifier.schedule() != unit.schedule() || slot != slot_for_schedule(unit.schedule()) {
        return Err(GeneralGemmPipelineErrorV1::ScheduleSubstitution);
    }
    let (unit, managed, observation) = execute_general_gemm_schedule_machine_core_v1(
        unit,
        configuration,
        output_directory,
        producer,
        attempt,
        slot,
    )?;
    Ok(InertGeneralGemmScheduleQualificationV1 {
        unit,
        verifier,
        managed,
        observation,
    })
}

fn execute_general_gemm_schedule_machine_core_v1(
    unit: GeneralGemmSymbolicCompilationUnitV1,
    configuration: &PreparedGeneralGemmPipelineV1,
    output_directory: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
) -> Result<
    (
        GeneralGemmSymbolicCompilationUnitV1,
        GeneralGemmManagedPipelineBindingsV1,
        OpaqueGeneralGemmPostLinkMachineObservationV1,
    ),
    GeneralGemmPipelineErrorV1,
> {
    if slot != slot_for_schedule(unit.schedule()) {
        return Err(GeneralGemmPipelineErrorV1::ScheduleSubstitution);
    }
    let machine = lower_general_gemm_symbolic_structural_machine_v1(&unit)
        .map_err(|error| GeneralGemmPipelineErrorV1::Lowering(error.to_string()))?;
    let expected_artifact = machine.artifact_identity();
    let receipt = publish_compiler_module_handoff_in_slot_v1(
        output_directory,
        producer,
        attempt,
        slot,
        machine.compiler_handoff().canonical_bytes(),
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Transaction(error.to_string()))?;
    if receipt.attempt() != attempt || receipt.slot() != slot {
        return Err(GeneralGemmPipelineErrorV1::ManagedBinding(
            "compiler handoff receipt names another attempt".to_owned(),
        ));
    }
    let consumed =
        consume_compiler_module_handoff_in_slot_v1(output_directory, producer, attempt, slot)
            .map_err(|error| GeneralGemmPipelineErrorV1::Transaction(error.to_string()))?;
    if consumed.attempt() != attempt
        || consumed.slot() != slot
        || consumed.identity() != receipt.identity()
        || consumed.bytes() != machine.compiler_handoff().canonical_bytes()
    {
        return Err(GeneralGemmPipelineErrorV1::ManagedBinding(
            "consumed compiler handoff differs from the live attempt publication".to_owned(),
        ));
    }
    let consumed_handoff = consumed.identity();
    let evidence = execute_symbolic_general_gemm_worker_v2_v1(
        machine,
        consumed,
        &configuration.worker,
        configuration.limits,
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Worker(error.to_string()))?;
    let observation = finalize_symbolic_general_gemm_worker_v2_v1(evidence)
        .map_err(|error| GeneralGemmPipelineErrorV1::Finalizer(error.to_string()))?;
    validate_observation(&unit, expected_artifact, consumed_handoff, &observation)?;

    Ok((
        unit,
        GeneralGemmManagedPipelineBindingsV1 {
            output_directory: output_directory.to_path_buf(),
            producer: producer.clone(),
            attempt,
            slot,
            handoff_receipt: receipt,
            consumed_handoff,
        },
        observation,
    ))
}

const fn slot_for_schedule(schedule: GeneralGemmScheduleV1) -> CompilerModuleHandoffSlotV1 {
    match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
            CompilerModuleHandoffSlotV1::GeneralGemmReference
        }
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
            CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly
        }
    }
}

fn validate_observation(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
    expected_artifact: GeneralGemmSymbolicArtifactIdentityV1,
    consumed_handoff: CompilerModuleHandoffIdentityV1,
    observation: &OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> Result<(), GeneralGemmPipelineErrorV1> {
    if observation.schedule() != unit.schedule()
        || observation.schedule_identity() != unit.schedule_identity()
    {
        return Err(GeneralGemmPipelineErrorV1::ScheduleSubstitution);
    }
    if observation.symbolic_compilation_identity() != unit.identity()
        || observation.symbolic_artifact_identity() != expected_artifact
        || observation.consumed_handoff_identity() != consumed_handoff
    {
        return Err(GeneralGemmPipelineErrorV1::ObservationSubstitution);
    }
    if observation.vector_global_load_count()
        != u32::from(unit.schedule() == GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1)
        || observation.barriers_ir() != 2
        || observation.barriers_isa() != 0
        || observation.mfma_numerical_refinement().count() != 1
        || !observation.mfma_numerical_refinement().fp_contract_is_off()
        || observation.grants_artifact_authority()
        || observation.grants_publication_authority()
        || observation.grants_load_authority()
        || observation.grants_launch_authority()
    {
        return Err(GeneralGemmPipelineErrorV1::ObservationSubstitution);
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum GeneralGemmPipelineErrorV1 {
    Configuration(String),
    ManagedBinding(String),
    ScheduleSubstitution,
    FrontendBindingSubstitution,
    Lowering(String),
    Transaction(String),
    Worker(String),
    Finalizer(String),
    Verifier(String),
    ObservationSubstitution,
}

impl fmt::Display for GeneralGemmPipelineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => {
                write!(formatter, "invalid general-GEMM configuration: {reason}")
            }
            Self::ManagedBinding(reason) => {
                write!(formatter, "invalid general-GEMM managed binding: {reason}")
            }
            Self::ScheduleSubstitution => formatter.write_str("general-GEMM schedule substitution"),
            Self::FrontendBindingSubstitution => {
                formatter.write_str("general-GEMM frontend binding substitution")
            }
            Self::Lowering(reason) => write!(formatter, "general-GEMM lowering failed: {reason}"),
            Self::Transaction(reason) => write!(
                formatter,
                "general-GEMM handoff transaction failed: {reason}"
            ),
            Self::Worker(reason) => write!(formatter, "general-GEMM Worker V2 failed: {reason}"),
            Self::Finalizer(reason) => write!(formatter, "general-GEMM finalizer failed: {reason}"),
            Self::Verifier(reason) => write!(formatter, "general-GEMM verifier failed: {reason}"),
            Self::ObservationSubstitution => {
                formatter.write_str("general-GEMM post-link observation substitution")
            }
        }
    }
}

impl std::error::Error for GeneralGemmPipelineErrorV1 {}

fn parse_worker(value: &Value) -> Result<PinnedWorkerV1, GeneralGemmPipelineErrorV1> {
    let object = exact_object(value, WORKER_KEYS, "worker")?;
    let path = PathBuf::from(required_string(object, "path", "worker")?);
    require_absolute_path(&path, "worker")?;
    let identity = ContentIdentityV1::from_parts(
        decode_sha256(
            required_string(object, "sha256", "worker")?,
            "worker.sha256",
        )?,
        required_u64(object, "byte_len", "worker")?,
    );
    let measurement = WorkerMeasurementV1::new(
        identity,
        required_string(object, "worker_build_identity", "worker")?,
        required_string(object, "llvm_build_identity", "worker")?,
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Configuration(error.to_string()))?;
    PinnedWorkerV1::open(path, measurement)
        .map_err(|error| GeneralGemmPipelineErrorV1::Configuration(error.to_string()))
}

fn parse_limits(value: &Value) -> Result<WorkerExecutionLimitsV1, GeneralGemmPipelineErrorV1> {
    let object = exact_object(value, LIMIT_KEYS, "limits")?;
    let stdout =
        usize::try_from(required_u64(object, "stdout_bytes", "limits")?).map_err(|_| {
            GeneralGemmPipelineErrorV1::Configuration("stdout limit exceeds usize".to_owned())
        })?;
    let stderr =
        usize::try_from(required_u64(object, "stderr_bytes", "limits")?).map_err(|_| {
            GeneralGemmPipelineErrorV1::Configuration("stderr limit exceeds usize".to_owned())
        })?;
    WorkerExecutionLimitsV1::new(
        Duration::from_millis(required_u64(object, "timeout_ms", "limits")?),
        stdout,
        stderr,
    )
    .map_err(|error| GeneralGemmPipelineErrorV1::Configuration(error.to_string()))
}

fn parse_fixed_options(value: &Value) -> Result<(), GeneralGemmPipelineErrorV1> {
    let values = value.as_array().ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration("link_options must be an array".to_owned())
    })?;
    if values.len() != FIXED_OPTIONS.len() {
        return Err(GeneralGemmPipelineErrorV1::Configuration(
            "link option count differs from the closed general-GEMM profile".to_owned(),
        ));
    }
    for (index, ((expected_name, expected_value), value)) in
        FIXED_OPTIONS.iter().zip(values).enumerate()
    {
        let context = format!("link_options[{index}]");
        let object = exact_object(value, OPTION_KEYS, &context)?;
        if required_string(object, "name", &context)? != *expected_name
            || required_string(object, "value", &context)? != *expected_value
        {
            return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
                "{context} differs from the closed general-GEMM profile"
            )));
        }
    }
    Ok(())
}

fn require_selected_unit(
    value: &Value,
    crate_name: &str,
    source: &Path,
    working_directory: &Path,
) -> Result<(), GeneralGemmPipelineErrorV1> {
    let values = value.as_array().ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration("units must be an array".to_owned())
    })?;
    if values.is_empty() || values.len() > 1024 {
        return Err(GeneralGemmPipelineErrorV1::Configuration(
            "units must contain 1..=1024 selectors".to_owned(),
        ));
    }
    let source = source.to_str().ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration("rustc source path is not UTF-8".to_owned())
    })?;
    let working_directory = working_directory.to_str().ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration("working directory is not UTF-8".to_owned())
    })?;
    let mut previous: Option<(String, String, String)> = None;
    let mut selected = false;
    for (index, value) in values.iter().enumerate() {
        let context = format!("units[{index}]");
        let object = exact_object(value, UNIT_KEYS, &context)?;
        let unit = (
            required_string(object, "crate_name", &context)?.to_owned(),
            required_string(object, "source", &context)?.to_owned(),
            required_string(object, "working_directory", &context)?.to_owned(),
        );
        require_absolute_path(Path::new(&unit.2), &format!("{context}.working_directory"))?;
        if previous.as_ref().is_some_and(|prior| prior >= &unit) {
            return Err(GeneralGemmPipelineErrorV1::Configuration(
                "units are not strictly ordered".to_owned(),
            ));
        }
        selected |= unit.0 == crate_name && unit.1 == source && unit.2 == working_directory;
        previous = Some(unit);
    }
    if !selected {
        return Err(GeneralGemmPipelineErrorV1::Configuration(
            "manifest does not select this exact rustc compilation unit".to_owned(),
        ));
    }
    Ok(())
}

fn calculate_config_identity(
    manifest: &[u8],
    worker: &PinnedWorkerV1,
) -> GeneralGemmPipelineConfigIdentityV1 {
    let mut hash = Sha256::new();
    update_identity(&mut hash, b"fe2o3-worker-v2-transitive-config-v2");
    update_identity(&mut hash, GENERAL_GEMM_PIPELINE_V1.as_bytes());
    update_identity(&mut hash, manifest);
    let measurement = worker.measurement();
    update_identity(&mut hash, measurement.executable().sha256());
    update_identity(
        &mut hash,
        &measurement.executable().byte_len().to_le_bytes(),
    );
    update_identity(&mut hash, measurement.worker_build_identity().as_bytes());
    update_identity(&mut hash, measurement.llvm_build_identity().as_bytes());
    update_identity(&mut hash, &0_u64.to_le_bytes());
    GeneralGemmPipelineConfigIdentityV1(hash.finalize().into())
}

fn update_identity(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, GeneralGemmPipelineErrorV1> {
    let object = value.as_object().ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration(format!("{context} must be an object"))
    })?;
    if object.keys().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
            "{context} contains unknown, missing, or reordered fields"
        )));
    }
    Ok(object)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Value, GeneralGemmPipelineErrorV1> {
    object.get(name).ok_or_else(|| {
        GeneralGemmPipelineErrorV1::Configuration(format!("{context} is missing {name}"))
    })
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str, GeneralGemmPipelineErrorV1> {
    required_value(object, name, context)?
        .as_str()
        .ok_or_else(|| {
            GeneralGemmPipelineErrorV1::Configuration(format!("{context}.{name} must be a string"))
        })
}

fn required_u64(
    object: &Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<u64, GeneralGemmPipelineErrorV1> {
    required_value(object, name, context)?
        .as_u64()
        .ok_or_else(|| {
            GeneralGemmPipelineErrorV1::Configuration(format!(
                "{context}.{name} must be an unsigned integer"
            ))
        })
}

fn decode_sha256(value: &str, context: &str) -> Result<[u8; 32], GeneralGemmPipelineErrorV1> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
            "{context} must be 64 lowercase hexadecimal digits"
        )));
    }
    let mut result = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(result)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("validated hexadecimal input"),
    }
}

fn require_absolute_path(path: &Path, context: &str) -> Result<(), GeneralGemmPipelineErrorV1> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_CONFIG_PATH_BYTES
        || bytes.contains(&0)
    {
        return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
            "{context} path must be a bounded absolute path"
        )));
    }
    Ok(())
}

fn read_bounded(
    path: &Path,
    maximum: usize,
    context: &str,
) -> Result<Vec<u8>, GeneralGemmPipelineErrorV1> {
    let bytes = fs::read(path).map_err(|error| {
        GeneralGemmPipelineErrorV1::Configuration(format!(
            "cannot read {context} {}: {error}",
            path.display()
        ))
    })?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(GeneralGemmPipelineErrorV1::Configuration(format!(
            "{context} must contain 1..={maximum} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
