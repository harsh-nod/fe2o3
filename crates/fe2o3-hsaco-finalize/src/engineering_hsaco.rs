//! Non-authoritative source-to-HSACO observation through the measured native worker.
//!
//! This module deliberately stops below every production publication, currentness, load, and
//! launch boundary. The engineering request domain is distinct from the protected Worker V3
//! domains, and the returned owner has no conversion into an authority-bearing type.

use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizationError, PinnedWorkerV1, WorkerExecutionError,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerProtocolError, WorkerResponseV2,
    finalize_unfinalized,
    request_construction::{
        DecodedCompilerModuleHandoffV2, WorkerRequestConstructionError,
        decode_compiler_module_handoff_v2, derive_manifest_symbol_closure,
    },
    worker_protocol_v2::{
        SealedWorkerRequestV2Parts, WorkerCompilerFfiEnvelopeIdentityV2, WorkerRequestV2,
    },
};

const ENGINEERING_REQUEST_DOMAIN_V1: &[u8] =
    b"FE2O3/NON-AUTHORITATIVE-ENGINEERING-HSACO-REQUEST/V1\0";
const GFX942_XNACK_MINUS: &str = "gfx942:xnack-";

/// Content identity and kind of one exact engineering-only provider input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineeringProviderObservationV1 {
    kind: WorkerInputKindV1,
    identity: ContentIdentityV1,
}

impl EngineeringProviderObservationV1 {
    pub const fn kind(self) -> WorkerInputKindV1 {
        self.kind
    }

    pub const fn identity(self) -> ContentIdentityV1 {
        self.identity
    }
}

/// Exact, inert result of an engineering compiler observation.
///
/// This owner is intentionally unrelated to protected publication and load owners.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{
///     EngineeringHsacoObservationV1, PublishedProtectedWorkerV3HsacoV1,
/// };
/// fn cannot_publish(value: EngineeringHsacoObservationV1) -> PublishedProtectedWorkerV3HsacoV1 {
///     value.into()
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct EngineeringHsacoObservationV1 {
    finalized_hsaco: Vec<u8>,
    handoff: ContentIdentityV1,
    worker: WorkerMeasurementV1,
    providers: Vec<EngineeringProviderObservationV1>,
    bootstrap_request: ContentIdentityV1,
    bootstrap_response: ContentIdentityV1,
    replay_request: ContentIdentityV1,
    replay_response: ContentIdentityV1,
    finalized_hsaco_identity: ContentIdentityV1,
    canonical_descriptor_digest: [u8; 32],
    kernel_names: Vec<String>,
}

impl EngineeringHsacoObservationV1 {
    pub fn hsaco_bytes(&self) -> &[u8] {
        &self.finalized_hsaco
    }

    pub const fn handoff_identity(&self) -> ContentIdentityV1 {
        self.handoff
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }

    pub fn providers(&self) -> &[EngineeringProviderObservationV1] {
        &self.providers
    }

    pub const fn bootstrap_request_identity(&self) -> ContentIdentityV1 {
        self.bootstrap_request
    }

    pub const fn bootstrap_response_identity(&self) -> ContentIdentityV1 {
        self.bootstrap_response
    }

    pub const fn replay_request_identity(&self) -> ContentIdentityV1 {
        self.replay_request
    }

    pub const fn replay_response_identity(&self) -> ContentIdentityV1 {
        self.replay_response
    }

    pub const fn finalized_hsaco_identity(&self) -> ContentIdentityV1 {
        self.finalized_hsaco_identity
    }

    pub const fn canonical_descriptor_digest(&self) -> &[u8; 32] {
        &self.canonical_descriptor_digest
    }

    pub fn kernel_names(&self) -> &[String] {
        &self.kernel_names
    }

    pub const fn authority(&self) -> &'static str {
        "none"
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Failure to create a non-authoritative engineering HSACO observation.
#[derive(Debug)]
pub struct EngineeringHsacoErrorV1(String);

impl fmt::Display for EngineeringHsacoErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for EngineeringHsacoErrorV1 {}

impl From<WorkerProtocolError> for EngineeringHsacoErrorV1 {
    fn from(error: WorkerProtocolError) -> Self {
        Self(format!(
            "engineering worker protocol rejected input: {error}"
        ))
    }
}

impl From<WorkerRequestConstructionError> for EngineeringHsacoErrorV1 {
    fn from(error: WorkerRequestConstructionError) -> Self {
        Self(format!(
            "engineering worker request rejected input: {error}"
        ))
    }
}

impl From<WorkerExecutionError> for EngineeringHsacoErrorV1 {
    fn from(error: WorkerExecutionError) -> Self {
        Self(format!("engineering measured worker failed: {error}"))
    }
}

impl From<FinalizationError> for EngineeringHsacoErrorV1 {
    fn from(error: FinalizationError) -> Self {
        Self(format!(
            "engineering HSACO inspection/finalization failed: {error}"
        ))
    }
}

/// Executes an inert compiler handoff twice through one measured worker and finalizes the exact
/// matching output for descriptive inspection only.
///
/// The target and options are fixed to `gfx942:xnack-`, COV6, O2, strip-debug, and verify-each.
/// No production receipt, carriage, currentness, publication, load, or launch value is accepted
/// or returned.
pub fn observe_engineering_hsaco_v1(
    handoff_bytes: &[u8],
    worker: &PinnedWorkerV1,
    mut external_providers: Vec<WorkerInputV1>,
    output_bound: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<EngineeringHsacoObservationV1, EngineeringHsacoErrorV1> {
    let decoded = decode_compiler_module_handoff_v2(handoff_bytes)?;
    if decoded.target().to_string() != GFX942_XNACK_MINUS {
        return Err(EngineeringHsacoErrorV1(format!(
            "engineering route requires exact target {GFX942_XNACK_MINUS}, handoff selected {}",
            decoded.target()
        )));
    }
    if decoded.code_object_version() != CodeObjectVersion::V6 {
        return Err(EngineeringHsacoErrorV1(
            "engineering route requires code-object version 6".to_owned(),
        ));
    }

    external_providers.sort_by_key(|input| (input.identity(), input.kind()));
    for pair in external_providers.windows(2) {
        if pair[0].identity() == pair[1].identity() {
            return Err(EngineeringHsacoErrorV1(
                "engineering providers contain a duplicate content identity".to_owned(),
            ));
        }
    }
    let provider_observations = external_providers
        .iter()
        .map(|provider| EngineeringProviderObservationV1 {
            kind: provider.kind(),
            identity: provider.identity(),
        })
        .collect();
    let options = WorkerOptionsV1::new(crate::WorkerOptimizationLevelV1::O2, true, true);
    let handoff_identity = ContentIdentityV1::calculate(handoff_bytes);

    let bootstrap_request = construct_engineering_request(
        &decoded,
        worker.measurement(),
        external_providers.clone(),
        options,
        output_bound,
        handoff_identity,
        EngineeringPhaseV1::Bootstrap,
    )?;
    let bootstrap_request_identity =
        ContentIdentityV1::calculate(bootstrap_request.canonical_bytes());
    let bootstrap = worker.execute_v2(&bootstrap_request, limits)?;
    let bootstrap_response = bootstrap.response();
    let bootstrap_response_identity =
        ContentIdentityV1::calculate(bootstrap_response.canonical_bytes());
    let bootstrap_output = bootstrap_response.output().ok_or_else(|| {
        EngineeringHsacoErrorV1("engineering bootstrap produced no HSACO".to_owned())
    })?;
    if !bootstrap_output
        .identity()
        .matches(bootstrap_output.bytes())
    {
        return Err(EngineeringHsacoErrorV1(
            "engineering bootstrap output identity does not match its bytes".to_owned(),
        ));
    }

    let replay_request = construct_engineering_request(
        &decoded,
        worker.measurement(),
        external_providers,
        options,
        WorkerOutputConstraintsV1::new(bootstrap_output.identity().byte_len())?,
        handoff_identity,
        EngineeringPhaseV1::Replay(bootstrap_output.identity()),
    )?;
    let replay_request_identity = ContentIdentityV1::calculate(replay_request.canonical_bytes());
    let replay = worker.execute_v2(&replay_request, limits)?;
    let replay_response = replay.response();
    let replay_response_identity = ContentIdentityV1::calculate(replay_response.canonical_bytes());
    let replay_output = replay_response.output().ok_or_else(|| {
        EngineeringHsacoErrorV1("engineering exact replay produced no HSACO".to_owned())
    })?;

    if bootstrap_output.identity() != replay_output.identity()
        || bootstrap_output.bytes() != replay_output.bytes()
    {
        return Err(EngineeringHsacoErrorV1(
            "engineering bootstrap/replay HSACO bytes differ".to_owned(),
        ));
    }
    let bootstrap_derivation = bootstrap_response.derivation().ok_or_else(|| {
        EngineeringHsacoErrorV1("engineering bootstrap omitted derivation evidence".to_owned())
    })?;
    let replay_derivation = replay_response.derivation().ok_or_else(|| {
        EngineeringHsacoErrorV1("engineering replay omitted derivation evidence".to_owned())
    })?;
    if bootstrap_derivation != replay_derivation
        || bootstrap_derivation.hsaco() != bootstrap_output.identity()
        || bootstrap_response.device_library_provider() != replay_response.device_library_provider()
    {
        return Err(EngineeringHsacoErrorV1(
            "engineering bootstrap/replay derivation evidence differs".to_owned(),
        ));
    }

    let finalized = finalize_unfinalized(bootstrap_output.bytes())?;
    let inspection = finalized.inspection();
    if inspection.hsaco().target().to_string() != GFX942_XNACK_MINUS
        || inspection.hsaco().code_object_version().number() != 6
        || inspection.hsaco().kernels().is_empty()
    {
        return Err(EngineeringHsacoErrorV1(
            "engineering output is not a nonempty gfx942:xnack- COV6 HSACO".to_owned(),
        ));
    }
    let canonical_descriptor_digest = *inspection.digest().as_bytes();
    let kernel_names = inspection
        .hsaco()
        .kernels()
        .iter()
        .map(|kernel| kernel.name().to_owned())
        .collect();
    let finalized_hsaco = finalized.into_bytes();
    let finalized_hsaco_identity = ContentIdentityV1::calculate(&finalized_hsaco);

    Ok(EngineeringHsacoObservationV1 {
        finalized_hsaco,
        handoff: handoff_identity,
        worker: worker.measurement().clone(),
        providers: provider_observations,
        bootstrap_request: bootstrap_request_identity,
        bootstrap_response: bootstrap_response_identity,
        replay_request: replay_request_identity,
        replay_response: replay_response_identity,
        finalized_hsaco_identity,
        canonical_descriptor_digest,
        kernel_names,
    })
}

#[derive(Clone, Copy)]
enum EngineeringPhaseV1 {
    Bootstrap,
    Replay(ContentIdentityV1),
}

#[allow(clippy::too_many_arguments)]
fn construct_engineering_request(
    decoded: &DecodedCompilerModuleHandoffV2,
    measurement: &WorkerMeasurementV1,
    external_providers: Vec<WorkerInputV1>,
    options: WorkerOptionsV1,
    output: WorkerOutputConstraintsV1,
    handoff_identity: ContentIdentityV1,
    phase: EngineeringPhaseV1,
) -> Result<WorkerRequestV2, EngineeringHsacoErrorV1> {
    let directional = decoded.envelope().directional_symbols();
    let symbols = derive_manifest_symbol_closure(
        decoded.symbol_manifest(),
        directional.imports().map(str::to_owned).collect(),
        directional.exports().map(str::to_owned).collect(),
    )?;
    let compiler_module = WorkerInputV1::new(
        decoded.compiler_module_kind(),
        decoded.compiler_module_bytes().to_vec(),
    )?;
    let request_id = engineering_request_id(
        decoded,
        measurement,
        &external_providers,
        options,
        &output,
        handoff_identity,
        phase,
    );
    WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
        request_id,
        llvm_build_identity: measurement.llvm_build_identity().to_owned(),
        worker_build_identity: measurement.worker_build_identity().to_owned(),
        worker_executable: measurement.executable(),
        target: decoded.target(),
        code_object_version: decoded.code_object_version(),
        options,
        compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
            decoded.envelope().identity(),
        ),
        compiler_module,
        external_providers,
        import_symbols: symbols.import_symbols().to_vec(),
        export_symbols: symbols.export_symbols().to_vec(),
        final_symbols: symbols.required_symbols().to_vec(),
        output,
    })
    .map_err(EngineeringHsacoErrorV1::from)
}

#[allow(clippy::too_many_arguments)]
fn engineering_request_id(
    decoded: &DecodedCompilerModuleHandoffV2,
    measurement: &WorkerMeasurementV1,
    providers: &[WorkerInputV1],
    options: WorkerOptionsV1,
    output: &WorkerOutputConstraintsV1,
    handoff: ContentIdentityV1,
    phase: EngineeringPhaseV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ENGINEERING_REQUEST_DOMAIN_V1);
    match phase {
        EngineeringPhaseV1::Bootstrap => hasher.update([1]),
        EngineeringPhaseV1::Replay(identity) => {
            hasher.update([2]);
            hash_identity(&mut hasher, identity);
        }
    }
    hash_identity(&mut hasher, handoff);
    hash_identity(&mut hasher, measurement.executable());
    hash_blob(&mut hasher, measurement.worker_build_identity().as_bytes());
    hash_blob(&mut hasher, measurement.llvm_build_identity().as_bytes());
    hash_blob(&mut hasher, decoded.target().to_string().as_bytes());
    hasher.update([decoded.code_object_version().number()]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hasher.update((providers.len() as u64).to_le_bytes());
    for provider in providers {
        hasher.update([provider.kind() as u8]);
        hash_identity(&mut hasher, provider.identity());
    }
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

fn hash_identity(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_blob(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_type_is_explicitly_non_authoritative() {
        let source = include_str!("engineering_hsaco.rs");
        for forbidden in [
            concat!("compiler_execution", "_client"),
            concat!("CompilerModuleHandoff", "ReceiptV3"),
            concat!("PublishedProtectedWorker", "V3HsacoV1 {"),
            concat!("CUR", "RENT"),
            concat!("/run/fe2o3/", "compiler-execution-supervisor.sock"),
        ] {
            assert!(
                !source.contains(forbidden),
                "engineering implementation references forbidden authority surface {forbidden}"
            );
        }
        assert!(source.contains("FE2O3/NON-AUTHORITATIVE-ENGINEERING-HSACO-REQUEST/V1"));
        assert!(source.contains("authority(&self) -> &'static str"));
    }

    #[test]
    fn request_domains_separate_bootstrap_and_exact_replay() {
        assert_ne!(
            EngineeringPhaseV1::Bootstrap.as_phase_tag(),
            EngineeringPhaseV1::Replay(ContentIdentityV1::from_parts([1; 32], 1)).as_phase_tag(),
        );
    }

    trait PhaseTag {
        fn as_phase_tag(self) -> u8;
    }

    impl PhaseTag for EngineeringPhaseV1 {
        fn as_phase_tag(self) -> u8 {
            match self {
                EngineeringPhaseV1::Bootstrap => 1,
                EngineeringPhaseV1::Replay(_) => 2,
            }
        }
    }
}
