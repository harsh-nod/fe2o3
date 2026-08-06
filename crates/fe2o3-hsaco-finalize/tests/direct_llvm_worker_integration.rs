#![cfg(target_os = "linux")]

use std::{collections::BTreeSet, env, fs, fs::OpenOptions, io::Write, path::PathBuf};

use fe2o3_hsaco::CodeObjectVersion as InspectedCodeObjectVersion;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    LinkSymbolClosureV1, MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1,
    WorkerExecutionErrorKind, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, WorkerRequestV1, WorkerStageV1, construct_worker_request_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};

const WORKER_ENV: &str = "FE2O3_DIRECT_LLVM_WORKER";
const WORKER_BUILD_ID_ENV: &str = "FE2O3_DIRECT_LLVM_WORKER_BUILD_ID";
const LLVM_BUILD_ID_ENV: &str = "FE2O3_DIRECT_LLVM_BUILD_ID";
const BITCODE_ENV: &str = "FE2O3_DIRECT_LLVM_BITCODE";
const OBJECT_ENV: &str = "FE2O3_DIRECT_LLVM_OBJECT";
const EXPECTED_OUTPUT_ENV: &str = "FE2O3_DIRECT_LLVM_EXPECTED_OUTPUT";
const OUTPUT_ENV: &str = "FE2O3_DIRECT_LLVM_OUTPUT";
const TARGET_ENV: &str = "FE2O3_DIRECT_LLVM_TARGET";

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required native integration pin {name} is absent"))
}

struct PlanFixture {
    plan: MultiInputLinkPlanV1,
    inputs: Vec<WorkerInputV1>,
    input_kinds: LinkInputKindClosureV1,
    symbols: LinkSymbolClosureV1,
}

fn worker_options() -> WorkerOptionsV1 {
    WorkerOptionsV1::new(WorkerOptimizationLevelV1::O3, true, true)
}

fn plan_options() -> Vec<LinkOptionV1> {
    vec![
        LinkOptionV1::new("code-object-version", "5").unwrap(),
        LinkOptionV1::new("opt-level", "3").unwrap(),
        LinkOptionV1::new("strip-debug", "true").unwrap(),
        LinkOptionV1::new("verify-each", "true").unwrap(),
    ]
}

fn plan_fixture(
    target: DeviceTargetV1,
    bitcode: Vec<u8>,
    object: Vec<u8>,
    expected_output: &[u8],
) -> PlanFixture {
    let mut inputs = vec![
        WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, bitcode).unwrap(),
        WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, object).unwrap(),
    ];
    inputs.sort_by_key(|input| input.identity());
    let link_inputs: Vec<_> = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect();
    let output_identity = ContentIdentityV1::calculate(expected_output);
    let mut provenance: Vec<_> = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]).unwrap())
        .collect();
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .unwrap(),
    );
    let plan = MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        plan_options(),
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .unwrap();
    plan.verify_output_bytes(expected_output).unwrap();
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, inputs.iter().map(|input| input.kind()).collect())
            .unwrap();
    let symbols = LinkSymbolClosureV1::new(
        vec!["mixed_entry".to_owned(), "object_helper".to_owned()],
        vec!["object_helper".to_owned()],
        vec!["mixed_entry".to_owned()],
    )
    .unwrap();
    PlanFixture {
        plan,
        inputs,
        input_kinds,
        symbols,
    }
}

fn planned_request(fixture: &PlanFixture, llvm_build_id: &str) -> WorkerRequestV1 {
    construct_worker_request_v1(
        &fixture.plan,
        llvm_build_id,
        fixture.plan.target(),
        CodeObjectVersion::V5,
        worker_options(),
        fixture.inputs.clone(),
        &fixture.input_kinds,
        &fixture.symbols,
        WorkerOutputConstraintsV1::new(fixture.plan.output().identity().byte_len()).unwrap(),
    )
    .unwrap()
}

fn adversarial_protocol_request(
    llvm_build_id: &str,
    target: DeviceTargetV1,
    mut inputs: Vec<WorkerInputV1>,
    symbols: &[&str],
    output_bytes: u64,
) -> WorkerRequestV1 {
    inputs.sort_by_key(|input| (input.identity(), input.kind()));
    let mut symbols: Vec<String> = symbols.iter().map(|symbol| (*symbol).to_owned()).collect();
    symbols.sort();
    WorkerRequestV1::new(
        [0xa7; 32],
        llvm_build_id,
        target,
        CodeObjectVersion::V5,
        worker_options(),
        inputs,
        symbols.clone(),
        symbols,
        WorkerOutputConstraintsV1::new(output_bytes).unwrap(),
    )
    .unwrap()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn checked_range(bytes: &[u8], offset: u64, length: u64) -> std::ops::Range<usize> {
    let start = usize::try_from(offset).unwrap();
    let length = usize::try_from(length).unwrap();
    let end = start.checked_add(length).unwrap();
    assert!(end <= bytes.len(), "ELF range exceeds output bytes");
    start..end
}

fn dynamic_definitions(bytes: &[u8]) -> BTreeSet<String> {
    let section_offset = usize::try_from(read_u64(bytes, 40)).unwrap();
    let section_entry_size = usize::from(read_u16(bytes, 58));
    let section_count = usize::from(read_u16(bytes, 60));
    assert_eq!(section_entry_size, 64, "unexpected ELF64 section size");
    assert!(section_count <= 256, "unbounded ELF section table");
    checked_range(
        bytes,
        section_offset as u64,
        (section_entry_size * section_count) as u64,
    );

    let mut result = BTreeSet::new();
    for index in 0..section_count {
        let header = section_offset + index * section_entry_size;
        if read_u32(bytes, header + 4) != 11 {
            continue;
        }
        let symbols = checked_range(
            bytes,
            read_u64(bytes, header + 24),
            read_u64(bytes, header + 32),
        );
        let string_index = usize::try_from(read_u32(bytes, header + 40)).unwrap();
        assert!(
            string_index < section_count,
            "invalid dynamic string table link"
        );
        let string_header = section_offset + string_index * section_entry_size;
        let strings = checked_range(
            bytes,
            read_u64(bytes, string_header + 24),
            read_u64(bytes, string_header + 32),
        );
        let symbol_size = usize::try_from(read_u64(bytes, header + 56)).unwrap();
        assert_eq!(symbol_size, 24, "unexpected ELF64 symbol size");
        assert_eq!(symbols.len() % symbol_size, 0, "partial dynamic symbol");
        for symbol in symbols.clone().step_by(symbol_size) {
            let binding = bytes[symbol + 4] >> 4;
            if !matches!(binding, 1 | 2) {
                continue;
            }
            let name_offset = usize::try_from(read_u32(bytes, symbol)).unwrap();
            assert!(
                name_offset < strings.len(),
                "dynamic symbol name is out of bounds"
            );
            let name_start = strings.start + name_offset;
            let name_end = bytes[name_start..strings.end]
                .iter()
                .position(|byte| *byte == 0)
                .map(|length| name_start + length)
                .expect("unterminated dynamic symbol name");
            let name = std::str::from_utf8(&bytes[name_start..name_end]).unwrap();
            if name.is_empty() {
                continue;
            }
            let section = read_u16(bytes, symbol + 6);
            assert_ne!(section, 0, "undefined public dynamic symbol: {name}");
            let visibility = bytes[symbol + 5] & 3;
            if visibility == 0 {
                result.insert(name.to_owned());
            }
        }
    }
    result
}

fn inspect_gfx942_v5_hsaco(bytes: &[u8]) {
    assert!(bytes.len() >= 64, "truncated ELF header");
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(bytes[4], 2, "output is not ELF64");
    assert_eq!(bytes[5], 1, "output is not little-endian");
    assert_eq!(bytes[6], 1, "invalid ELF identification version");
    assert_eq!(bytes[7], 64, "output does not use the AMDHSA OS ABI");
    // ELF ABI versions 2, 3, and 4 encode AMDHSA code objects V4, V5, and V6.
    assert_eq!(bytes[8], 3, "output is not an AMDHSA V5 code object");
    assert_eq!(read_u16(bytes, 16), 3, "output is not ET_DYN");
    assert_eq!(read_u16(bytes, 18), 224, "output is not EM_AMDGPU");
    assert_eq!(read_u32(bytes, 20), 1, "invalid ELF version");
    assert_eq!(
        read_u32(bytes, 48),
        0x54c,
        "output does not have the complete canonical gfx942 feature flags"
    );
    let inspected = fe2o3_hsaco::inspect(bytes).expect("structured HSACO inspection failed");
    assert_eq!(
        inspected.code_object_version(),
        InspectedCodeObjectVersion::V5
    );
    assert_eq!(inspected.target().to_string(), "gfx942");
    assert_eq!(inspected.metadata_version().major(), 1);
    assert_eq!(inspected.metadata_version().minor(), 2);
    assert!(!inspected.has_printf_metadata());
    assert!(
        inspected.kernels().is_empty(),
        "the link-only fixture unexpectedly declares dispatchable kernels"
    );
    assert_eq!(
        dynamic_definitions(bytes),
        BTreeSet::from(["mixed_entry".to_owned(), "object_helper".to_owned()])
    );
}

#[test]
#[ignore = "requires an explicitly pinned ROCm LLVM worker and generated native fixtures"]
fn real_worker_links_mixed_inputs_through_pinned_supervision() {
    let worker_path = PathBuf::from(required_env(WORKER_ENV));
    let worker_build_id = required_env(WORKER_BUILD_ID_ENV);
    let llvm_build_id = required_env(LLVM_BUILD_ID_ENV);
    let target_text = required_env(TARGET_ENV);
    assert_eq!(
        target_text, "gfx942",
        "the exported fixture is pinned to gfx942"
    );
    let target = DeviceTargetV1::parse(&target_text).expect("invalid pinned target");
    let bitcode = fs::read(required_env(BITCODE_ENV)).expect("could not read bitcode fixture");
    let object = fs::read(required_env(OBJECT_ENV)).expect("could not read object fixture");
    let expected_output =
        fs::read(required_env(EXPECTED_OUTPUT_ENV)).expect("could not read expected HSACO fixture");
    let worker_bytes = fs::read(&worker_path).expect("could not read worker executable");
    let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
    let measurement = WorkerMeasurementV1::new(
        worker_identity,
        worker_build_id.clone(),
        llvm_build_id.clone(),
    )
    .unwrap();

    let wrong_input_identity = ContentIdentityV1::from_parts([0x7d; 32], bitcode.len() as u64);
    assert_eq!(
        WorkerInputV1::from_declared(
            WorkerInputKindV1::LlvmBitcode,
            wrong_input_identity,
            bitcode.clone(),
        ),
        Err(WorkerProtocolError::ContentIdentityMismatch)
    );
    let wrong_worker_identity =
        ContentIdentityV1::from_parts([0x6e; 32], worker_bytes.len() as u64);
    let wrong_measurement = WorkerMeasurementV1::new(
        wrong_worker_identity,
        worker_build_id.clone(),
        llvm_build_id.clone(),
    )
    .unwrap();
    assert!(matches!(
        PinnedWorkerV1::open(&worker_path, wrong_measurement)
            .unwrap_err()
            .kind(),
        WorkerExecutionErrorKind::WorkerIdentityMismatch { .. }
    ));

    let pinned = PinnedWorkerV1::open(&worker_path, measurement).unwrap();
    assert_eq!(pinned.measurement().executable(), worker_identity);
    assert_eq!(
        pinned.measurement().worker_build_identity(),
        worker_build_id
    );
    assert_eq!(pinned.measurement().llvm_build_identity(), llvm_build_id);

    let fixture = plan_fixture(target, bitcode.clone(), object.clone(), &expected_output);
    let mixed_request = planned_request(&fixture, &llvm_build_id);
    let repeated_request = planned_request(&fixture, &llvm_build_id);
    assert_eq!(mixed_request.request_id(), repeated_request.request_id());
    assert_eq!(mixed_request.identity(), repeated_request.identity());
    assert_ne!(mixed_request.request_id(), &[0x23; 32]);
    assert_ne!(mixed_request.request_id(), &[0; 32]);
    assert_eq!(
        WorkerRequestV1::decode(mixed_request.canonical_bytes()).unwrap(),
        mixed_request
    );
    assert_eq!(mixed_request.llvm_build_identity(), llvm_build_id);

    let execution = pinned
        .execute(&mixed_request, WorkerExecutionLimitsV1::default())
        .unwrap();
    assert_eq!(execution.worker_executable(), worker_identity);
    assert!(execution.response().binds_request(&mixed_request));
    assert_eq!(
        execution.response().request_id(),
        mixed_request.request_id()
    );
    assert_eq!(
        execution.response().request_identity(),
        mixed_request.identity()
    );
    assert_eq!(
        execution.response().worker_build_identity(),
        worker_build_id
    );
    assert_eq!(execution.response().stage(), WorkerStageV1::Complete);
    let output = execution
        .response()
        .output()
        .expect("successful output absent");
    assert!(output.identity().matches(output.bytes()));
    assert_eq!(output.identity(), fixture.plan.output().identity());
    fixture.plan.verify_output_bytes(output.bytes()).unwrap();
    inspect_gfx942_v5_hsaco(output.bytes());
    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(required_env(OUTPUT_ENV))
        .expect("could not create fresh inspected HSACO");
    output_file
        .write_all(output.bytes())
        .expect("could not persist inspected HSACO");
    output_file
        .sync_all()
        .expect("could not sync inspected HSACO");

    let object_as_bitcode = adversarial_protocol_request(
        &llvm_build_id,
        target,
        vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, object).unwrap()],
        &["object_helper"],
        expected_output.len() as u64,
    );
    let rejected = pinned
        .execute(&object_as_bitcode, WorkerExecutionLimitsV1::default())
        .unwrap();
    assert!(rejected.response().binds_request(&object_as_bitcode));
    assert_eq!(rejected.response().stage(), WorkerStageV1::InputValidation);
    assert!(rejected.response().output().is_none());

    let bitcode_as_object = adversarial_protocol_request(
        &llvm_build_id,
        target,
        vec![WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, bitcode).unwrap()],
        &["mixed_entry"],
        expected_output.len() as u64,
    );
    let rejected = pinned
        .execute(&bitcode_as_object, WorkerExecutionLimitsV1::default())
        .unwrap();
    assert!(rejected.response().binds_request(&bitcode_as_object));
    assert_eq!(rejected.response().stage(), WorkerStageV1::InputValidation);
    assert!(rejected.response().output().is_none());

    let wrong_llvm_build_id = "deliberately-wrong-llvm-build";
    let wrong_toolchain_request = planned_request(&fixture, wrong_llvm_build_id);
    assert_eq!(
        pinned
            .execute(&wrong_toolchain_request, WorkerExecutionLimitsV1::default())
            .unwrap_err()
            .kind(),
        &WorkerExecutionErrorKind::LlvmBuildIdentityMismatch
    );

    let wrong_toolchain_measurement = WorkerMeasurementV1::new(
        worker_identity,
        worker_build_id.clone(),
        wrong_llvm_build_id,
    )
    .unwrap();
    let wrong_toolchain_worker =
        PinnedWorkerV1::open(&worker_path, wrong_toolchain_measurement).unwrap();
    let rejected = wrong_toolchain_worker
        .execute(&wrong_toolchain_request, WorkerExecutionLimitsV1::default())
        .unwrap();
    assert!(rejected.response().binds_request(&wrong_toolchain_request));
    assert_eq!(rejected.response().stage(), WorkerStageV1::Toolchain);
    assert_eq!(rejected.response().worker_build_identity(), worker_build_id);
    assert!(rejected.response().output().is_none());
    assert!(!rejected.response().diagnostics().is_empty());

    let wrong_worker_measurement = WorkerMeasurementV1::new(
        worker_identity,
        "deliberately-wrong-worker-build",
        llvm_build_id,
    )
    .unwrap();
    let wrong_worker = PinnedWorkerV1::open(&worker_path, wrong_worker_measurement).unwrap();
    assert_eq!(
        wrong_worker
            .execute(&mixed_request, WorkerExecutionLimitsV1::default())
            .unwrap_err()
            .kind(),
        &WorkerExecutionErrorKind::WorkerBuildIdentityMismatch
    );
}
