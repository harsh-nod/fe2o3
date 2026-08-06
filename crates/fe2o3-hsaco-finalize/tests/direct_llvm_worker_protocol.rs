use std::{
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
};

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, MAX_WORKER_DIAGNOSTIC_BYTES, MAX_WORKER_DIAGNOSTICS,
    MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_SYMBOL_BYTES, MAX_WORKER_SYMBOLS,
    MAX_WORKER_TOOLCHAIN_ID_BYTES, WorkerEvidenceClassV1, WorkerInputKindV1, WorkerInputV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerOutputV1,
    WorkerProtocolError, WorkerRequestV1, WorkerResponseV1, WorkerStageV1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

fn sample_request() -> WorkerRequestV1 {
    let mut inputs = vec![
        WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, b"bitcode-b".to_vec()).unwrap(),
        WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, b"object-a".to_vec()).unwrap(),
    ];
    inputs.sort_by_key(|input| (input.identity(), input.kind()));
    WorkerRequestV1::new(
        [0x5a; 32],
        "llvmorg-22.0.0-rocm-7.2+0123456789abcdef",
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        inputs,
        vec!["kernel_a".to_owned()],
        vec!["kernel_a".to_owned(), "kernel_b".to_owned()],
        WorkerOutputConstraintsV1::new(4 * 1024 * 1024).unwrap(),
    )
    .unwrap()
}

#[test]
fn request_round_trip_has_stable_golden_wire() {
    let request = sample_request();
    let decoded = WorkerRequestV1::decode(request.canonical_bytes()).unwrap();
    assert_eq!(decoded, request);
    assert_eq!(request.canonical_bytes().len(), 344);
    assert_eq!(
        hex(Sha256::digest(request.canonical_bytes()).as_slice()),
        "5c05af2a5b3ac53c14b28e2fa7f5bd64b0936545e94980132229d97d61ea199c"
    );
    assert_eq!(
        hex(request.identity()),
        "c0c0bbd080561748e3ff39bbeaaf6684dcb8b368e6a92bdde32b3a8c39c4fdfa"
    );
    assert_eq!(decoded.target().to_string(), "gfx942:xnack-");
    assert_eq!(decoded.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        decoded.options().optimization(),
        WorkerOptimizationLevelV1::O2
    );
    assert!(decoded.options().strip_debug());
    assert!(decoded.options().verify_each());
    assert_eq!(decoded.evidence_class(), WorkerEvidenceClassV1::GenericLink);
    assert!(!decoded.grants_link_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
}

#[test]
fn independently_constructed_ffi_like_claims_remain_generic_link_evidence() {
    let base = sample_request();
    let request = WorkerRequestV1::new(
        [0x6b; 32],
        base.llvm_build_identity(),
        base.target(),
        base.code_object_version(),
        base.options(),
        base.inputs().to_vec(),
        vec!["external_device_add".to_owned()],
        vec!["external_device_add".to_owned()],
        WorkerOutputConstraintsV1::new(base.output_constraints().max_bytes()).unwrap(),
    )
    .unwrap();
    let output = WorkerOutputV1::new(b"FFI-like bytes without FFI provenance".to_vec()).unwrap();
    let response = WorkerResponseV1::success(&request, "worker", vec![], output).unwrap();

    assert_eq!(request.evidence_class(), WorkerEvidenceClassV1::GenericLink);
    assert_eq!(
        response.evidence_class(),
        WorkerEvidenceClassV1::GenericLink
    );
    assert_eq!(
        response.output().unwrap().evidence_class(),
        WorkerEvidenceClassV1::GenericLink
    );
}

#[test]
fn every_request_truncation_and_trailing_byte_is_rejected() {
    let bytes = sample_request().canonical_bytes().to_vec();
    for length in 0..bytes.len() {
        assert!(
            WorkerRequestV1::decode(&bytes[..length]).is_err(),
            "accepted request prefix {length}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        WorkerRequestV1::decode(&trailing),
        Err(WorkerProtocolError::TrailingBytes)
    );
}

#[test]
fn mutations_are_panic_free_and_identity_protected() {
    let bytes = sample_request().canonical_bytes().to_vec();
    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0x80;
        catch_unwind(AssertUnwindSafe(|| WorkerRequestV1::decode(&mutated)))
            .unwrap_or_else(|_| panic!("decoder panicked at byte {index}"))
            .ok();
    }

    let input_payload = find_field(&bytes, 6);
    let payload_offset = input_payload.as_ptr() as usize - bytes.as_ptr() as usize;
    let first_input_byte = payload_offset + 4 + 1 + 32 + 8;
    let mut bad_digest = bytes;
    bad_digest[first_input_byte] ^= 1;
    assert_eq!(
        WorkerRequestV1::decode(&bad_digest),
        Err(WorkerProtocolError::ContentIdentityMismatch)
    );
}

#[test]
fn rejects_unknown_duplicate_and_noncanonical_tags() {
    let bytes = sample_request().canonical_bytes().to_vec();
    let mut unknown = bytes.clone();
    unknown[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        WorkerRequestV1::decode(&unknown),
        Err(WorkerProtocolError::UnknownTag(u16::MAX))
    );

    let second_tag = 8 + 6 + 32;
    let mut duplicate = bytes.clone();
    duplicate[second_tag..second_tag + 2].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        WorkerRequestV1::decode(&duplicate),
        Err(WorkerProtocolError::DuplicateTag(1))
    );

    let mut reordered = bytes;
    reordered[second_tag..second_tag + 2].copy_from_slice(&3u16.to_le_bytes());
    assert_eq!(
        WorkerRequestV1::decode(&reordered),
        Err(WorkerProtocolError::NonCanonicalTag {
            expected: 2,
            actual: 3
        })
    );
}

#[test]
fn hostile_counts_and_lengths_fail_before_payload_allocation() {
    let request = sample_request();
    let mut too_many_inputs = request.canonical_bytes().to_vec();
    let inputs = find_field(&too_many_inputs, 6);
    let offset = inputs.as_ptr() as usize - too_many_inputs.as_ptr() as usize;
    too_many_inputs[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        WorkerRequestV1::decode(&too_many_inputs),
        Err(WorkerProtocolError::TooManyInputs)
    );

    let mut huge_field = request.canonical_bytes().to_vec();
    huge_field[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        WorkerRequestV1::decode(&huge_field),
        Err(WorkerProtocolError::FieldTooLarge(1))
    );
}

#[test]
fn option_whitelist_and_canonical_target_are_strict() {
    let request = sample_request();
    let mut bad_option = request.canonical_bytes().to_vec();
    let options = find_field(&bad_option, 5);
    let offset = options.as_ptr() as usize - bad_option.as_ptr() as usize;
    bad_option[offset] = 4;
    assert_eq!(
        WorkerRequestV1::decode(&bad_option),
        Err(WorkerProtocolError::UnsupportedOption)
    );

    let mut bad_bool = request.canonical_bytes().to_vec();
    let options = find_field(&bad_bool, 5);
    let offset = options.as_ptr() as usize - bad_bool.as_ptr() as usize;
    bad_bool[offset + 1] = 2;
    assert_eq!(
        WorkerRequestV1::decode(&bad_bool),
        Err(WorkerProtocolError::UnsupportedOption)
    );

    let mut noncanonical_target = request.canonical_bytes().to_vec();
    let target = find_field(&noncanonical_target, 3);
    let offset = target.as_ptr() as usize - noncanonical_target.as_ptr() as usize;
    // Same byte count as gfx942:xnack-, but noncanonical feature punctuation.
    noncanonical_target[offset + 6] = b'_';
    assert_eq!(
        WorkerRequestV1::decode(&noncanonical_target),
        Err(WorkerProtocolError::InvalidTarget)
    );
}

#[test]
fn constructors_reject_noncanonical_inputs_symbols_and_bounds() {
    let mut inputs = sample_request().inputs().to_vec();
    inputs.reverse();
    assert_eq!(
        request_with(
            inputs,
            strings(&["kernel_a"]),
            strings(&["kernel_a", "kernel_b"])
        ),
        Err(WorkerProtocolError::NonCanonicalInputs)
    );
    assert_eq!(
        request_with(
            vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap()],
            strings(&["kernel_b", "kernel_a"]),
            strings(&["kernel_a", "kernel_b"])
        ),
        Err(WorkerProtocolError::NonCanonicalSymbols)
    );
    assert_eq!(
        request_with(
            vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap()],
            strings(&["missing"]),
            strings(&["kernel_a"])
        ),
        Err(WorkerProtocolError::RequiredSymbolNotExpected)
    );
    assert_eq!(
        WorkerOutputConstraintsV1::new(0),
        Err(WorkerProtocolError::InvalidOutputBound)
    );
    assert_eq!(
        WorkerOutputConstraintsV1::new(MAX_WORKER_OUTPUT_BYTES as u64 + 1),
        Err(WorkerProtocolError::InvalidOutputBound)
    );
    assert!(WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![]).is_err());
    assert_eq!(
        WorkerInputV1::from_declared(
            WorkerInputKindV1::LlvmBitcode,
            ContentIdentityV1::from_parts([7; 32], 1),
            vec![1]
        ),
        Err(WorkerProtocolError::ContentIdentityMismatch)
    );
}

#[test]
fn response_round_trip_binds_request_measurement_and_output() {
    let request = sample_request();
    let output = WorkerOutputV1::new(b"\x7fELF deterministic hsaco".to_vec()).unwrap();
    let response = WorkerResponseV1::success(
        &request,
        "llvmorg-22.0.0-rocm-7.2+0123456789abcdef",
        vec![
            "linked 2 inputs".to_owned(),
            "verified AMDGPU ELF".to_owned(),
        ],
        output,
    )
    .unwrap();
    let decoded = WorkerResponseV1::decode(response.canonical_bytes()).unwrap();
    assert_eq!(decoded, response);
    assert!(decoded.binds_request(&request));
    assert_eq!(decoded.stage(), WorkerStageV1::Complete);
    assert_eq!(decoded.evidence_class(), WorkerEvidenceClassV1::GenericLink);
    assert_eq!(
        decoded.output().unwrap().evidence_class(),
        WorkerEvidenceClassV1::GenericLink
    );
    assert_eq!(
        decoded.output().unwrap().bytes(),
        b"\x7fELF deterministic hsaco"
    );
    assert!(
        decoded
            .output()
            .unwrap()
            .identity()
            .matches(decoded.output().unwrap().bytes())
    );
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());

    let failure = WorkerResponseV1::failure(
        *request.request_id(),
        *request.identity(),
        request.llvm_build_identity(),
        WorkerStageV1::NativeLink,
        vec!["undefined symbol: helper".to_owned()],
    )
    .unwrap();
    let decoded_failure = WorkerResponseV1::decode(failure.canonical_bytes()).unwrap();
    assert!(decoded_failure.binds_request(&request));
    assert!(decoded_failure.output().is_none());
    assert_eq!(
        decoded_failure.evidence_class(),
        WorkerEvidenceClassV1::GenericLink
    );
}

#[test]
fn response_state_and_diagnostics_are_canonical_and_bounded() {
    let request = sample_request();
    assert_eq!(
        WorkerResponseV1::failure(
            *request.request_id(),
            *request.identity(),
            request.llvm_build_identity(),
            WorkerStageV1::Complete,
            vec![]
        ),
        Err(WorkerProtocolError::InvalidResponseState)
    );
    assert_eq!(
        WorkerResponseV1::failure(
            *request.request_id(),
            *request.identity(),
            request.llvm_build_identity(),
            WorkerStageV1::Codegen,
            vec!["z".to_owned(), "a".to_owned()]
        ),
        Err(WorkerProtocolError::NonCanonicalDiagnostics)
    );
    assert_eq!(
        WorkerResponseV1::failure(
            *request.request_id(),
            *request.identity(),
            request.llvm_build_identity(),
            WorkerStageV1::Codegen,
            vec!["x".repeat(MAX_WORKER_DIAGNOSTIC_BYTES + 1)]
        ),
        Err(WorkerProtocolError::InvalidDiagnostic)
    );
    assert_eq!(
        WorkerResponseV1::failure(
            *request.request_id(),
            *request.identity(),
            request.llvm_build_identity(),
            WorkerStageV1::Codegen,
            vec![String::new(); MAX_WORKER_DIAGNOSTICS + 1]
        ),
        Err(WorkerProtocolError::TooManyDiagnostics)
    );
    assert_eq!(
        WorkerResponseV1::failure(
            *request.request_id(),
            *request.identity(),
            request.llvm_build_identity(),
            WorkerStageV1::Codegen,
            vec![String::new()]
        ),
        Err(WorkerProtocolError::InvalidDiagnostic)
    );
}

#[test]
fn response_truncation_trailing_bytes_and_output_tampering_are_rejected() {
    let request = sample_request();
    let response = WorkerResponseV1::success(
        &request,
        "measured-worker",
        vec![],
        WorkerOutputV1::new(b"\x7fELF output".to_vec()).unwrap(),
    )
    .unwrap();
    let bytes = response.canonical_bytes().to_vec();
    for length in 0..bytes.len() {
        assert!(
            WorkerResponseV1::decode(&bytes[..length]).is_err(),
            "accepted response prefix {length}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        WorkerResponseV1::decode(&trailing),
        Err(WorkerProtocolError::TrailingBytes)
    );

    let output = find_field(&bytes, 6);
    let offset = output.as_ptr() as usize - bytes.as_ptr() as usize;
    let mut tampered = bytes;
    tampered[offset + 1 + 32 + 8] ^= 1;
    assert_eq!(
        WorkerResponseV1::decode(&tampered),
        Err(WorkerProtocolError::ContentIdentityMismatch)
    );
}

#[test]
fn public_text_and_collection_bounds_are_enforced() {
    assert_eq!(
        WorkerRequestV1::new(
            [1; 32],
            "x".repeat(MAX_WORKER_TOOLCHAIN_ID_BYTES + 1),
            DeviceTargetV1::parse("gfx942").unwrap(),
            CodeObjectVersion::V6,
            WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, false, true),
            vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap()],
            vec![],
            vec![],
            WorkerOutputConstraintsV1::new(1).unwrap()
        ),
        Err(WorkerProtocolError::InvalidText("LLVM build identity"))
    );
    let too_many_symbols = (0..=MAX_WORKER_SYMBOLS)
        .map(|index| format!("s{index:04}"))
        .collect();
    assert_eq!(
        request_with(
            vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap()],
            Vec::new(),
            too_many_symbols
        ),
        Err(WorkerProtocolError::TooManySymbols)
    );
    assert_eq!(
        request_with(
            vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap()],
            Vec::new(),
            vec!["s".repeat(MAX_WORKER_SYMBOL_BYTES + 1)]
        ),
        Err(WorkerProtocolError::InvalidSymbol)
    );
}

#[test]
fn cpp_worker_cross_language_failure_round_trip_when_configured() {
    let Ok(worker) = std::env::var("FE2O3_LLVM_LINK_WORKER") else {
        return;
    };
    let worker_build_identity = std::env::var("FE2O3_LLVM_LINK_WORKER_BUILD_ID")
        .expect("worker path requires its exact measured build identity");
    let llvm_build_identity = std::env::var("FE2O3_LLVM_BUILD_ID")
        .expect("worker path requires its exact LLVM build identity");
    let request = WorkerRequestV1::new(
        [0x81; 32],
        llvm_build_identity,
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        vec![
            WorkerInputV1::new(
                WorkerInputKindV1::LlvmBitcode,
                b"deliberately invalid bitcode".to_vec(),
            )
            .unwrap(),
        ],
        vec![],
        vec![],
        WorkerOutputConstraintsV1::new(1024 * 1024).unwrap(),
    )
    .unwrap();
    if let Ok(path) = std::env::var("FE2O3_LLVM_LINK_WORKER_DUMP_REQUEST") {
        std::fs::write(path, request.canonical_bytes()).unwrap();
    }
    let mut child = Command::new(worker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.canonical_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "worker stderr: {:?}",
        output.stderr
    );
    assert!(output.stderr.is_empty());
    let response = WorkerResponseV1::decode(&output.stdout).unwrap();
    assert!(response.binds_request(&request));
    assert!(
        response
            .worker_build_identity()
            .starts_with("fe2o3-worker-v1-sha256-")
    );
    assert_eq!(response.worker_build_identity(), worker_build_identity);
    assert_ne!(
        response.worker_build_identity(),
        request.llvm_build_identity()
    );
    assert_eq!(response.stage(), WorkerStageV1::InputValidation);
    assert!(response.output().is_none());
    assert!(!response.diagnostics().is_empty());
}

fn request_with(
    inputs: Vec<WorkerInputV1>,
    required: Vec<String>,
    defined: Vec<String>,
) -> Result<WorkerRequestV1, WorkerProtocolError> {
    WorkerRequestV1::new(
        [1; 32],
        "llvm-build",
        DeviceTargetV1::parse("gfx942").unwrap(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, false, true),
        inputs,
        required,
        defined,
        WorkerOutputConstraintsV1::new(1024).unwrap(),
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn find_field(message: &[u8], wanted: u16) -> &[u8] {
    let mut cursor = 8;
    while cursor < message.len() {
        let tag = u16::from_le_bytes(message[cursor..cursor + 2].try_into().unwrap());
        let len = u32::from_le_bytes(message[cursor + 2..cursor + 6].try_into().unwrap()) as usize;
        let payload = &message[cursor + 6..cursor + 6 + len];
        if tag == wanted {
            return payload;
        }
        cursor += 6 + len;
    }
    panic!("missing field {wanted}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
