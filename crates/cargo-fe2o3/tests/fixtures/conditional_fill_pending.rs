//! Genuine publication and executed refinement joined to a test-key service, never launch authority.
use super::conditional_fill_host::{AuditOnlyFillMarker, FILL_BINDING, GeneratedFillMarker};
use super::*;
use ed25519_dalek::Signer;
use fe2o3_compiler_execution_client::COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;
use fe2o3_external_anchor_protocol::{
    AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1,
    PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use fe2o3_host::{
    InheritedWorkerV3CompilerCurrentRecordAuditorV1, PendingWorkerV3ConditionalFillArtifactV1,
    WorkerV3CompilerCurrentRecordAuditErrorV1, WorkerV3ConditionalFillAssociationErrorV1,
    WorkerV3ConditionalFillPendingErrorV1, WorkerV3ConditionalFillRetainedErrorV1,
    WorkerV3VerificationChallengeIdentityV1, execute_retained_worker_v3_conditional_fill_v1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_runtime_protocol::{
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationV3,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionServiceRequestKindV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1, OwnedConditionalFillRefinementExecutionV1,
    check_conditional_fill_program_v1, execute_owned_conditional_fill_refinement_v1,
    validate_conditional_compiler_proof_inputs_v1, validate_conditional_compiler_target_lineage_v1,
};
use object::{Object, ObjectSection};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::time::Duration;

struct RefinementAuditor {
    worker: AuthenticatedPhysicalMachineEffectWorkerV1,
    limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    runtime: FunctionalRefinementVerusRuntimeLeaseV1,
    change_payload: bool,
    producer_controls: bool,
}

impl WorkerV3AuditorV1<GeneratedFillMarker> for RefinementAuditor {
    type Error = Infallible;
    type Evidence = (
        OwnedConditionalFillRefinementExecutionV1,
        CompilerExecutionReceiptCarriageV1,
        WorkerV3VerificationChallengeIdentityV1,
        Option<fe2o3_host::InertWorkerV3ConditionalFillSubjectV1>,
    );

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, GeneratedFillMarker>,
    ) -> Result<Self::Evidence, Self::Error> {
        let closure = super::compiler_closure::check_request(request);
        if !self.change_payload {
            if self.producer_controls {
                check_producer_rejections(self, request);
            }
            let envelope = request
                .load_envelope_evidence_view()
                .exact_canonical_bytes()
                .to_vec()
                .into_boxed_slice();
            let payload = request.finalized_hsaco_bytes().to_vec().into_boxed_slice();
            let envelope_pointer = envelope.as_ptr();
            let payload_pointer = payload.as_ptr();
            let retained = execute_retained_worker_v3_conditional_fill_v1(
                envelope,
                payload,
                request.descriptor().kernel_id(),
                &self.worker,
                &self.runtime,
                std::time::Instant::now() + Duration::from_secs(300),
            )
            .unwrap();
            assert_eq!(
                retained.exact_canonical_envelope_bytes().as_ptr(),
                envelope_pointer
            );
            assert_eq!(retained.finalized_hsaco_bytes().as_ptr(), payload_pointer);
            assert_eq!(retained.kernel_id(), request.descriptor().kernel_id());
            assert!(!retained.authenticates_compiler_origin());
            assert!(!retained.grants_currentness_authority());
            assert!(!retained.grants_load_authority());
            assert!(!retained.grants_launch_authority());
            let subject = retained.subject().clone();
            assert_eq!(
                subject,
                closure
                    .check_conditional_fill_refinement_v1(retained.refinement())
                    .unwrap()
            );
            let source_pointer = retained.refinement().generated_source().as_ptr();
            let analysis_pointer = retained
                .refinement()
                .analysis_execution()
                .canonical_receipt_bytes()
                .as_ptr();
            let refinement = retained.into_refinement();
            assert_eq!(source_pointer, refinement.generated_source().as_ptr());
            assert_eq!(
                analysis_pointer,
                refinement
                    .analysis_execution()
                    .canonical_receipt_bytes()
                    .as_ptr()
            );
            return Ok((
                refinement,
                request.compiler_execution_receipt_carriage().clone(),
                request.challenge_identity(),
                Some(subject),
            ));
        }
        // Keep the genuinely proved wrong-payload case at the later Pending boundary.
        let receipts = request.semantic_compiler_handoff().capsule().receipts();
        let inputs = validate_conditional_compiler_proof_inputs_v1(
            receipts.proof_binding(),
            receipts.semantic_mir(),
            receipts.middle_end(),
            receipts.kernel_ir(),
            receipts.mir_to_kir_correspondence(),
            receipts.formal_memory(),
        )
        .unwrap();
        let lineage = validate_conditional_compiler_target_lineage_v1(
            request.semantic_compiler_handoff().capsule(),
            &inputs,
        )
        .unwrap();
        let program = check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
        let mut payload = request.finalized_hsaco_bytes().to_vec();
        mutate_comment(&mut payload);
        assert!(
            fe2o3_host::check_worker_v3_compiler_closure_v1(
                request
                    .load_envelope_evidence_view()
                    .exact_canonical_bytes(),
                &payload,
                request.descriptor().kernel_id(),
            )
            .is_err()
        );
        let analysis = self
            .worker
            .analyze(
                payload,
                vec![
                    PhysicalMachineEffectEntryRequestV1::new(
                        program.function_symbol(),
                        PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
                    )
                    .unwrap(),
                ],
                self.limits,
            )
            .unwrap();
        let refinement = execute_owned_conditional_fill_refinement_v1(
            &self.runtime,
            inputs,
            lineage,
            analysis,
            180,
        )
        .unwrap();
        let independent_subject = closure.check_conditional_fill_refinement_v1(&refinement);
        assert!(matches!(
            independent_subject,
            Err(WorkerV3ConditionalFillPendingErrorV1::Association(
                WorkerV3ConditionalFillAssociationErrorV1::Machine("finalized payload")
            ))
        ));
        Ok((
            refinement,
            request.compiler_execution_receipt_carriage().clone(),
            request.challenge_identity(),
            None,
        ))
    }
}

fn mutate_comment(payload: &mut [u8]) {
    let (start, len) = object::File::parse(&*payload)
        .unwrap()
        .section_by_name(".comment")
        .unwrap()
        .file_range()
        .unwrap();
    let byte = payload[start as usize..(start + len) as usize]
        .iter_mut()
        .find(|byte| **byte != 0)
        .unwrap();
    *byte ^= 1;
}

fn check_producer_rejections(
    auditor: &RefinementAuditor,
    request: &WorkerV3VerificationRequestV1<'_, GeneratedFillMarker>,
) {
    use WorkerV3ConditionalFillRetainedErrorV1 as E;
    use fe2o3_runtime_protocol::WorkerV3LoadEnvelopeErrorV2;
    let canonical = request
        .load_envelope_evidence_view()
        .exact_canonical_bytes();
    let payload = request.finalized_hsaco_bytes();
    let kernel = request.descriptor().kernel_id();
    let reject = |envelope: Vec<u8>, hsaco: Vec<u8>, selected, deadline| {
        execute_retained_worker_v3_conditional_fill_v1(
            envelope.into_boxed_slice(),
            hsaco.into_boxed_slice(),
            selected,
            &auditor.worker,
            &auditor.runtime,
            deadline,
        )
        .unwrap_err()
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut corrupt = canonical.to_vec();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(matches!(
        reject(corrupt, payload.to_vec(), kernel, deadline),
        E::Closure(RecoveredWorkerV3AdmissionErrorV1::Envelope(
            WorkerV3LoadEnvelopeErrorV2::ChecksumMismatch
        ))
    ));
    assert!(matches!(
        reject(
            canonical.to_vec(),
            payload[..payload.len() - 1].to_vec(),
            kernel,
            deadline
        ),
        E::Closure(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch)
    ));
    let mut changed = payload.to_vec();
    changed.push(0);
    assert!(matches!(
        reject(canonical.to_vec(), changed, kernel, deadline),
        E::Closure(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch)
    ));
    let mut changed = payload.to_vec();
    mutate_comment(&mut changed);
    assert!(matches!(
        reject(canonical.to_vec(), changed, kernel, deadline),
        E::Closure(RecoveredWorkerV3AdmissionErrorV1::FinalizerDerivation(
            fe2o3_hsaco_finalize::WorkerV3HsacoPublicationErrorV1::FinalizedHsaco(
                fe2o3_hsaco_finalize::FinalizationError::CanonicalDigestMismatch { .. }
            )
        ))
    ));
    assert!(matches!(
        reject(
            canonical.to_vec(),
            payload.to_vec(),
            KernelId::from_bytes([0x37; 32]),
            deadline
        ),
        E::Closure(RecoveredWorkerV3AdmissionErrorV1::KernelNotFound)
    ));
    assert!(matches!(
        reject(
            canonical.to_vec(),
            payload.to_vec(),
            kernel,
            std::time::Instant::now()
        ),
        E::Deadline
    ));
    assert!(matches!(
        reject(Vec::new(), payload.to_vec(), kernel, deadline),
        E::InputSize {
            field: "envelope",
            ..
        }
    ));
    assert!(matches!(
        reject(canonical.to_vec(), Vec::new(), kernel, deadline),
        E::InputSize {
            field: "finalized HSACO",
            ..
        }
    ));
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Good,
    Marker,
    HostContract,
    Payload,
    StaleSuccess,
    StaleFailure,
    ServiceFailure,
}

#[test]
#[ignore = "requires native Worker and protected Verus; test-key compiler service, no GPU"]
fn genuine_fill_pending_retains_original_evidence_and_currentness() {
    for case in [
        Case::Good,
        Case::Marker,
        Case::HostContract,
        Case::Payload,
        Case::StaleSuccess,
        Case::StaleFailure,
        Case::ServiceFailure,
    ] {
        run_case(case);
    }
}

fn run_case(case: Case) {
    let fixture = worker_v3_fixture::published_genuine_conditional_fill_fixture();
    let private_worker = fixture.directory.0.join("real-worker");
    let (directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let admission =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes(FILL_BINDING))
            .unwrap();
    let expected_lineage = admission.lineage_identity();
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        std::time::Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(&private_worker, limits).unwrap();
    let mut proof_auditor = RefinementAuditor {
        worker: AuthenticatedPhysicalMachineEffectWorkerV1::open(
            &private_worker,
            candidate.policy(),
            limits,
        )
        .unwrap(),
        limits,
        runtime: FunctionalRefinementVerusRuntimeLeaseV1::open(
            std::env::var_os("FE2O3_FUNCTIONAL_REFINEMENT_TEST_RUNTIME_ROOT")
                .expect("protected pinned proof runtime"),
        )
        .unwrap(),
        change_payload: matches!(case, Case::Payload),
        producer_controls: matches!(case, Case::Good),
    };
    let (refinement, carriage, host_challenge, independent_subject) =
        audit_recovered_worker_v3_verification_v1::<GeneratedFillMarker, _>(
            &admission,
            &mut proof_auditor,
        )
        .unwrap();
    drop(proof_auditor);
    let source_pointer = refinement.generated_source().as_ptr();
    let analysis_pointer = refinement
        .analysis_execution()
        .canonical_receipt_bytes()
        .as_ptr();
    let proof_binding = refinement.binding();
    let mut expected_subject = b"FE2O3/WORKER-V3-CONDITIONAL-FILL-SUBJECT/V1\0".to_vec();
    expected_subject.extend_from_slice(expected_lineage.as_bytes());
    expected_subject.extend_from_slice(host_challenge.as_bytes());
    expected_subject.push(refinement.boundary() as u8);
    for bytes in [
        refinement.obligation_preimage(),
        refinement.signed_receipt_wire(),
    ] {
        expected_subject.extend_from_slice(&Sha256::digest(bytes));
        expected_subject.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    }
    expected_subject.extend_from_slice(refinement.receipt_verifying_key());
    if let Some(subject) = &independent_subject {
        assert_eq!(subject.canonical_bytes(), expected_subject);
        assert!(!subject.grants_load_authority());
        assert!(!subject.grants_launch_authority());
        capture(case, "independent-subject.bin", subject.canonical_bytes());
    }
    capture(case, "proof.rs", refinement.generated_source());
    capture(case, "obligation.bin", refinement.obligation_preimage());
    capture(case, "proof.receipt", refinement.signed_receipt_wire());
    capture(case, "proof.key", refinement.receipt_verifying_key());
    capture(
        case,
        "analysis.request",
        refinement.analysis_execution().request().canonical_bytes(),
    );
    capture(
        case,
        "analysis.bundle",
        refinement.analysis_execution().analysis().canonical_bytes(),
    );
    capture(
        case,
        "analysis.receipt",
        refinement.analysis_execution().canonical_receipt_bytes(),
    );

    // The inherited client has a 30-second deadline, so admit it only after proving.
    let (mut auditor, service) = inherited_service();
    if matches!(case, Case::Marker) {
        let error = PendingWorkerV3ConditionalFillArtifactV1::<WorkerV3VecAddMarker>::check(
            admission,
            refinement,
            &mut auditor,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3ConditionalFillPendingErrorV1::Marker(_)
        ));
        assert_endpoint_unused(&service);
        drop(auditor);
    } else if matches!(case, Case::HostContract) {
        let error = PendingWorkerV3ConditionalFillArtifactV1::<AuditOnlyFillMarker>::check(
            admission,
            refinement,
            &mut auditor,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3ConditionalFillPendingErrorV1::Marker("generated host contract")
        ));
        assert_endpoint_unused(&service);
        drop(auditor);
    } else if matches!(case, Case::Payload) {
        let error = PendingWorkerV3ConditionalFillArtifactV1::<GeneratedFillMarker>::check(
            admission,
            refinement,
            &mut auditor,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WorkerV3ConditionalFillPendingErrorV1::Association(
                WorkerV3ConditionalFillAssociationErrorV1::Machine("finalized payload")
            )
        ));
        assert_endpoint_unused(&service);
        drop(auditor);
    } else {
        let root = directory.0.clone();
        let moved = root.with_extension("pending-fill-stale");
        let replace = matches!(case, Case::StaleSuccess | Case::StaleFailure);
        let service_root = root.clone();
        let service_moved = moved.clone();
        let server = std::thread::spawn(move || {
            let request = receive_request(&service);
            assert_eq!(request.carriage(), Some(&carriage));
            assert_eq!(
                request.kind(),
                CompilerExecutionServiceRequestKindV1::VerifyCurrent
            );
            assert_ne!(request.verification_challenge().unwrap(), [0; 32]);
            capture(case, "service.request", request.canonical_bytes());
            if replace {
                fs::rename(&service_root, &service_moved).unwrap();
                fs::create_dir(&service_root).unwrap();
            }
            if matches!(case, Case::StaleFailure | Case::ServiceFailure) {
                return None;
            }
            let response = signed_response(&request);
            capture(case, "service.response", response.canonical_bytes());
            send_response(&service, response.canonical_bytes());
            request.verification_challenge()
        });
        let result = PendingWorkerV3ConditionalFillArtifactV1::<GeneratedFillMarker>::check(
            admission,
            refinement,
            &mut auditor,
        );
        let service_result = server.join();
        if replace {
            fs::remove_dir(&root).unwrap();
            fs::rename(&moved, &root).unwrap();
        }
        let service_challenge = service_result.unwrap();
        if replace {
            assert!(matches!(
                result,
                Err(WorkerV3ConditionalFillPendingErrorV1::CurrentPublication(_))
            ));
        } else if matches!(case, Case::ServiceFailure) {
            assert!(matches!(
                result,
                Err(WorkerV3ConditionalFillPendingErrorV1::CurrentRecord(
                    WorkerV3CompilerCurrentRecordAuditErrorV1::Client(_)
                ))
            ));
        } else {
            let pending = result.unwrap();
            assert_eq!(Some(pending.subject()), independent_subject.as_ref());
            assert_eq!(pending.subject().canonical_bytes(), expected_subject);
            assert_eq!(
                pending.subject().identity(),
                fe2o3_hsaco_finalize::ContentIdentityV1::calculate(&expected_subject)
            );
            assert_eq!(pending.subject(), &pending.subject().clone());
            assert!(!pending.subject().grants_load_authority());
            assert!(!pending.subject().grants_launch_authority());
            capture(case, "subject.bin", pending.subject().canonical_bytes());
            assert_eq!(pending.lineage_identity(), expected_lineage);
            assert_eq!(
                pending.refinement().generated_source().as_ptr(),
                source_pointer
            );
            assert_eq!(
                pending
                    .refinement()
                    .analysis_execution()
                    .canonical_receipt_bytes()
                    .as_ptr(),
                analysis_pointer
            );
            assert_eq!(pending.refinement().binding(), proof_binding);
            assert_eq!(
                pending.descriptor().kernel_id(),
                KernelId::from_bytes(FILL_BINDING)
            );
            let compiler_evidence = pending
                .compiler_execution()
                .current_record_evidence_view()
                .unwrap();
            assert_eq!(
                Some(compiler_evidence.verification_challenge()),
                service_challenge
            );
            assert!(!compiler_evidence.grants_authority());
            capture(
                case,
                "current.verification",
                compiler_evidence.verification_canonical_bytes(),
            );
            capture(
                case,
                "current.attestation",
                compiler_evidence.attestation_canonical_bytes(),
            );
            assert!(
                pending
                    .refinement()
                    .retains_strictly_imported_signed_receipt()
            );
            assert!(!pending.authenticates_protected_compiler_origin());
            assert!(!pending.authenticates_verification_authority());
            assert!(!pending.grants_load_authority());
            assert!(!pending.grants_launch_authority());
            pending.revalidate_currentness().unwrap();
            fs::rename(&root, &moved).unwrap();
            fs::create_dir(&root).unwrap();
            let stale = pending.revalidate_currentness();
            assert_eq!(pending.subject().canonical_bytes(), expected_subject);
            fs::remove_dir(&root).unwrap();
            fs::rename(&moved, &root).unwrap();
            assert!(stale.is_err());
            pending.revalidate_currentness().unwrap();
            assert_eq!(pending.subject().canonical_bytes(), expected_subject);
        }
        let (_foreign_directory, recovered) = recovered_host_fixture();
        let foreign = admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes(TEST_MARKER_BINDING),
        )
        .unwrap();
        assert!(matches!(
            audit_recovered_worker_v3_verification_v1::<WorkerV3VecAddMarker, _>(
                &foreign,
                &mut auditor,
            ),
            Err(fe2o3_host::WorkerV3VerificationAuditErrorV1::Auditor(
                WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed
            ))
        ));
    }
    eprintln!("pending fill {case:?}: expected ownership/currentness outcome; no launch authority");
}

fn inherited_service() -> (InheritedWorkerV3CompilerCurrentRecordAuditorV1, OwnedFd) {
    let mut fds = [-1; 2];
    // SAFETY: the writable array has exactly the two slots required by socketpair.
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        },
        0
    );
    // SAFETY: successful socketpair returned distinct newly owned descriptors.
    let (client, service) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    // F_DUPFD allocates without replacing any existing FD195. Do not clobber an inherited service.
    let inherited = unsafe {
        libc::fcntl(
            client.as_raw_fd(),
            libc::F_DUPFD,
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
        )
    };
    assert!(inherited >= 0);
    // SAFETY: successful F_DUPFD created a new descriptor owned by this test.
    let inherited = unsafe { OwnedFd::from_raw_fd(inherited) };
    assert_eq!(
        inherited.as_raw_fd(),
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    );
    drop(client);
    let _ = inherited.into_raw_fd();
    let auditor =
        InheritedWorkerV3CompilerCurrentRecordAuditorV1::admit_inherited_application_service()
            .unwrap();
    (auditor, service)
}

fn assert_endpoint_unused(service: &OwnedFd) {
    let mut descriptor = libc::pollfd {
        fd: service.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll borrows one initialized descriptor record for an immediate observation.
    assert_eq!(
        unsafe { libc::poll(&mut descriptor, 1, 0) },
        0,
        "local rejection must leave the connection open with no request"
    );
}

fn receive_request(service: &OwnedFd) -> CompilerExecutionServiceRequestV1 {
    let mut descriptor = libc::pollfd {
        fd: service.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll borrows one initialized descriptor record for a bounded wait.
    assert_eq!(unsafe { libc::poll(&mut descriptor, 1, 30_000) }, 1);
    let mut bytes = vec![0; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
    // SAFETY: recv writes only within the initialized buffer and borrows the service fd.
    let received = unsafe {
        libc::recv(
            service.as_raw_fd(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            libc::MSG_DONTWAIT | libc::MSG_TRUNC,
        )
    };
    assert!(received > 0 && received as usize <= bytes.len());
    CompilerExecutionServiceRequestV1::decode(&bytes[..received as usize]).unwrap()
}

fn send_response(service: &OwnedFd, bytes: &[u8]) {
    // SAFETY: send reads the borrowed response slice and cannot deliver SIGPIPE.
    assert_eq!(
        unsafe {
            libc::send(
                service.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
            )
        },
        bytes.len() as isize
    );
}

fn signed_response(
    request: &CompilerExecutionServiceRequestV1,
) -> CompilerExecutionServiceResponseV1 {
    let carriage = request.carriage().unwrap();
    let signing_key = SigningKey::from_bytes(&[0x71; 32]);
    let anchor_signing_key = SigningKey::from_bytes(&[0x72; 32]);
    let key = PinnedAnchorKeyV1::from_bytes(anchor_signing_key.verifying_key().to_bytes()).unwrap();
    let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
        carriage.policy().clone(),
        carriage.request().clone(),
        carriage.publication().clone(),
    )
    .unwrap();
    let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(transaction.external_anchor_digest(), &key)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x73; 32]), &key)
        .unwrap();
    let unsigned = UnsignedAnchorObservationV1::from_challenge(
        pending.challenge(),
        AnchorPositionV1::Proposed,
    );
    let signature = anchor_signing_key.sign(&unsigned.signing_bytes());
    let commit = AnchorTransitionReceiptV1::new(
        pending.challenge().clone(),
        &unsigned.attach_signature(signature.to_bytes()),
        &key,
    )
    .unwrap();
    let challenge = request.verification_challenge().unwrap();
    let currentness =
        CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
            carriage, &commit, challenge,
        )
        .unwrap();
    let unsigned =
        UnsignedAnchorObservationV1::from_challenge(&currentness, AnchorPositionV1::Proposed);
    let signature = anchor_signing_key.sign(&unsigned.signing_bytes());
    let currentness = AnchorTransitionReceiptV1::new(
        currentness,
        &unsigned.attach_signature(signature.to_bytes()),
        &key,
    )
    .unwrap();
    let verification = CompilerExecutionCurrentRecordVerificationV3::new(
        carriage,
        commit,
        currentness,
        challenge,
        [0x74; 32],
        [0x75; 32],
    )
    .unwrap();
    let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
        carriage.policy(),
        carriage,
        verification,
        challenge,
        &signing_key,
    )
    .unwrap();
    CompilerExecutionServiceResponseV1::verified_current(request.identity(), attestation).unwrap()
}

fn capture(case: Case, name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("FE2O3_PENDING_FILL_CAPTURE") {
        let directory = Path::new(&directory).join(format!("{case:?}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(name), bytes).unwrap();
    }
}
