//! Real fixed-path admission with explicitly test-key responses, not deployed issuer qualification.
use super::*;
use fe2o3_compiler_execution_client::COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProfileV1, CompilerExecutionExternalAnchorDeploymentV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionSupervisorDeploymentV1,
};
use std::fs;
use std::os::fd::IntoRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Instant;

#[test]
#[ignore = "writes test-key public profiles for an isolated real-root qualification namespace"]
fn write_configuration() {
    let fixture = Fixture::new(0x20);
    let service = CompilerExecutionExternalAnchorServiceIdentityV1::new(61_003, 61_004).unwrap();
    let client =
        CompilerExecutionClientProfileV1::new(61_001, 61_002, service, fixture.policy.clone())
            .unwrap();
    let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
        61_001,
        61_002,
        service,
        CompilerExecutionIssuerMeasurementV1::new([0x31; 32], 1024).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([0x32; 32], 2048).unwrap(),
        &fixture.policy,
    )
    .unwrap();
    let anchor = CompilerExecutionExternalAnchorDeploymentV1::new(
        &supervisor,
        &fixture.policy,
        CompilerExecutionIssuerMeasurementV1::new([0x33; 32], 4096).unwrap(),
    )
    .unwrap();
    let directory =
        PathBuf::from(std::env::var_os("FE2O3_PRODUCTION_DEPLOYMENT_FIXTURE_OUT").unwrap());
    for (name, bytes) in [
        ("client-profile-v1", client.canonical_bytes().as_slice()),
        (
            "supervisor-deployment-v1",
            supervisor.canonical_bytes().as_slice(),
        ),
        ("anchor-deployment-v1", anchor.canonical_bytes().as_slice()),
    ] {
        let file = directory.join(name);
        fs::write(&file, bytes).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o444)).unwrap();
    }
}

#[test]
#[ignore = "requires a private real-root deployment namespace and bounded replacement helper"]
fn installed_production_audit() {
    let case = std::env::var("FE2O3_PRODUCTION_AUDIT_CASE").unwrap();
    let fixture = if case == "policy" {
        Fixture::with_signing_seed(0x20, 0x71)
    } else {
        Fixture::new(0x20)
    };
    let service = install_endpoint();
    let admitted =
        InheritedWorkerV3CompilerCurrentRecordAuditorV1::admit_production_application_service();
    if case == "reject" {
        assert!(matches!(
            admitted,
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::ProductionDeployment(_))
        ));
        assert_closed_without_request(&service);
        return;
    }
    let mut auditor = admitted.unwrap();
    if case == "pre" {
        replace_configuration();
    }
    if case == "pre" || case == "policy" {
        let result = auditor.audit_exact(&fixture.subject, &fixture.carriage);
        if case == "pre" {
            assert!(matches!(
                result,
                Err(WorkerV3CompilerCurrentRecordAuditErrorV1::ProductionDeployment(_))
            ));
        } else {
            assert!(matches!(
                result,
                Err(WorkerV3CompilerCurrentRecordAuditErrorV1::PolicyMismatch)
            ));
        }
        assert_closed_without_request(&service);
        assert_consumed(&mut auditor, &fixture);
        return;
    }
    let service_case = case.clone();
    let server = thread::spawn(move || {
        poll_ready(&service);
        let request = receive_request(&service);
        let fixture = Fixture::new(0x20);
        assert_eq!(request.carriage(), Some(&fixture.carriage));
        let challenge = request.verification_challenge().unwrap();
        assert_ne!(challenge, [0; 32]);
        if service_case == "supplied" {
            assert_eq!(challenge, [0xa1; 32]);
        }
        if matches!(service_case.as_str(), "stale_success" | "stale_failure") {
            replace_configuration();
        }
        if matches!(service_case.as_str(), "service_failure" | "stale_failure") {
            return;
        }
        let mut attestation = direct_current_record_attestation(&fixture, challenge);
        if service_case == "wrong_signer" {
            let wrong = Fixture::with_signing_seed(0x20, 0x71);
            let mut wire = attestation.canonical_bytes().to_vec();
            wire[CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES - 32
                ..CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES]
                .copy_from_slice(&wrong.signing_key.verifying_key().to_bytes());
            let wire = rebuild_current_record_attestation(
                &wrong,
                wire,
                challenge,
                attestation.verification().canonical_bytes(),
            );
            attestation = CompilerExecutionCurrentRecordAttestationV3::decode(&wire).unwrap();
        }
        let response =
            CompilerExecutionServiceResponseV1::verified_current(request.identity(), attestation)
                .unwrap();
        // SAFETY: reads exactly the borrowed response; bounded nonblocking send cannot SIGPIPE.
        assert_eq!(
            unsafe {
                libc::send(
                    service.as_raw_fd(),
                    response.canonical_bytes().as_ptr().cast(),
                    response.canonical_bytes().len(),
                    libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
                )
            },
            response.canonical_bytes().len() as isize
        );
    });
    let result = if case == "supplied" {
        auditor.audit_exact_with_challenge(
            &fixture.subject,
            &fixture.carriage,
            CompilerExecutionCurrentRecordChallengeV1::from_bytes([0xa1; 32]).unwrap(),
        )
    } else {
        auditor.audit_exact(&fixture.subject, &fixture.carriage)
    };
    server.join().unwrap();
    assert_consumed(&mut auditor, &fixture);
    drop(auditor);
    match case.as_str() {
        "stale_success" | "stale_failure" => assert!(matches!(
            result,
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::ProductionDeployment(_))
        )),
        "service_failure" | "wrong_signer" => assert!(matches!(
            result,
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::Client(_))
        )),
        "late_binding" => {
            let audit = result.unwrap();
            audit.revalidate_production_deployment().unwrap();
            replace_configuration();
            assert!(matches!(
                audit.bind_exact_compiler_execution_v1(&fixture.subject, &fixture.carriage),
                Err(WorkerV3CompilerExecutionEvidenceErrorV1::ProductionDeployment(_))
            ));
        }
        "good" | "supplied" | "retained" => {
            let audit = result.unwrap();
            audit.revalidate_production_deployment().unwrap();
            assert!(!audit.grants_authority());
            let bound = audit
                .bind_exact_compiler_execution_v1(&fixture.subject, &fixture.carriage)
                .unwrap();
            bound.revalidate_production_deployment().unwrap();
            assert!(!bound.grants_verification_authority());
            if case == "retained" {
                replace_configuration();
                assert!(matches!(
                    bound.revalidate_production_deployment(),
                    Err(WorkerV3CompilerCurrentRecordAuditErrorV1::ProductionDeployment(_))
                ));
            }
        }
        _ => panic!("unknown production audit qualification case"),
    }
    eprintln!("production audit {case}: expected result; test-key response, no launch authority");
}

fn install_endpoint() -> OwnedFd {
    let (client, service) = socket_pair();
    // SAFETY: F_DUPFD allocates a new slot without clobbering an existing inherited endpoint.
    let fd = unsafe {
        libc::fcntl(
            client.as_raw_fd(),
            libc::F_DUPFD,
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
        )
    };
    assert!(fd >= 0);
    // SAFETY: successful F_DUPFD transfers a new descriptor to this test.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    assert_eq!(fd.as_raw_fd(), COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
    drop(client);
    let _ = fd.into_raw_fd();
    service
}
fn poll_ready(service: &OwnedFd) {
    let mut event = libc::pollfd {
        fd: service.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll borrows one initialized event for a bounded wait.
    assert_eq!(unsafe { libc::poll(&mut event, 1, 10_000) }, 1);
}
fn assert_closed_without_request(service: &OwnedFd) {
    poll_ready(service);
    let mut byte = 0u8;
    // SAFETY: one writable byte, nonblocking. A sent request would return positive length.
    assert_eq!(
        unsafe {
            libc::recv(
                service.as_raw_fd(),
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        },
        0
    );
}
fn assert_consumed(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    fixture: &Fixture,
) {
    assert!(matches!(
        auditor.audit_exact(&fixture.subject, &fixture.carriage),
        Err(WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed)
    ));
}
fn replace_configuration() {
    let directory = PathBuf::from(std::env::var_os("FE2O3_PRODUCTION_DEPLOYMENT_CONTROL").unwrap());
    fs::write(directory.join("replace-request"), []).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !directory.join("replaced").exists() {
        assert!(
            Instant::now() < deadline,
            "root-owned qualification replacement did not arrive"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
