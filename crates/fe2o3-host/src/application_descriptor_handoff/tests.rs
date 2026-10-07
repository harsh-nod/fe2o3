use super::*;
use crate::eof_test_process::run as isolated_eof_case;

#[test]
fn production_consumer_rejects_three_slots_without_ack() {
    const CASE: &str = "FE2O3_TEST_THREE_SLOT_APPLICATION";
    if std::env::var_os(CASE).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "application_descriptor_handoff::tests::production_consumer_rejects_three_slots_without_ack", "--nocapture"])
                .env(CASE, "1").status().unwrap();
        assert!(status.success());
        return;
    }
    use std::os::fd::IntoRawFd;
    let envelope = File::open("/dev/null").unwrap();
    let directory = File::open("/dev/null").unwrap();
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::NONBLOCK).unwrap();
    let descriptors = [
        envelope.into_raw_fd(),
        directory.into_raw_fd(),
        write.into_raw_fd(),
    ];
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
            .unwrap(),
        [9; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
        ],
    )
    .unwrap();
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"inert fixture").unwrap(),
        &occurrence,
    );
    let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([7; 32]).unwrap();
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    // SAFETY: exact self-spawned helper, no competing environment or descriptor users.
    unsafe {
        for name in worker_v2_handoff_environment_names()
            .into_iter()
            .chain(worker_v3_handoff_environment_names())
        {
            std::env::remove_var(name);
        }
        for (name, fd) in [
            WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
            WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
            WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        ]
        .into_iter()
        .zip(descriptors)
        {
            assert_eq!(libc::fcntl(fd, libc::F_SETFD, 0), 0);
            std::env::set_var(name, fd.to_string());
        }
        for (name, bytes) in [
            (
                WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
                occurrence.encode_canonical().unwrap().to_vec(),
            ),
            (
                WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
                expectation
                    .commitment()
                    .encode_canonical()
                    .unwrap()
                    .to_vec(),
            ),
            (
                WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
                challenge.encode_canonical().unwrap().to_vec(),
            ),
        ] {
            std::env::set_var(name, hex(&bytes));
        }
        assert!(matches!(
            consume_inherited_worker_v3_application_handoff_v1(KernelId::from_bytes([1; 32])),
            Err(
                WorkerV3ApplicationDescriptorHandoffErrorV1::MissingEnvironment(
                    WORKER_V3_APPLICATION_PROOF_FD_ENV_V1
                )
            )
        ));
        for fd in descriptors {
            assert_eq!(libc::fcntl(fd, libc::F_GETFD), -1);
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
        }
    }
    assert!(
        worker_v3_handoff_environment_names()
            .into_iter()
            .all(|name| std::env::var_os(name).is_none())
    );
    assert_eq!(rustix::io::read(&read, &mut [0_u8; 1]).unwrap(), 0);
}

#[test]
fn acknowledgment_revalidates_before_write_and_drops_to_eof() {
    isolated_eof_case(
        "application_descriptor_handoff::tests::acknowledgment_revalidates_before_write_and_drops_to_eof",
        acknowledgment_revalidates_before_write_and_drops_to_eof_inner,
    );
}

fn acknowledgment_revalidates_before_write_and_drops_to_eof_inner() {
    for valid in [false, true] {
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::NONBLOCK).unwrap();
        let write = File::from(write);
        let called = std::cell::Cell::new(false);
        let result = emit_acknowledgment_bytes(
            &write,
            b"exact ack",
            Instant::now() + Duration::from_secs(1),
            || {
                called.set(true);
                if valid {
                    Ok(())
                } else {
                    Err(WorkerV3ApplicationDescriptorHandoffErrorV1::CommitmentMismatch)
                }
            },
        );
        assert!(called.get());
        assert_eq!(result.is_ok(), valid);
        drop(write);
        let mut bytes = Vec::new();
        File::from(read).read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, if valid { b"exact ack".as_slice() } else { &[] });
    }
}

#[test]
fn acknowledgment_cannot_extend_an_expired_registration_deadline() {
    isolated_eof_case(
        "application_descriptor_handoff::tests::acknowledgment_cannot_extend_an_expired_registration_deadline",
        acknowledgment_cannot_extend_an_expired_registration_deadline_inner,
    );
}

fn acknowledgment_cannot_extend_an_expired_registration_deadline_inner() {
    for expire_during_validation in [false, true] {
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::NONBLOCK).unwrap();
        let write = File::from(write);
        let deadline = if expire_during_validation {
            Instant::now() + Duration::from_millis(100)
        } else {
            Instant::now()
        };
        let entered = std::cell::Cell::new(false);
        let result = emit_acknowledgment_bytes(&write, b"ack", deadline, || {
            entered.set(true);
            assert!(expire_during_validation);
            std::thread::sleep(
                deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
            );
            Ok(())
        });
        assert_eq!(entered.get(), expire_during_validation);
        assert!(matches!(
            result,
            Err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(
                ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout
            ))
        ));
        drop(write);
        assert_eq!(rustix::io::read(&read, &mut [0_u8; 1]).unwrap(), 0);
    }
}

#[test]
fn worker_v3_failed_claim_closes_every_available_input() {
    const CASE: &str = "FE2O3_TEST_FAILED_APPLICATION_CLAIM";
    let Ok(case) = std::env::var(CASE) else {
        for case in ["parse", "alias", "flags", "wire", "mixed"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "application_descriptor_handoff::tests::worker_v3_failed_claim_closes_every_available_input", "--nocapture"])
                    .env(CASE, case).status().unwrap();
            assert!(status.success(), "claim cleanup case {case}");
        }
        return;
    };
    use std::os::fd::IntoRawFd;
    let descriptors: [RawFd; 4] = std::array::from_fn(|_| {
        let (read, write) = rustix::pipe::pipe().unwrap();
        drop(write);
        rustix::io::fcntl_setfd(&read, rustix::io::FdFlags::empty()).unwrap();
        read.into_raw_fd()
    });
    // SAFETY: this exact self-spawned helper has no environment users or descriptor mutators.
    unsafe {
        for name in worker_v2_handoff_environment_names()
            .into_iter()
            .chain(worker_v3_handoff_environment_names())
        {
            std::env::remove_var(name);
        }
        for (name, fd) in [
            WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
            WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
            WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
            WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
        ]
        .into_iter()
        .zip(descriptors)
        {
            std::env::set_var(name, fd.to_string());
        }
        match case.as_str() {
            "parse" => std::env::set_var(WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, "bad"),
            "alias" => std::env::set_var(
                WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
                descriptors[2].to_string(),
            ),
            "flags" => assert_eq!(
                libc::fcntl(descriptors[0], libc::F_SETFD, libc::FD_CLOEXEC),
                0
            ),
            "wire" => {}
            "mixed" => std::env::set_var(WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, "bad"),
            _ => panic!("unknown cleanup case"),
        }
        assert!(claim_inherited_worker_v3_application_handoff_v1().is_err());
        for (index, fd) in descriptors.into_iter().enumerate() {
            if (case == "parse" && index == 0) || (case == "alias" && index == 3) {
                assert_eq!(libc::close(fd), 0);
            } else {
                assert_eq!(
                    libc::fcntl(fd, libc::F_GETFD),
                    -1,
                    "named input {index} leaked on {case}"
                );
                assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
            }
        }
    }
    assert!(
        worker_v3_handoff_environment_names()
            .into_iter()
            .all(|name| std::env::var_os(name).is_none())
    );
}

#[test]
fn worker_v3_environment_wire_requires_bounded_lowercase_canonical_hex() {
    assert_eq!(
        worker_v3_environment_wire("TEST", Some(OsStr::new("00af")), 2).unwrap(),
        [0, 0xaf]
    );
    for invalid in ["", "0", "00AF", "00ag", "000000"] {
        assert!(matches!(
            worker_v3_environment_wire("TEST", Some(OsStr::new(invalid)), 2),
            Err(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment("TEST"))
        ));
    }
}

#[test]
fn worker_v3_occurrence_rejects_application_and_descriptor_substitution() {
    let application =
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
            .unwrap();
    let inputs = [
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(1, 1, 2, 3).unwrap(),
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(2, 4, 5, 6).unwrap(),
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(3, 7, 8, 9).unwrap(),
    ];
    let supplied = WorkerV3ApplicationOccurrenceV1::new(application, [7; 32], &inputs).unwrap();
    assert!(validate_worker_v3_application_occurrence(&supplied, application, &inputs).is_ok());

    let mut substituted_image = sealed_static_test_elf_v1();
    *substituted_image.last_mut().unwrap() ^= 1;
    let substituted_application =
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&substituted_image).unwrap();
    assert!(matches!(
        validate_worker_v3_application_occurrence(&supplied, substituted_application, &inputs),
        Err(WorkerV3ApplicationDescriptorHandoffErrorV1::ApplicationIdentityMismatch)
    ));

    let substituted_inputs = [
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(1, 10, 2, 3).unwrap(),
        inputs[1],
        inputs[2],
    ];
    assert!(matches!(
        validate_worker_v3_application_occurrence(&supplied, application, &substituted_inputs),
        Err(WorkerV3ApplicationDescriptorHandoffErrorV1::DescriptorOccurrenceMismatch)
    ));
    let mut four_inputs = inputs.to_vec();
    four_inputs.push(
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, 10, 11, 12).unwrap(),
    );
    let four = WorkerV3ApplicationOccurrenceV1::new(application, [7; 32], &four_inputs).unwrap();
    assert!(validate_worker_v3_application_occurrence(&four, application, &four_inputs).is_ok());
    for (supplied, observed) in [
        (&four, inputs.as_slice()),
        (&supplied, four_inputs.as_slice()),
    ] {
        assert!(matches!(
            validate_worker_v3_application_occurrence(supplied, application, observed),
            Err(WorkerV3ApplicationDescriptorHandoffErrorV1::DescriptorOccurrenceMismatch)
        ));
    }
    four_inputs[3] =
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, 10, 99, 12).unwrap();
    assert!(matches!(
        validate_worker_v3_application_occurrence(&four, application, &four_inputs),
        Err(WorkerV3ApplicationDescriptorHandoffErrorV1::DescriptorOccurrenceMismatch)
    ));
}

#[test]
fn worker_v3_descriptor_environment_excludes_stdio_fd195_and_noncanonical_numbers() {
    for invalid in [
        "0",
        "1",
        "2",
        "195",
        "-1",
        "+3",
        "03",
        " 3",
        "3 ",
        "2147483648",
    ] {
        assert!(worker_v3_environment_fd("TEST", Some(OsStr::new(invalid))).is_err());
    }
    for valid in [3, 194, 196] {
        assert_eq!(
            worker_v3_environment_fd("TEST", Some(OsStr::new(&valid.to_string()))).unwrap(),
            valid
        );
    }
}

#[test]
fn worker_v3_commitment_rejects_envelope_substitution() {
    let application =
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
            .unwrap();
    let input = WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(1, 1, 2, 3).unwrap();
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(application, [9; 32], &[input]).unwrap();
    let first = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"first").unwrap(),
        &occurrence,
    );
    let second = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"second").unwrap(),
        &occurrence,
    );
    assert!(matches!(
        validate_worker_v3_application_commitment(first.commitment(), second.commitment()),
        Err(WorkerV3ApplicationDescriptorHandoffErrorV1::CommitmentMismatch)
    ));
    assert!(
        validate_worker_v3_application_commitment(first.commitment(), first.commitment()).is_ok()
    );
}

#[test]
fn worker_v3_envelope_name_requires_exact_lowercase_digest() {
    let valid = format!(
        "{WORKER_V3_ENVELOPE_PREFIX_V1}{}{WORKER_V3_ENVELOPE_SUFFIX_V1}",
        "ab".repeat(32)
    );
    assert!(is_canonical_worker_v3_envelope_name(valid.as_bytes()));
    assert!(!is_canonical_worker_v3_envelope_name(
        valid.to_ascii_uppercase().as_bytes()
    ));
    assert!(!is_canonical_worker_v3_envelope_name(
        format!("{valid}0").as_bytes()
    ));
}
