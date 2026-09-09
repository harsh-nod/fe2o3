use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::Duration;

use fe2o3_artifact_transaction::{
    AuthenticatedCompilerCapabilityCompletionV5, CompletedCompilerCapabilityTransactionV5,
    InertCompilerCapabilityVerifierResponseV5, MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5,
    NoRetainedDurableDirectoryHooksV1, PreparedCompilerCapabilityCompletionV5,
    RecoverableCompilerCapabilityCompletionErrorV5,
    RejectedAuthenticatedCompilerCapabilityCompletionCustodyV5, RejectedW4WitnessBindingCustodyV5,
    RetainedDurableDirectoryV1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV1;
use fe2o3_compiler_ffi::InertProductionCapabilityTransactionV5;
use fe2o3_compiler_lineage::MultiRootProofRosterKindV3;
use fe2o3_hsaco_finalize::{
    MachineRefinementPendingFinalizedProtectedWorkerV3HsacoV1,
    PreparedFinalizedProtectedWorkerV3HsacoV1,
};
use fe2o3_runtime_protocol::{
    WorkerV3CapabilityAncillaryEvidenceV1, WorkerV3LoadEnvelopeV2,
    WorkerV3PendingCapabilityResultV1,
};
use fe2o3_worker_v3_verification_client::{
    IntakeAuthenticatedWorkerV3CapabilityCustodyV5, PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5,
    WorkerV3VerificationCapabilityClientV5, WorkerV3VerificationCapabilityCompletedReceiptV5,
    WorkerV3VerificationCapabilityExchangeOutcomeV5, WorkerV3VerificationCapabilityPeerPolicyV5,
    WorkerV3VerificationCapabilityRequestPlanV5, WorkerV3VerificationClientFailureQuarantineV5,
    WorkerV3VerificationPayloadSnapshotsV1, WorkerV3VerificationResponseReplayGuardV5,
    production_worker_v3_verifier_measurement_identity_v5,
};
use fe2o3_worker_v3_verification_protocol::{
    ExactIdentityCoordinateV5, InertWorkerV3MachineRefinedFinalizationV5,
    InertWorkerV3ProtectedCompilerInputV5, MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
    MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
    WorkerV3VerificationCapabilityRequestIdentityV5, WorkerV3VerificationCapabilityRequestV5,
    WorkerV3VerificationCapabilityResponseV5, WorkerV3VerificationEntryCoordinateV1,
    WorkerV3VerificationFdPayloadDescriptorV1, WorkerV3VerificationFreshChallengeV1,
    WorkerV3VerificationMeasurementIdentityV1, WorkerV3VerificationPolicyIdentityV1,
    WorkerV3VerificationProductionAttemptV5, WorkerV3VerificationRosterIdentityV1,
    worker_v3_protected_compiler_input_name_v5, worker_v3_protected_compiler_input_redo_name_v5,
};
use fe2o3_worker_v3_verification_service::WorkerV3ProtectedCompletionRequestV5;
use rustix::fs::{MemfdFlags, Mode, SealFlags};
use sha2::{Digest as _, Sha256};

/// Fixed service selected by both Cargo diagnostics and the V5 connected-path client.
pub(crate) const PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1: &str =
    PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5;

const PROTECTED_RESPONSE_MAGIC_V5: [u8; 8] = *b"F2WVPS05";
const PROTECTED_RESPONSE_VERSION_V5: u16 = 5;
const PROTECTED_RESPONSE_FIELD_COUNT_V5: usize = 6;
const PROTECTED_RESPONSE_HEADER_BYTES_V5: usize = 24;
const PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5: usize = 8;
const PROTECTED_RESPONSE_IDENTITY_BYTES_V5: usize = 32;
const PROTECTED_RESPONSE_DOMAIN_V5: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE/V5\0";
const PROTECTED_RESPONSE_IDENTITY_DOMAIN_V5: &[u8] =
    b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE-IDENTITY/V5\0";
const MAX_PROTECTED_RESPONSE_BYTES_V5: usize = PROTECTED_RESPONSE_HEADER_BYTES_V5
    + PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5 * PROTECTED_RESPONSE_FIELD_COUNT_V5
    + PROTECTED_RESPONSE_DOMAIN_V5.len()
    + 32
    + 32
    + MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5
    + MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5
    + 64
    + PROTECTED_RESPONSE_IDENTITY_BYTES_V5;
const PROTECTED_EXCHANGE_TIMEOUT_V5: Duration = Duration::from_secs(30);
const CLIENT_REPLAY_DOMAIN_V5: &[u8] = b"FE2O3/WORKER-V3/CLIENT-RESPONSE-REPLAY/V5\0";
const CLIENT_QUARANTINE_DOMAIN_V5: &[u8] = b"FE2O3/WORKER-V3/CLIENT-QUARANTINE/V5\0";
const REQUIRED_PAYLOAD_SEALS_V5: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

/// Move-only pre-publication custody for the native V5 completion join.
#[must_use = "native V5 custody must reach authenticated completion or fail as one unit"]
pub(crate) struct PendingProductionCapabilityCompletionJoinV5 {
    prepared: PreparedCompilerCapabilityCompletionV5,
    finalized: MachineRefinementPendingFinalizedProtectedWorkerV3HsacoV1,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
}

/// One-shot Cargo continuation supplied only by the authenticated compiler-artifact boundary.
///
/// The executor owns the connected protected client, exact immutable payload snapshots, durable
/// replay/quarantine state. A Rust object is never represented as crossing the process boundary:
/// the process-local W4 custody token is reconstructed only after the protected response has been
/// authenticated and remains bound to the exact handoff carrying those W4 bytes.
pub(crate) struct ProductionCapabilityCompletionExecutorV5 {
    execute: Box<ProductionCapabilityCompletionOperationV5>,
}

type ProductionCapabilityCompletionOperationV5 = dyn FnOnce(
    PendingProductionCapabilityCompletionJoinV5,
) -> Result<
    CompletedProductionCapabilityCompletionJoinV5,
    ProductionCapabilityCompletionExecutionErrorV5,
>;

impl ProductionCapabilityCompletionExecutorV5 {
    /// Recovers the exact protected-compiler package and captures the fixed V5 exchange.
    pub(crate) fn from_protected_build(
        output_dir: &Path,
        expected_attempt: fe2o3_artifact_transaction::BuildAttempt,
        source_profile: &CompilerExecutionClientProfileCapabilityV1,
    ) -> Result<Self, String> {
        source_profile.revalidate()?;
        let profile = CompilerExecutionClientProfileCapabilityV1::from_file(
            source_profile.try_clone_for_transfer()?,
        )?;
        if profile.profile() != source_profile.profile() {
            return Err(
                "retained V5 verifier profile differs from protected compiler custody".to_owned(),
            );
        }
        let input_root = retain_durable_root(output_dir)?;
        let replay_root = retain_durable_root(output_dir)?;
        let quarantine_root = retain_durable_root(output_dir)?;
        let execute = Box::new(move |join: PendingProductionCapabilityCompletionJoinV5| {
            if let Err(error) = profile.revalidate() {
                return Err(ProductionCapabilityCompletionExecutionErrorV5::Terminal(
                    ProductionCapabilityCompletionJoinErrorV5::ClientProfileChanged(error),
                ));
            }
            if join.prepared.attempt() != expected_attempt {
                return Err(ProductionCapabilityCompletionExecutionErrorV5::Exchange(
                    join.into_exchange_failure(
                        ProductionCapabilityCompletionJoinErrorV5::AttemptMismatch,
                    ),
                ));
            }
            let prepared = match prepare_protected_exchange_v5(&join, &profile, &input_root) {
                Ok(prepared) => prepared,
                Err(reason) => {
                    return Err(ProductionCapabilityCompletionExecutionErrorV5::Exchange(
                        join.into_exchange_failure(reason),
                    ));
                }
            };
            let replay = DurableCapabilityClientStateV5::new(replay_root, prepared.attempt);
            let quarantine = DurableCapabilityClientStateV5::new(quarantine_root, prepared.attempt);
            Self::from_authenticated_production_artifacts(
                prepared.client,
                prepared.custody,
                prepared.snapshots,
                replay,
                quarantine,
            )
            .execute(join)
        });
        Ok(Self { execute })
    }

    /// Captures every move-only input needed by the real protected exchange.
    ///
    /// This API intentionally accepts typed upstream custody rather than proof bytes. Building the
    /// prepublication evidence snapshot from a #213 owner, complete association roster, and #214
    /// machine receipt remains the responsibility of their owning production stages.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_authenticated_production_artifacts<R, Q>(
        client: WorkerV3VerificationCapabilityClientV5,
        custody: IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
        snapshots: WorkerV3VerificationPayloadSnapshotsV1,
        response_replay: R,
        quarantine: Q,
    ) -> Self
    where
        R: WorkerV3VerificationResponseReplayGuardV5 + 'static,
        Q: WorkerV3VerificationClientFailureQuarantineV5 + 'static,
    {
        let execute = Box::new(move |join: PendingProductionCapabilityCompletionJoinV5| {
            let mut response_replay = response_replay;
            let mut quarantine = quarantine;
            let correlated = join
                .exchange_canonical_request(
                    client,
                    custody,
                    snapshots,
                    &mut response_replay,
                    &mut quarantine,
                )
                .map_err(ProductionCapabilityCompletionExecutionErrorV5::Exchange)?;
            let authenticated = correlated
                .authenticate(&mut quarantine)
                .map_err(ProductionCapabilityCompletionExecutionErrorV5::Exchange)?;
            authenticated.complete(&mut quarantine)
        });
        Self { execute }
    }

    fn execute(
        self,
        join: PendingProductionCapabilityCompletionJoinV5,
    ) -> Result<
        CompletedProductionCapabilityCompletionJoinV5,
        ProductionCapabilityCompletionExecutionErrorV5,
    > {
        (self.execute)(join)
    }
}

struct PreparedProtectedExchangeV5 {
    attempt: WorkerV3VerificationProductionAttemptV5,
    client: WorkerV3VerificationCapabilityClientV5,
    custody: IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
    snapshots: WorkerV3VerificationPayloadSnapshotsV1,
}

fn prepare_protected_exchange_v5(
    join: &PendingProductionCapabilityCompletionJoinV5,
    profile: &CompilerExecutionClientProfileCapabilityV1,
    input_root: &RetainedDurableDirectoryV1,
) -> Result<PreparedProtectedExchangeV5, ProductionCapabilityCompletionJoinErrorV5> {
    profile
        .revalidate()
        .map_err(ProductionCapabilityCompletionJoinErrorV5::ClientProfileChanged)?;
    let attempt = verification_attempt_v5(join.prepared.attempt())?;
    let package = recover_protected_compiler_input_v5(input_root, attempt)?;
    let transaction = validate_protected_compiler_input_v5(&package, &join.prepared, attempt)?;

    let handoff = join.prepared.handoff();
    let roster = handoff
        .proof_lineage()
        .roster(MultiRootProofRosterKindV3::MiddleEnd);
    if roster.root_count() != package.generated_host_contract_identities().len() {
        return Err(
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                "generated-host contract roster length differs from the exact proof roster"
                    .to_owned(),
            ),
        );
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(roster.root_count())
        .map_err(|_| {
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                "cannot allocate the exact verifier entry roster".to_owned(),
            )
        })?;
    for ordinal in 0..roster.root_count() {
        let root = roster.root(ordinal).ok_or_else(|| {
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                "proof roster lost a canonical root".to_owned(),
            )
        })?;
        entries.push(
            WorkerV3VerificationEntryCoordinateV1::new(
                u32::try_from(ordinal).map_err(|_| {
                    ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                        "proof roster ordinal exceeds the V5 protocol".to_owned(),
                    )
                })?,
                root.logical_name(),
                root.export_symbol(),
                root.semantic_root_identity(),
                root.kernel_binding(),
                package.generated_host_contract_identities()[ordinal],
            )
            .map_err(|error| {
                ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
            })?,
        );
    }

    let mut challenge_bytes = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut challenge_bytes))
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::ChallengeEntropy(error.to_string())
        })?;
    let challenge =
        WorkerV3VerificationFreshChallengeV1::new(challenge_bytes).map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
        })?;
    let roster_identity = WorkerV3VerificationRosterIdentityV1::new(roster.roster_identity())
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
        })?;
    let compiler_profile = profile.profile();
    let policy_identity = *compiler_profile.policy().identity().as_bytes();
    let measurement_identity =
        production_worker_v3_verifier_measurement_identity_v5(compiler_profile);
    let policy = WorkerV3VerificationPolicyIdentityV1::new(policy_identity).map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
    })?;
    let measurement = WorkerV3VerificationMeasurementIdentityV1::new(measurement_identity)
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
        })?;
    let plan = WorkerV3VerificationCapabilityRequestPlanV5::from_prepared_completion(
        challenge,
        roster_identity,
        policy,
        measurement,
        entries,
        &join.prepared,
    )
    .map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
    })?;
    let machine_receipt = join
        .finalized
        .machine_refinement_receipt_v1()
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::MachineRefinement(error.to_string())
        })?;
    let raw_object = join.finalized.raw_output_identity();
    let finalized_object = join.finalized.finalized_output_identity();
    let subject = handoff.inputs().subject();
    let finalization = InertWorkerV3MachineRefinedFinalizationV5::new(
        attempt,
        *join.prepared.transaction_identity().as_bytes(),
        ExactIdentityCoordinateV5::new(handoff.identity().sha256(), handoff.identity().byte_len())
            .map_err(|error| {
                ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
            })?,
        ExactIdentityCoordinateV5::new(*raw_object.sha256(), raw_object.byte_len()).map_err(
            |error| ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string()),
        )?,
        ExactIdentityCoordinateV5::new(*finalized_object.sha256(), finalized_object.byte_len())
            .map_err(|error| {
                ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
            })?,
        *subject.target_model().digest().as_bytes(),
        *subject.launch_contract().digest().as_bytes(),
        handoff.inputs().compiler_policy(),
        machine_receipt.canonical_bytes(),
    )
    .map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
    })?;
    let protected = WorkerV3ProtectedCompletionRequestV5::new(
        plan.protected_evidence_binding(),
        &transaction,
        &finalization,
    )
    .map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
    })?;
    let evidence_descriptor =
        WorkerV3VerificationFdPayloadDescriptorV1::protected_completion_evidence_v5(
            u64::try_from(protected.canonical_bytes().len()).map_err(|_| {
                ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(
                    "protected completion evidence exceeds the V5 length domain".to_owned(),
                )
            })?,
            Sha256::digest(protected.canonical_bytes()).into(),
        )
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
        })?;
    let evidence = sealed_payload_v5(
        "fe2o3-worker-v3-protected-completion-v5",
        protected.canonical_bytes(),
    )?;
    let object = sealed_payload_v5(
        "fe2o3-worker-v3-finalized-hsaco-v5",
        join.prepared.object_bytes(),
    )?;
    let custody = plan.complete(evidence_descriptor).map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::CanonicalRequest(error.to_string())
    })?;
    let snapshots = WorkerV3VerificationPayloadSnapshotsV1::admit(
        custody.request().base_request(),
        vec![evidence, object],
    )
    .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Client(error.to_string()))?;
    let peer_policy = WorkerV3VerificationCapabilityPeerPolicyV5::new(
        compiler_profile.supervisor_uid(),
        compiler_profile.supervisor_gid(),
        policy_identity,
        measurement_identity,
    )
    .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Client(error.to_string()))?;
    let client = WorkerV3VerificationCapabilityClientV5::connect_production(
        peer_policy,
        PROTECTED_EXCHANGE_TIMEOUT_V5,
    )
    .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Client(error.to_string()))?;
    Ok(PreparedProtectedExchangeV5 {
        attempt,
        client,
        custody,
        snapshots,
    })
}

fn verification_attempt_v5(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
) -> Result<WorkerV3VerificationProductionAttemptV5, ProductionCapabilityCompletionJoinErrorV5> {
    WorkerV3VerificationProductionAttemptV5::new(
        attempt.generation(),
        *attempt.session().as_bytes(),
        *attempt.invocation().as_bytes(),
    )
    .map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
    })
}

fn validate_protected_compiler_input_v5(
    package: &InertWorkerV3ProtectedCompilerInputV5,
    prepared: &PreparedCompilerCapabilityCompletionV5,
    attempt: WorkerV3VerificationProductionAttemptV5,
) -> Result<InertProductionCapabilityTransactionV5, ProductionCapabilityCompletionJoinErrorV5> {
    let handoff = prepared.handoff().identity();
    if package.attempt() != attempt
        || package.transaction_identity() != *prepared.transaction_identity().as_bytes()
        || package.handoff_identity().sha256() != handoff.sha256()
        || package.handoff_identity().byte_len() != handoff.byte_len()
    {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
            "attempt, transaction, or typed handoff coordinate does not match live Cargo custody"
                .to_owned(),
        ));
    }
    if package.production_transaction_bytes() != prepared.production_transaction().canonical_bytes()
    {
        return Err(
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                "protected compiler transaction bytes differ from live Cargo custody".to_owned(),
            ),
        );
    }
    let transaction =
        InertProductionCapabilityTransactionV5::decode(package.production_transaction_bytes())
            .map_err(|error| {
                ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
            })?;
    if transaction.identity() != prepared.production_transaction().identity() {
        return Err(
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                "protected compiler transaction identity differs from live Cargo custody"
                    .to_owned(),
            ),
        );
    }
    Ok(transaction)
}

fn recover_protected_compiler_input_v5(
    root: &RetainedDurableDirectoryV1,
    attempt: WorkerV3VerificationProductionAttemptV5,
) -> Result<InertWorkerV3ProtectedCompilerInputV5, ProductionCapabilityCompletionJoinErrorV5> {
    let canonical = worker_v3_protected_compiler_input_name_v5(attempt);
    let redo = worker_v3_protected_compiler_input_redo_name_v5(attempt);
    let canonical_bytes = root
        .read_private(&canonical, MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5)
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
        })?;
    let redo_bytes = root
        .read_private(&redo, MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5)
        .map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
        })?;
    let bytes = match (canonical_bytes, redo_bytes) {
        (None, None) => {
            return Err(
                ProductionCapabilityCompletionJoinErrorV5::AuthenticatedCompilerArtifactsUnavailable,
            );
        }
        (Some(bytes), None) => bytes,
        (expected, Some(redo_bytes)) => {
            if expected.as_deref().is_some_and(|bytes| bytes != redo_bytes) {
                return Err(
                    ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(
                        "canonical and redo compiler-input journals disagree".to_owned(),
                    ),
                );
            }
            let mut hooks = NoRetainedDurableDirectoryHooksV1;
            root.promote_validated_redo(
                &canonical,
                &redo,
                expected.as_deref(),
                &redo_bytes,
                MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
                &mut hooks,
            )
            .map_err(|error| {
                ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
            })?;
            redo_bytes
        }
    };
    InertWorkerV3ProtectedCompilerInputV5::decode_canonical(&bytes).map_err(|error| {
        ProductionCapabilityCompletionJoinErrorV5::ProtectedCompilerInput(error.to_string())
    })
}

fn sealed_payload_v5(
    name: &str,
    bytes: &[u8],
) -> Result<OwnedFd, ProductionCapabilityCompletionJoinErrorV5> {
    let descriptor =
        rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).map_err(
            |error| ProductionCapabilityCompletionJoinErrorV5::Payload(error.to_string()),
        )?;
    let mut file = File::from(descriptor);
    rustix::fs::fchmod(&file, Mode::RUSR)
        .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Payload(error.to_string()))?;
    file.write_all(bytes)
        .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Payload(error.to_string()))?;
    rustix::fs::fcntl_add_seals(&file, REQUIRED_PAYLOAD_SEALS_V5)
        .map_err(|error| ProductionCapabilityCompletionJoinErrorV5::Payload(error.to_string()))?;
    Ok(file.into())
}

fn retain_durable_root(output_dir: &Path) -> Result<RetainedDurableDirectoryV1, String> {
    let descriptor = File::open(output_dir)
        .map(OwnedFd::from)
        .map_err(|error| format!("cannot retain native V5 output root: {error}"))?;
    RetainedDurableDirectoryV1::admit_service_owned(descriptor)
        .map_err(|error| format!("cannot admit native V5 output root: {error}"))
}

struct DurableCapabilityClientStateV5 {
    root: RetainedDurableDirectoryV1,
    attempt: WorkerV3VerificationProductionAttemptV5,
}

impl DurableCapabilityClientStateV5 {
    const fn new(
        root: RetainedDurableDirectoryV1,
        attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Self {
        Self { root, attempt }
    }

    fn record_name(&self, class: &str, suffix: &str) -> String {
        format!(
            ".fe2o3-worker-v3-v5-{class}-{}-{}-{}-{suffix}",
            self.attempt.generation(),
            hex(&self.attempt.session()),
            hex(&self.attempt.invocation()),
        )
    }
}

impl WorkerV3VerificationResponseReplayGuardV5 for DurableCapabilityClientStateV5 {
    fn admit_response_once(
        &mut self,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        response: fe2o3_worker_v3_verification_protocol::WorkerV3VerificationCapabilityResponseIdentityV5,
    ) -> bool {
        let mut bytes = Vec::with_capacity(CLIENT_REPLAY_DOMAIN_V5.len() + 64);
        bytes.extend_from_slice(CLIENT_REPLAY_DOMAIN_V5);
        bytes.extend_from_slice(request.as_bytes());
        bytes.extend_from_slice(response.as_bytes());
        commit_once_v5(
            &self.root,
            &self.record_name("response", "record"),
            &self.record_name("response", "redo"),
            &bytes,
        )
        .unwrap_or(false)
    }
}

impl WorkerV3VerificationClientFailureQuarantineV5 for DurableCapabilityClientStateV5 {
    type Error = DurableCapabilityClientStateErrorV5;

    fn quarantine_failed_exchange(
        &mut self,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Result<(), Self::Error> {
        if attempt != self.attempt {
            return Err(DurableCapabilityClientStateErrorV5(
                "quarantine attempt differs from retained state".to_owned(),
            ));
        }
        let mut bytes = Vec::with_capacity(CLIENT_QUARANTINE_DOMAIN_V5.len() + 88);
        bytes.extend_from_slice(CLIENT_QUARANTINE_DOMAIN_V5);
        bytes.extend_from_slice(request.as_bytes());
        bytes.extend_from_slice(&attempt.generation().to_le_bytes());
        bytes.extend_from_slice(&attempt.session());
        bytes.extend_from_slice(&attempt.invocation());
        commit_idempotent_v5(
            &self.root,
            &self.record_name("quarantine", "record"),
            &self.record_name("quarantine", "redo"),
            &bytes,
        )
    }
}

fn commit_once_v5(
    root: &RetainedDurableDirectoryV1,
    canonical: &str,
    redo: &str,
    bytes: &[u8],
) -> Result<bool, DurableCapabilityClientStateErrorV5> {
    match root
        .read_private(canonical, bytes.len())
        .map_err(durable_client_error_v5)?
    {
        Some(_) => Ok(false),
        None => {
            let pending = root
                .read_private(redo, bytes.len())
                .map_err(durable_client_error_v5)?;
            let mut hooks = NoRetainedDurableDirectoryHooksV1;
            if let Some(pending) = pending {
                if pending != bytes {
                    return Err(DurableCapabilityClientStateErrorV5(
                        "response replay redo record differs".to_owned(),
                    ));
                }
                root.promote_validated_redo(canonical, redo, None, bytes, bytes.len(), &mut hooks)
                    .map_err(durable_client_error_v5)?;
            } else {
                root.commit_record(canonical, redo, bytes, bytes.len(), &mut hooks)
                    .map_err(durable_client_error_v5)?;
            }
            Ok(true)
        }
    }
}

fn commit_idempotent_v5(
    root: &RetainedDurableDirectoryV1,
    canonical: &str,
    redo: &str,
    bytes: &[u8],
) -> Result<(), DurableCapabilityClientStateErrorV5> {
    if let Some(existing) = root
        .read_private(canonical, bytes.len())
        .map_err(durable_client_error_v5)?
    {
        return if existing == bytes {
            Ok(())
        } else {
            Err(DurableCapabilityClientStateErrorV5(
                "durable quarantine record differs".to_owned(),
            ))
        };
    }
    commit_once_v5(root, canonical, redo, bytes).map(|_| ())
}

fn durable_client_error_v5(error: impl fmt::Display) -> DurableCapabilityClientStateErrorV5 {
    DurableCapabilityClientStateErrorV5(error.to_string())
}

#[derive(Debug)]
struct DurableCapabilityClientStateErrorV5(String);

impl fmt::Display for DurableCapabilityClientStateErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for DurableCapabilityClientStateErrorV5 {}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

/// Completed V5 result while finalization and compiler-execution custody are still live.
#[must_use = "completed capability custody must be persisted with the matching load envelope"]
pub(crate) struct CompletedProductionCapabilityCompletionJoinV5 {
    completed: CompletedCompilerCapabilityTransactionV5,
    finalized: PreparedFinalizedProtectedWorkerV3HsacoV1,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
}

impl CompletedProductionCapabilityCompletionJoinV5 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        CompletedCompilerCapabilityTransactionV5,
        PreparedFinalizedProtectedWorkerV3HsacoV1,
        CompilerExecutionReceiptCarriageV1,
    ) {
        (self.completed, self.finalized, self.compiler_execution)
    }
}

impl PendingProductionCapabilityCompletionJoinV5 {
    pub(crate) fn new(
        prepared: PreparedCompilerCapabilityCompletionV5,
        finalized: MachineRefinementPendingFinalizedProtectedWorkerV3HsacoV1,
        compiler_execution: CompilerExecutionReceiptCarriageV1,
    ) -> Result<Self, ProductionCapabilityCompletionJoinErrorV5> {
        if prepared.attempt() != finalized.attempt() {
            return Err(ProductionCapabilityCompletionJoinErrorV5::AttemptMismatch);
        }
        if prepared.object_bytes() != finalized.exact_finalized_bytes() {
            return Err(ProductionCapabilityCompletionJoinErrorV5::ObjectMismatch);
        }
        let handoff = prepared.handoff().legacy_handoff();
        if handoff.canonical_bytes() != finalized.outer_handoff().canonical_bytes() {
            return Err(ProductionCapabilityCompletionJoinErrorV5::HandoffMismatch);
        }
        let subject = compiler_execution.request().subject();
        let subject_handoff = subject.outer_handoff();
        if subject.attempt() != prepared.attempt()
            || subject.compiler_closure() != finalized.compiler_closure()
            || subject_handoff.byte_len() != handoff.canonical_bytes().len() as u64
            || subject_handoff.sha256() != Sha256::digest(handoff.canonical_bytes()).as_slice()
            || compiler_execution.grants_compiler_authority()
            || compiler_execution.grants_load_authority()
            || compiler_execution.grants_launch_authority()
        {
            return Err(ProductionCapabilityCompletionJoinErrorV5::CompilerReceiptMismatch);
        }
        let decoded =
            CompilerExecutionReceiptCarriageV1::decode(compiler_execution.canonical_bytes())
                .map_err(|error| {
                    ProductionCapabilityCompletionJoinErrorV5::InvalidCompilerReceipt(
                        error.to_string(),
                    )
                })?;
        if decoded != compiler_execution {
            return Err(ProductionCapabilityCompletionJoinErrorV5::CompilerReceiptMismatch);
        }
        Ok(Self {
            prepared,
            finalized,
            compiler_execution,
        })
    }

    /// Performs the one-shot canonical V5 exchange while this parent keeps the prepared owner.
    pub(crate) fn exchange_canonical_request<R, Q>(
        self,
        client: WorkerV3VerificationCapabilityClientV5,
        custody: IntakeAuthenticatedWorkerV3CapabilityCustodyV5,
        snapshots: WorkerV3VerificationPayloadSnapshotsV1,
        response_replay: &mut R,
        quarantine: &mut Q,
    ) -> Result<PendingCorrelatedCapabilityResponseV5, ProductionCapabilityExchangeFailureV5>
    where
        R: WorkerV3VerificationResponseReplayGuardV5,
        Q: WorkerV3VerificationClientFailureQuarantineV5,
    {
        let request = custody.request().identity();
        let attempt = custody.request().carriage().attempt();
        match client.exchange(custody, snapshots, response_replay, quarantine) {
            Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Completed(receipt)) => {
                Ok(PendingCorrelatedCapabilityResponseV5 {
                    join: self,
                    receipt,
                })
            }
            Ok(WorkerV3VerificationCapabilityExchangeOutcomeV5::Rejected(_)) => {
                Err(ProductionCapabilityExchangeFailureV5 {
                    join: self,
                    reason: ProductionCapabilityCompletionJoinErrorV5::VerifierRejected,
                })
            }
            Ok(_) => {
                let reason = quarantine_after_terminal_failure_v5(
                    quarantine,
                    request,
                    attempt,
                    ProductionCapabilityCompletionJoinErrorV5::UnexpectedResponse,
                );
                Err(ProductionCapabilityExchangeFailureV5 { join: self, reason })
            }
            Err(error) => Err(ProductionCapabilityExchangeFailureV5 {
                join: self,
                reason: ProductionCapabilityCompletionJoinErrorV5::Client(error.to_string()),
            }),
        }
    }

    /// Selects the protected path only when all move-only production inputs were supplied.
    pub(crate) fn complete_with_available_authority(
        self,
        authority: Option<ProductionCapabilityCompletionExecutorV5>,
    ) -> Result<
        CompletedProductionCapabilityCompletionJoinV5,
        ProductionCapabilityCompletionExecutionErrorV5,
    > {
        match authority {
            Some(authority) => authority.execute(self),
            None => Err(ProductionCapabilityCompletionExecutionErrorV5::Exchange(
                self.into_missing_authenticated_compiler_artifacts_failure(),
            )),
        }
    }

    /// Returns whole custody when the compiler did not carry the inputs required by the exchange.
    pub(crate) fn into_missing_authenticated_compiler_artifacts_failure(
        self,
    ) -> ProductionCapabilityExchangeFailureV5 {
        debug_assert_eq!(self.prepared.attempt(), self.finalized.attempt());
        debug_assert_eq!(
            self.prepared.object_bytes(),
            self.finalized.exact_finalized_bytes()
        );
        debug_assert_eq!(
            self.compiler_execution.request().subject().attempt(),
            self.prepared.attempt()
        );
        self.into_exchange_failure(
            ProductionCapabilityCompletionJoinErrorV5::AuthenticatedCompilerArtifactsUnavailable,
        )
    }

    fn into_exchange_failure(
        self,
        reason: ProductionCapabilityCompletionJoinErrorV5,
    ) -> ProductionCapabilityExchangeFailureV5 {
        ProductionCapabilityExchangeFailureV5 { join: self, reason }
    }
}

/// Correlated endpoint-authenticated response plus the still-live parent completion owner.
#[must_use = "an inert response cannot replace signed artifact-completion authority"]
pub(crate) struct PendingCorrelatedCapabilityResponseV5 {
    join: PendingProductionCapabilityCompletionJoinV5,
    receipt: WorkerV3VerificationCapabilityCompletedReceiptV5,
}

impl PendingCorrelatedCapabilityResponseV5 {
    /// Rechecks the sealed envelope and consumes its signed inner response into artifact authority.
    pub(crate) fn authenticate<Q>(
        self,
        quarantine: &mut Q,
    ) -> Result<
        AuthenticatedPendingProductionCapabilityCompletionV5,
        ProductionCapabilityExchangeFailureV5,
    >
    where
        Q: WorkerV3VerificationClientFailureQuarantineV5,
    {
        let request = self.receipt.request().identity();
        let attempt = self.receipt.request().carriage().attempt();
        let authenticated = match authenticate_completed_receipt_v5(&self.receipt) {
            Ok(authenticated) => authenticated,
            Err(reason) => {
                let reason =
                    quarantine_after_terminal_failure_v5(quarantine, request, attempt, reason);
                return Err(ProductionCapabilityExchangeFailureV5 {
                    join: self.join,
                    reason,
                });
            }
        };
        Ok(AuthenticatedPendingProductionCapabilityCompletionV5 {
            join: self.join,
            authenticated,
            request,
            attempt,
        })
    }
}

/// The parent transaction paired with authority reconstructed from the exact signed response.
#[must_use = "authenticated completion must bind the live W4 owner or be quarantined"]
pub(crate) struct AuthenticatedPendingProductionCapabilityCompletionV5 {
    join: PendingProductionCapabilityCompletionJoinV5,
    authenticated: AuthenticatedCompilerCapabilityCompletionV5,
    request: WorkerV3VerificationCapabilityRequestIdentityV5,
    attempt: WorkerV3VerificationProductionAttemptV5,
}

impl AuthenticatedPendingProductionCapabilityCompletionV5 {
    pub(crate) fn complete<Q>(
        self,
        quarantine: &mut Q,
    ) -> Result<
        CompletedProductionCapabilityCompletionJoinV5,
        ProductionCapabilityCompletionExecutionErrorV5,
    >
    where
        Q: WorkerV3VerificationClientFailureQuarantineV5,
    {
        let Self {
            join,
            authenticated,
            request,
            attempt,
        } = self;
        let PendingProductionCapabilityCompletionJoinV5 {
            prepared,
            finalized,
            compiler_execution,
        } = join;
        let finalized = match finalized
            .complete_with_authenticated_compiler_completion_v1(&authenticated)
        {
            Ok(finalized) => finalized,
            Err(error) => {
                let reason = quarantine_after_terminal_failure_v5(
                    quarantine,
                    request,
                    attempt,
                    ProductionCapabilityCompletionJoinErrorV5::MachineRefinement(error.to_string()),
                );
                drop(prepared);
                drop(compiler_execution);
                return Err(ProductionCapabilityCompletionExecutionErrorV5::Terminal(
                    reason,
                ));
            }
        };
        let w4_witness = AuthenticatedProcessBoundaryW4CustodyV5 {
            canonical: prepared
                .expected_w4_witness()
                .canonical_encoding()
                .to_vec()
                .into_boxed_slice(),
        };
        match complete_with_sealed_artifact_adapter_v5(
            prepared,
            w4_witness,
            authenticated,
            AuthenticatedProcessBoundaryW4CustodyV5::canonical_encoding,
        ) {
            Ok(completed) => Ok(CompletedProductionCapabilityCompletionJoinV5 {
                completed,
                finalized,
                compiler_execution,
            }),
            Err(failure) => {
                let stage = failure.stage();
                let reason = quarantine_after_terminal_failure_v5(
                    quarantine,
                    request,
                    attempt,
                    ProductionCapabilityCompletionJoinErrorV5::SealedCompletion(stage),
                );
                drop(failure);
                drop(finalized);
                drop(compiler_execution);
                Err(ProductionCapabilityCompletionExecutionErrorV5::Terminal(
                    reason,
                ))
            }
        }
    }
}

/// Process-local W4 custody minted only after exact protected-response authentication.
///
/// The private representation prevents callers from converting the inert W4 bytes in a compiler
/// handoff directly into a completion witness. This token denotes the protected verifier's
/// re-admission of those bytes; it does not claim that a child-process Rust object crossed IPC.
struct AuthenticatedProcessBoundaryW4CustodyV5 {
    canonical: Box<[u8]>,
}

impl AuthenticatedProcessBoundaryW4CustodyV5 {
    fn canonical_encoding(&self) -> &[u8] {
        &self.canonical
    }
}

fn authenticate_completed_receipt_v5(
    receipt: &WorkerV3VerificationCapabilityCompletedReceiptV5,
) -> Result<AuthenticatedCompilerCapabilityCompletionV5, ProductionCapabilityCompletionJoinErrorV5>
{
    authenticate_protected_response_v5(
        receipt.protected_response_bytes(),
        receipt.protected_response_identity(),
        receipt.authenticated_response_identity(),
        receipt.request(),
        receipt.response(),
        |response, signature| {
            AuthenticatedCompilerCapabilityCompletionV5::from_signed_verifier_response(
                response, signature,
            )
            .map_err(|error| error.to_string())
        },
        AuthenticatedCompilerCapabilityCompletionV5::signed_response_identity,
    )
}

fn authenticate_protected_response_v5<T, A, I>(
    bytes: &[u8],
    expected_envelope_identity: [u8; 32],
    expected_authenticated_identity: [u8; 32],
    request: &WorkerV3VerificationCapabilityRequestV5,
    response: &WorkerV3VerificationCapabilityResponseV5,
    authenticate: A,
    authenticated_identity: I,
) -> Result<T, ProductionCapabilityCompletionJoinErrorV5>
where
    A: FnOnce(InertCompilerCapabilityVerifierResponseV5, [u8; 64]) -> Result<T, String>,
    I: FnOnce(&T) -> [u8; 32],
{
    let decoded = decode_protected_response_v5(bytes)?;
    if decoded.identity != expected_envelope_identity
        || decoded.fields[0] != PROTECTED_RESPONSE_DOMAIN_V5
        || decoded.fields[1] != request.identity().as_bytes()
        || decoded.fields[2] != request.base_request().challenge().as_bytes()
        || decoded.fields[3] != response.encode_canonical()
    {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMismatch);
    }
    let inert =
        InertCompilerCapabilityVerifierResponseV5::decode(decoded.fields[4]).map_err(|error| {
            ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseAuthentication(
                error.to_string(),
            )
        })?;
    let signature: [u8; 64] = decoded.fields[5]
        .try_into()
        .map_err(|_| ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)?;
    let authenticated = authenticate(inert, signature)
        .map_err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseAuthentication)?;
    if authenticated_identity(&authenticated) != expected_authenticated_identity {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMismatch);
    }
    Ok(authenticated)
}

struct DecodedProtectedResponseV5<'a> {
    fields: [&'a [u8]; PROTECTED_RESPONSE_FIELD_COUNT_V5],
    identity: [u8; 32],
}

fn decode_protected_response_v5(
    bytes: &[u8],
) -> Result<DecodedProtectedResponseV5<'_>, ProductionCapabilityCompletionJoinErrorV5> {
    if bytes.len() < PROTECTED_RESPONSE_HEADER_BYTES_V5 + PROTECTED_RESPONSE_IDENTITY_BYTES_V5
        || bytes.len() > MAX_PROTECTED_RESPONSE_BYTES_V5
        || bytes[..8] != PROTECTED_RESPONSE_MAGIC_V5
        || read_u16(bytes, 8)? != PROTECTED_RESPONSE_VERSION_V5
        || usize::from(read_u16(bytes, 10)?) != PROTECTED_RESPONSE_FIELD_COUNT_V5
        || read_u32(bytes, 12)? != 0
        || usize::try_from(read_u64(bytes, 16)?).ok() != Some(bytes.len())
    {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed);
    }
    let payload_end = bytes.len() - PROTECTED_RESPONSE_IDENTITY_BYTES_V5;
    let mut offset = PROTECTED_RESPONSE_HEADER_BYTES_V5;
    let mut fields = [&[][..]; PROTECTED_RESPONSE_FIELD_COUNT_V5];
    for (index, field) in fields.iter_mut().enumerate() {
        let header_end = offset
            .checked_add(PROTECTED_RESPONSE_FIELD_HEADER_BYTES_V5)
            .ok_or(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)?;
        if header_end > payload_end
            || usize::from(read_u16(bytes, offset)?) != index + 1
            || read_u16(bytes, offset + 2)? != 0
        {
            return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed);
        }
        let length = usize::try_from(read_u32(bytes, offset + 4)?)
            .map_err(|_| ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)?;
        let end = header_end
            .checked_add(length)
            .ok_or(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)?;
        if end > payload_end {
            return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed);
        }
        *field = &bytes[header_end..end];
        offset = end;
    }
    if offset != payload_end {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed);
    }
    let identity: [u8; 32] = bytes[payload_end..]
        .try_into()
        .map_err(|_| ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)?;
    let mut digest = Sha256::new();
    digest.update(PROTECTED_RESPONSE_IDENTITY_DOMAIN_V5);
    digest.update(&bytes[..payload_end]);
    if digest.finalize().as_slice() != identity {
        return Err(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed);
    }
    Ok(DecodedProtectedResponseV5 { fields, identity })
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ProductionCapabilityCompletionJoinErrorV5> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ProductionCapabilityCompletionJoinErrorV5> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ProductionCapabilityCompletionJoinErrorV5> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ProductionCapabilityCompletionJoinErrorV5::ProtectedResponseMalformed)
}

/// Narrow adapter around the artifact crate's final W4-bound completion API.
///
/// The caller must first authenticate a signed verifier response into
/// `AuthenticatedCompilerCapabilityCompletionV5`. Identity-only V5 wire coordinates never enter
/// this adapter.
pub(crate) fn complete_with_sealed_artifact_adapter_v5<W, F>(
    prepared: PreparedCompilerCapabilityCompletionV5,
    w4_witness: W,
    authenticated: AuthenticatedCompilerCapabilityCompletionV5,
    canonical_w4: F,
) -> Result<CompletedCompilerCapabilityTransactionV5, SealedArtifactCompletionFailureV5<W>>
where
    F: for<'a> FnOnce(&'a W) -> &'a [u8],
{
    let bound = prepared
        .bind_w4_witness(w4_witness, canonical_w4)
        .map_err(SealedArtifactCompletionFailureV5::W4Binding)?;
    bound
        .complete_multi_root(authenticated)
        .map_err(SealedArtifactCompletionFailureV5::AuthenticatedCompletion)
}

pub(crate) enum SealedArtifactCompletionFailureV5<W> {
    W4Binding(RecoverableCompilerCapabilityCompletionErrorV5<RejectedW4WitnessBindingCustodyV5<W>>),
    AuthenticatedCompletion(
        RecoverableCompilerCapabilityCompletionErrorV5<
            RejectedAuthenticatedCompilerCapabilityCompletionCustodyV5<W>,
        >,
    ),
}

impl<W> SealedArtifactCompletionFailureV5<W> {
    const fn stage(&self) -> &'static str {
        match self {
            Self::W4Binding(failure) => {
                let _ = failure;
                "live W4 witness binding"
            }
            Self::AuthenticatedCompletion(failure) => {
                let _ = failure;
                "authenticated multi-root completion"
            }
        }
    }
}

/// Converts completed V5 custody into the journal that must precede load readiness.
pub(crate) fn prepare_pending_capability_result_v1(
    completed: CompletedCompilerCapabilityTransactionV5,
    envelope: &WorkerV3LoadEnvelopeV2,
) -> Result<PreparedPendingCapabilityPersistenceV1, String> {
    let ancillary = WorkerV3CapabilityAncillaryEvidenceV1::new(envelope, &completed)
        .map_err(|error| error.to_string())?;
    let (_object, result, _checker_evidence, _checker_evidence_identity) = completed.into_parts();
    let pending = WorkerV3PendingCapabilityResultV1::new(envelope, result)
        .map_err(|error| error.to_string())?;
    Ok(PreparedPendingCapabilityPersistenceV1 { pending, ancillary })
}

pub(crate) struct PreparedPendingCapabilityPersistenceV1 {
    pending: WorkerV3PendingCapabilityResultV1,
    ancillary: WorkerV3CapabilityAncillaryEvidenceV1,
}

impl PreparedPendingCapabilityPersistenceV1 {
    pub(crate) fn persist_before_readiness(
        self,
        directory: &RetainedDurableDirectoryV1,
    ) -> Result<WorkerV3PendingCapabilityResultV1, String> {
        self.ancillary
            .persist_durable_journal_v1(directory)
            .map_err(|error| error.to_string())?;
        self.pending
            .persist_durable_journal_v1(directory)
            .map_err(|error| error.to_string())?;
        Ok(self.pending)
    }
}

/// Failed exchange retaining every prepublication parent owner until caller quarantine begins.
pub(crate) struct ProductionCapabilityExchangeFailureV5 {
    join: PendingProductionCapabilityCompletionJoinV5,
    reason: ProductionCapabilityCompletionJoinErrorV5,
}

/// Failure from the complete Cargo-side V5 exchange/authentication/completion terminal.
#[derive(Debug)]
pub(crate) enum ProductionCapabilityCompletionExecutionErrorV5 {
    Exchange(ProductionCapabilityExchangeFailureV5),
    Terminal(ProductionCapabilityCompletionJoinErrorV5),
}

impl fmt::Display for ProductionCapabilityCompletionExecutionErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exchange(error) => error.fmt(formatter),
            Self::Terminal(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionCapabilityCompletionExecutionErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Exchange(error) => Some(error),
            Self::Terminal(error) => Some(error),
        }
    }
}

fn quarantine_after_terminal_failure_v5<Q>(
    quarantine: &mut Q,
    request: WorkerV3VerificationCapabilityRequestIdentityV5,
    attempt: WorkerV3VerificationProductionAttemptV5,
    reason: ProductionCapabilityCompletionJoinErrorV5,
) -> ProductionCapabilityCompletionJoinErrorV5
where
    Q: WorkerV3VerificationClientFailureQuarantineV5,
{
    match quarantine.quarantine_failed_exchange(request, attempt) {
        Ok(()) => reason,
        Err(error) => ProductionCapabilityCompletionJoinErrorV5::ClientQuarantine {
            terminal: reason.to_string(),
            quarantine: error.to_string(),
        },
    }
}

impl fmt::Display for ProductionCapabilityExchangeFailureV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = self.join.prepared.attempt();
        self.reason.fmt(formatter)
    }
}

impl fmt::Debug for ProductionCapabilityExchangeFailureV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionCapabilityExchangeFailureV5")
            .field("reason", &self.reason)
            .field("parent_custody_retained", &true)
            .finish_non_exhaustive()
    }
}

impl Error for ProductionCapabilityExchangeFailureV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.reason)
    }
}

#[derive(Debug)]
pub(crate) enum ProductionCapabilityCompletionJoinErrorV5 {
    AttemptMismatch,
    ObjectMismatch,
    HandoffMismatch,
    CompilerReceiptMismatch,
    InvalidCompilerReceipt(String),
    ProtectedCompilerInput(String),
    ChallengeEntropy(String),
    Payload(String),
    CanonicalRequest(String),
    Client(String),
    VerifierRejected,
    UnexpectedResponse,
    AuthenticatedCompilerArtifactsUnavailable,
    MachineRefinement(String),
    SealedCompletion(&'static str),
    ClientQuarantine {
        terminal: String,
        quarantine: String,
    },
    ProtectedResponseMalformed,
    ProtectedResponseMismatch,
    ProtectedResponseAuthentication(String),
    ClientProfileChanged(String),
}

impl fmt::Display for ProductionCapabilityCompletionJoinErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttemptMismatch => formatter
                .write_str("V5 transaction and finalized HSACO name different build attempts"),
            Self::ObjectMismatch => formatter
                .write_str("V5 transaction object bytes differ from the finalized HSACO"),
            Self::HandoffMismatch => formatter
                .write_str("V5 transaction and finalized HSACO retain different compiler handoffs"),
            Self::CompilerReceiptMismatch => formatter.write_str(
                "V5 transaction, finalized HSACO, and compiler receipt do not share one exact subject",
            ),
            Self::InvalidCompilerReceipt(error) => {
                write!(formatter, "exact compiler receipt is not canonical: {error}")
            }
            Self::ProtectedCompilerInput(error) => {
                write!(formatter, "protected compiler input package is invalid: {error}")
            }
            Self::ChallengeEntropy(error) => {
                write!(formatter, "V5 verifier challenge entropy failed: {error}")
            }
            Self::Payload(error) => {
                write!(formatter, "V5 immutable payload construction failed: {error}")
            }
            Self::CanonicalRequest(error) => {
                write!(formatter, "canonical V5 verifier request construction failed: {error}")
            }
            Self::Client(error) => write!(formatter, "fixed protected V5 exchange failed: {error}"),
            Self::VerifierRejected => {
                formatter.write_str("fixed protected V5 verifier rejected the exact request")
            }
            Self::UnexpectedResponse => formatter
                .write_str("fixed protected V5 verifier returned an unsupported disposition"),
            Self::AuthenticatedCompilerArtifactsUnavailable => write!(
                formatter,
                "native V5 publication is denied before contacting {PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1}: protected rustc did not persist the exact attempt-bound #213 proof owner, association roster, generated-host roster, and #214 machine-refinement package"
            ),
            Self::MachineRefinement(error) => {
                write!(formatter, "authenticated machine refinement failed: {error}")
            }
            Self::SealedCompletion(stage) => {
                write!(formatter, "native V5 {stage} rejected exact authenticated custody")
            }
            Self::ClientQuarantine {
                terminal,
                quarantine,
            } => write!(
                formatter,
                "native V5 terminal failed ({terminal}) and durable client quarantine also failed: {quarantine}"
            ),
            Self::ProtectedResponseMalformed => formatter
                .write_str("sealed V5 protected response is not canonical or exceeds its bound"),
            Self::ProtectedResponseMismatch => formatter.write_str(
                "sealed V5 protected response does not match the exact request, response, or authenticated identities",
            ),
            Self::ProtectedResponseAuthentication(error) => {
                write!(formatter, "sealed V5 protected response authentication failed: {error}")
            }
            Self::ClientProfileChanged(error) => {
                write!(formatter, "protected V5 verifier profile changed: {error}")
            }
        }
    }
}

impl Error for ProductionCapabilityCompletionJoinErrorV5 {}
