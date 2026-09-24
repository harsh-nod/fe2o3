//! Opt-in native V4 -> measured fixture Worker -> strict HSACO finalization.
//!
//! Uses the same four signed-content TEST exports as native_first_build_worker.
//! The ELF bytes and derivation stages are synthetic. These tests exercise real
//! structural admission/finalization, not LLVM, protected proof, or GPU execution.
#![cfg(target_os = "linux")]

use std::{fs, io::Write, os::unix::fs::PermissionsExt, path::Path, time::Duration};

use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceV1 as Descriptor,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as STORAGE_LIMIT,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizationError, InertDecodedWorkerExchangeV2,
    InertNativeFirstBuildWorkerEvidenceV1, LinkOptionV1, NativeWorkerFinalizationErrorV1,
    PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    execute_preflighted_native_reproducible_first_build_worker_v1 as execute,
    finalize_native_worker_hsaco_v1 as finalize, inspect_unfinalized,
    preflight_native_reproducible_first_build_worker_v1 as preflight, verify_finalized,
};
use fe2o3_kernel_descriptor::{BlockSizeV1, DeviceDescriptorTableV1, ProducerIdentityV1, Text};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[path = "fixtures/native_worker_publication_roundtrip_tests.rs"]
mod publication_roundtrip_tests;
#[path = "native_first_build_worker.rs"]
mod worker;
use worker::{CASES, Fixtures, LLVM_ID, Ready, Scratch, WORK_LIMIT, WORKER_ID, backing, closure};

const OUTPUT_BOUND: u64 = 64 * 1024;
const APPENDED_HSACO_MAGIC: &[u8; 16] = b"F3NATIVEHSACO01\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingDescriptor,
    ForeignDescriptor,
    Symbols,
    Target,
    Launch,
    Abi,
}

mod elf_fixture {
    // Reuse the crate's ELF layout/symbol/descriptor/note machinery rather than
    // another generator. Only this test's pointer ABI metadata differs.
    include!("fixtures/worker_v3_hsaco_test_support.rs");

    use super::{BlockSizeV1, Descriptor, Mutation};
    use object::{Object as _, ObjectSection as _};

    pub(super) fn raw(
        descriptor: &Descriptor,
        embedded: Option<&[u8]>,
        mutation: Mutation,
    ) -> Vec<u8> {
        let table = descriptor.table();
        assert_eq!(
            table.kernels().len(),
            2,
            "the exported fixture has two source roots"
        );
        let target = if mutation == Mutation::Target {
            if table.device_target().to_string() == "gfx942:xnack-" {
                "gfx950:xnack-".to_owned()
            } else {
                "gfx942:xnack-".to_owned()
            }
        } else {
            table.device_target().to_string()
        };
        let options: Vec<_> = table
            .kernels()
            .iter()
            .enumerate()
            .map(|(index, kernel)| {
                let mut options = FixtureOptions::valid();
                options.target = &target;
                options.entry = kernel.entry_name().as_str();
                options.descriptor = kernel.descriptor_symbol().as_str();
                options.include_export = false;
                options.kernarg_segment_size_override =
                    Some(u64::from(kernel.abi_layout().kernarg_segment_size()));
                assert_eq!(kernel.abi_layout().explicit_argument_size(), 8);
                assert_eq!(kernel.abi_layout().kernarg_segment_size(), 264);
                assert_eq!(kernel.arguments().len(), 1);
                let BlockSizeV1::Exact(block) = kernel.launch().block_size() else {
                    panic!("fixture launch must be exact");
                };
                options.required_workgroup_size = [block.x(), block.y(), block.z()];
                options.max_flat_workgroup_size = kernel.launch().max_flat_workgroup_size();
                if mutation == Mutation::Symbols && index == 1 {
                    options.entry = "foreign_native_store";
                    options.descriptor = "foreign_native_store.kd";
                }
                if mutation == Mutation::Launch {
                    options.required_workgroup_size = [2, 1, 1];
                    options.max_flat_workgroup_size = 2;
                }
                options
            })
            .collect();
        let mut result =
            legacy_fixture_with_optional_second_kernel(options[0], Some(options[1]), embedded);
        let kernels = options
            .into_iter()
            .map(|options| {
                let mut metadata = kernel_metadata(options);
                let Value::Map(fields) = &mut metadata else {
                    unreachable!()
                };
                let (_, arguments) = fields
                    .iter_mut()
                    .find(|(key, _)| matches!(key, Value::String(key) if key.as_str() == ".args"))
                    .unwrap();
                let pointer = if mutation == Mutation::Abi {
                    explicit_argument(Some("output"), 0, 8, Some(8), "by_value", None)
                } else {
                    explicit_pointer_argument(
                        Some("output"),
                        0,
                        8,
                        Some(8),
                        "global_buffer",
                        Some("global"),
                        Some(4),
                    )
                };
                let mut actual = vec![pointer];
                actual.extend(v5_hidden_arguments(8));
                *arguments = Value::Array(actual);
                metadata
            })
            .collect();
        let mut metadata = Vec::new();
        write_value(
            &mut metadata,
            &Value::Map(vec![
                (
                    Value::from("amdhsa.version"),
                    Value::Array(vec![Value::from(1), Value::from(2)]),
                ),
                (
                    Value::from("amdhsa.target"),
                    Value::from(format!("amdgcn-amd-amdhsa--{target}")),
                ),
                (Value::from("amdhsa.kernels"), Value::Array(kernels)),
            ]),
        )
        .unwrap();
        let note = metadata_note(&metadata);
        let (offset, old_size, index) = {
            let object = object::File::parse(result.bytes.as_slice()).unwrap();
            let section = object.section_by_name(".note").unwrap();
            let (offset, size) = section.file_range().unwrap();
            (offset as usize, size as usize, section.index().0)
        };
        assert!(
            note.len() <= old_size,
            "one pointer replaces the fixture's larger slice ABI"
        );
        result.bytes[offset..offset + old_size].fill(0);
        result.bytes[offset..offset + note.len()].copy_from_slice(&note);
        let table_offset = u64::from_le_bytes(result.bytes[40..48].try_into().unwrap()) as usize;
        write_u64(
            &mut result.bytes,
            table_offset + index * SECTION_HEADER_BYTES + 32,
            note.len() as u64,
        );
        result.bytes
    }
}

fn embedded_descriptor(descriptor: &Descriptor, mutation: Mutation) -> Vec<u8> {
    if mutation != Mutation::ForeignDescriptor {
        return descriptor.canonical_bytes().to_vec();
    }
    // Individually valid, ABI-identical descriptor with a foreign producer. This
    // must fail the retained-source byte join, not ordinary ELF/ABI inspection.
    let table = descriptor.table();
    Descriptor::new(
        DeviceDescriptorTableV1::new(
            table.canonical_code_object_digest(),
            table.code_object_version(),
            table.compiler().clone(),
            ProducerIdentityV1::new(
                Text::new("foreign-native-test").unwrap(),
                Text::new("1").unwrap(),
            ),
            table.device_target(),
            table.type_records().to_vec(),
            table.layout_records().to_vec(),
            table.kernels().to_vec(),
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .to_vec()
}

fn measured_worker(output: &[u8]) -> (Scratch, PinnedWorkerV1) {
    assert!((1..=OUTPUT_BOUND as usize).contains(&output.len()));
    let scratch = Scratch::new();
    let path = scratch.0.join("native-finalization-worker");
    fs::copy(
        Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-executor-fixture")),
        &path,
    )
    .unwrap();
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(output).unwrap();
    file.write_all(&(output.len() as u64).to_le_bytes())
        .unwrap();
    file.write_all(APPENDED_HSACO_MAGIC).unwrap();
    file.sync_all().unwrap();
    drop(file); // No writable handle survives measurement or executable spawn.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
    let bytes = fs::read(&path).unwrap();
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    (scratch, PinnedWorkerV1::open(&path, measurement).unwrap())
}

fn evidence(
    fixtures: &Fixtures,
    case: usize,
    mutation: Mutation,
    budget: &mut Budget<'_>,
) -> (InertNativeFirstBuildWorkerEvidenceV1, Ready) {
    let (erased, _, name) = CASES[case];
    let ready = Ready::publish(fixtures, name, 90 + case as u8, budget);
    let token = ready.recover(budget);
    let original = backing(token.content(), erased);
    let descriptor = Descriptor::decode(
        token
            .content()
            .handoff()
            .capsule()
            .base()
            .receipts()
            .abi()
            .canonical_preimage(),
    )
    .unwrap();
    let embedded = embedded_descriptor(&descriptor, mutation);
    let raw = elf_fixture::raw(
        &descriptor,
        (mutation != Mutation::MissingDescriptor).then_some(embedded.as_slice()),
        mutation,
    );
    if matches!(mutation, Mutation::None | Mutation::ForeignDescriptor) {
        let inspected = inspect_unfinalized(&raw)
            .expect("fixture must be valid AMDHSA ELF before Worker execution");
        assert_eq!(inspected.descriptor_table().kernels().len(), 2);
    } else {
        let error = inspect_unfinalized(&raw).unwrap_err();
        let expected = match mutation {
            Mutation::MissingDescriptor => {
                matches!(error, FinalizationError::MissingDescriptorSection)
            }
            Mutation::Symbols => matches!(
                error,
                FinalizationError::DescriptorKernelMissingInMetadata { .. }
            ),
            Mutation::Target => matches!(error, FinalizationError::DeviceTargetMismatch),
            Mutation::Launch => matches!(
                error,
                FinalizationError::MaxFlatWorkgroupSizeMismatch { .. }
            ),
            Mutation::Abi => matches!(
                error,
                FinalizationError::PhysicalArgumentMismatch {
                    field: ".value_kind",
                    ..
                }
            ),
            Mutation::None | Mutation::ForeignDescriptor => unreachable!(),
        };
        assert!(expected, "wrong negative fixture: {mutation:?}: {error:?}");
    }
    let (scratch, worker) = measured_worker(&raw);
    let options = [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect();
    let (prepared, storage) = preflight(
        &token,
        ready.receipt,
        closure(token.content()),
        &worker,
        Vec::new(),
        options,
        WorkerOutputConstraintsV1::new(OUTPUT_BOUND).unwrap(),
        WorkerExecutionLimitsV1::new(Duration::from_secs(5), 128 * 1024, 1024).unwrap(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let consumed = ready.consume(token, budget);
    let (evidence, storage) = execute(consumed, prepared, &worker, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(backing(evidence.recovered_handoff(), erased), original);
    assert_eq!(evidence.output_bytes(), raw);
    assert_eq!(evidence.worker_measurement(), worker.measurement());
    for (request, response) in [
        (
            evidence.bootstrap_request_bytes(),
            evidence.bootstrap_response(),
        ),
        (
            evidence.exact_replay_request_bytes(),
            evidence.exact_replay_response(),
        ),
    ] {
        let exchange =
            InertDecodedWorkerExchangeV2::decode(request, response.canonical_bytes()).unwrap();
        assert!(exchange.request().external_providers().is_empty());
        assert_eq!(
            exchange.request().compiler_module().bytes(),
            evidence
                .recovered_handoff()
                .handoff()
                .module_handoff()
                .module_bytes()
        );
        assert_eq!(exchange.response().output().unwrap().bytes(), raw);
    }
    drop(worker);
    drop(scratch);
    (evidence, ready)
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_finalization_retains_all_four_v4_owners_and_patches_only_digest() {
    let fixtures = Fixtures::open();
    let mut identities = Vec::new();
    for (case, (erased, profile, _)) in CASES.into_iter().enumerate() {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (source, _ready) = evidence(&fixtures, case, Mutation::None, &mut budget);
        let original = backing(source.recovered_handoff(), erased);
        let source_identity = source.identity();
        let binding = source.binding().identity();
        let raw_identity = source.output_identity();
        let pointers = [
            source.output_bytes().as_ptr(),
            source.bootstrap_request_bytes().as_ptr(),
            source.exact_replay_request_bytes().as_ptr(),
        ];
        let required = source.required_retained_storage();
        assert!(required > source.storage().retained_storage());
        assert!(budget.storage() >= required);
        let floor = budget.storage();
        let before = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let (prepared, storage) = finalize(source, &mut budget).unwrap();
        assert_eq!(
            budget.storage(),
            floor,
            "native finalization header is returned unreserved"
        );
        assert!(budget.work() > before);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(storage.retained_storage() > 0);
        assert_eq!(
            prepared.required_retained_storage(),
            required + storage.retained_storage()
        );
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let source = prepared.source_evidence();
        assert_eq!(source.identity(), source_identity);
        assert_eq!(source.binding().identity(), binding);
        assert_eq!(source.output_identity(), raw_identity);
        assert_eq!(backing(source.recovered_handoff(), erased), original);
        assert_eq!(
            [
                source.output_bytes().as_ptr(),
                source.bootstrap_request_bytes().as_ptr(),
                source.exact_replay_request_bytes().as_ptr()
            ],
            pointers
        );
        let bytes = prepared.exact_finalized_bytes();
        let verified = verify_finalized(bytes).unwrap();
        assert_eq!(verified.digest(), prepared.canonical_digest());
        assert_eq!(
            verified.descriptor_table().device_target().to_string(),
            profile.device_target()
        );
        assert_eq!(
            prepared.output_identity(),
            ContentIdentityV1::calculate(bytes)
        );
        assert_ne!(prepared.output_identity(), raw_identity);
        let location = verified.location();
        let start = location.digest_offset();
        assert_eq!(source.output_bytes().len(), bytes.len());
        assert_eq!(&source.output_bytes()[start..start + 32], &[0; 32]);
        assert_eq!(&source.output_bytes()[..start], &bytes[..start]);
        assert_eq!(&source.output_bytes()[start + 32..], &bytes[start + 32..]);
        assert_eq!(
            prepared.descriptor_identity(),
            ContentIdentityV1::calculate(
                &bytes[location.offset()..location.offset() + location.size()]
            )
        );
        assert!(!verified.grants_launch_authority());
        assert!(!prepared.authenticates_compiler_origin());
        assert!(!prepared.grants_publication_authority());
        assert!(!prepared.grants_load_authority());
        assert!(!prepared.grants_launch_authority());
        assert!(
            !source
                .recovered_handoff()
                .grants_artifact_or_launch_authority()
        );
        let identity = *prepared.identity().as_bytes();
        assert_ne!(identity, [0; 32]);
        assert!(!identities.contains(&identity));
        identities.push(identity);
        drop(prepared);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_finalization_rejects_descriptor_symbols_target_launch_and_abi() {
    let fixtures = Fixtures::open();
    for case in 0..CASES.len() {
        for mutation in [
            Mutation::MissingDescriptor,
            Mutation::ForeignDescriptor,
            Mutation::Symbols,
            Mutation::Target,
            Mutation::Launch,
            Mutation::Abi,
        ] {
            let mut work = Work::new(WORK_LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            let (source, _ready) = evidence(&fixtures, case, mutation, &mut budget);
            let floor = budget.storage();
            let error = finalize(source, &mut budget).err().unwrap_or_else(|| {
                panic!("finalization accepted {mutation:?} in {}", CASES[case].2)
            });
            let expected_phase = match mutation {
                Mutation::Symbols | Mutation::Target | Mutation::Launch => "raw inspection",
                Mutation::MissingDescriptor | Mutation::ForeignDescriptor | Mutation::Abi => {
                    "canonical finalization"
                }
                Mutation::None => unreachable!(),
            };
            assert!(
                matches!(error, NativeWorkerFinalizationErrorV1::Artifact { phase, .. } if phase == expected_phase),
                "fixture must reach the intended structural refusal: {mutation:?}: {error:?}"
            );
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_finalization_refuses_unpaid_complete_source_floor() {
    let fixtures = Fixtures::open();
    for case in 0..CASES.len() {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (source, _ready) = evidence(&fixtures, case, Mutation::None, &mut budget);
        let missing_floor = source.required_retained_storage() - 1;
        budget
            .release_storage(budget.storage() - missing_floor)
            .unwrap();
        let error = finalize(source, &mut budget)
            .err()
            .expect("cannot detach the Worker header from its original source/preflight floor");
        assert!(
            matches!(
                error,
                NativeWorkerFinalizationErrorV1::Resource(Resource::Accounting)
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), missing_floor);
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_worker_finalization_refuses_exhausted_original_work_and_storage() {
    let fixtures = Fixtures::open();
    for exhausted_work in [true, false] {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (source, _ready) = evidence(&fixtures, 3, Mutation::None, &mut budget);
        if exhausted_work {
            budget.charge_work(WORK_LIMIT - budget.work()).unwrap();
        } else {
            budget
                .reserve_storage(STORAGE_LIMIT - budget.storage())
                .unwrap();
        }
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let error = finalize(source, &mut budget)
            .err()
            .expect("must use the original cumulative ledger");
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if exhausted_work {
            assert!(
                matches!(
                    error,
                    NativeWorkerFinalizationErrorV1::Resource(Resource::Work(_))
                ),
                "{error:?}"
            );
            assert_eq!(budget.work(), WORK_LIMIT);
            assert!(work.failed_work().unwrap() > WORK_LIMIT);
        } else {
            assert!(
                matches!(
                    error,
                    NativeWorkerFinalizationErrorV1::Resource(Resource::Storage(_))
                ),
                "{error:?}"
            );
            assert!(budget.failed_storage().unwrap() > STORAGE_LIMIT);
        }
    }
}
