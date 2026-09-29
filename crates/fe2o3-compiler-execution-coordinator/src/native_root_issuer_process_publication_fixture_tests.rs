// Real process descriptors and filesystem publication, with explicitly inert V5
// source/history/receipt leaves. This does not execute rustc or a protected proof.
use super::*;
use fe2o3_artifact_transaction::{
    BROKERED_ARTIFACT_DIRECTORY_PATH_V1, BROKERED_CODEGEN_BACKEND_PATH_V1, BUILD_ATTEMPT_ENV_V1,
    BuildInvocation, BuildSession, InertCompilerExecutionSubjectV3 as Subject, ProducerIdentity,
    begin_build_attempt, publish_compiler_module_handoff_v5,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    InertSemanticCompilerModuleHandoffLayoutV5 as OuterLayout,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5 as decode_work,
    seal_inert_semantic_compiler_module_handoff_v5 as seal_outer,
};
use fe2o3_compiler_lineage::{
    InertProductionSemanticCapsuleLayoutV5 as CapsuleLayout,
    NativeConditionalMetadataInputV1 as MetadataInputs,
    NativeConditionalMetadataLayoutV1 as MetadataLayout,
    NativeConditionalMetadataLayoutV2 as EnvelopeLayout,
    seal_inert_production_semantic_capsule_v5 as seal_capsule,
    seal_native_conditional_metadata_v1 as seal_metadata,
    seal_native_conditional_metadata_v2 as seal_envelope,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3 as Descriptor, RustcUnitV2, encode_descriptor_v3,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::{CString, OsString},
    fs::File,
    path::Path,
};

const TEMPLATE: &[u8] =
    include_bytes!("../../fe2o3-broker-authority-service/tests/fixtures/locked-publication-v5.bin");
const FIXTURE_STORAGE: usize = 32 * MAX_DESCRIPTOR_BYTES_V3 + 131072;
const CLOSURE_ENV: &str = "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1";
const BACKEND_ENV: &str = "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";

pub(super) struct Publication {
    pub directory: tempfile::TempDir,
    pub subject: Subject,
    pub handoff_limit: usize,
}

impl Publication {
    pub fn new(
        f: &fixtures::Fixture,
        mismatched_invocation: bool,
        b: &mut Budget<'_>,
    ) -> (Self, compiler::PublicationInputs) {
        let result = b
            .with_prepaid_scope(
                b.storage(),
                8,
                8192 * MAX_DESCRIPTOR_BYTES_V3,
                FIXTURE_STORAGE,
                |b| Ok::<_, Resource>(Self::construct(f, mismatched_invocation, b)),
            )
            .unwrap();
        // Only these surviving fixture owners escape construction. Decoded V5
        // metadata drops inside the scope, before its request charge retires.
        b.reserve_storage(result.1.storage() + size_of::<Self>() + 32768)
            .unwrap();
        result
    }

    fn construct(
        f: &fixtures::Fixture,
        mismatched_invocation: bool,
        b: &mut Budget<'_>,
    ) -> (Self, compiler::PublicationInputs) {
        let directory = tempfile::Builder::new()
            .prefix("publication-")
            .tempdir_in(f.dir.path())
            .unwrap();
        let backend_path = f.dir.path().join("observation-backend");
        let backend_bytes = [0x5a; 4096];
        disk::write(&backend_path, backend_bytes).unwrap();
        fixtures::mode(&backend_path, 0o444);
        let backend_hash = Sha256::digest(backend_bytes).into();
        let image = fixtures::measurement(&f.compiler);
        let rustc_hash = image.sha256();
        // Non-executed closure roles are inert coordinates, not admitted runtime
        // images. Native observation really measures the two process image FDs.
        let closure = CompilerClosureV2::new(
            [0x11; 32],
            [0x22; 32],
            [0x33; 32],
            rustc_hash,
            [0x44; 32],
            backend_hash,
        )
        .unwrap();
        let producer = ProducerIdentity::from_codegen(
            "root_publication_observation",
            Some(Path::new("/fixture.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            directory.path(),
            &producer,
            BuildInvocation::from_bytes([3; 32]),
            BuildSession::from_bytes([4; 16]),
        )
        .unwrap();
        let environment = CompileEnvironmentV2::from_child_environment([
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from(BROKERED_ARTIFACT_DIRECTORY_PATH_V1),
            ),
            (
                OsString::from("FE2O3_TARGET"),
                OsString::from("gfx942:xnack-"),
            ),
            (
                OsString::from(CLOSURE_ENV),
                OsString::from(hex(closure.identity_sha256())),
            ),
            (
                OsString::from(BACKEND_ENV),
                OsString::from(hex(backend_hash)),
            ),
            (
                OsString::from(BUILD_ATTEMPT_ENV_V1),
                OsString::from(attempt.to_env_value()),
            ),
        ])
        .unwrap();
        let argv = vec![
            f.compiler.to_str().unwrap().to_owned(),
            "--crate-name".into(),
            "root_publication_observation".into(),
            "/fixture.rs".into(),
            format!("-Zcodegen-backend={BROKERED_CODEGEN_BACKEND_PATH_V1}"),
        ];
        let cwd = disk::canonicalize(f.dir.path()).unwrap();
        let descriptor = Descriptor::new(
            RustcInvocationDescriptorV2::new(
                rustc_hash,
                backend_hash,
                RustcUnitV2::new(cwd.to_str().unwrap(), argv.clone()).unwrap(),
                environment.clone(),
            )
            .unwrap(),
            closure,
        )
        .unwrap();
        assert_eq!(
            ProducerIdentity::from_rustc_invocation_descriptor_v3(&descriptor).unwrap(),
            producer
        );
        let published = if mismatched_invocation {
            changed_invocation(&descriptor)
        } else {
            descriptor.clone()
        };
        let handoff = reframe(&published, b);
        b.reserve_storage(handoff.backing_capacity() + METADATA)
            .unwrap();
        assert!(!handoff.grants_authority());
        let receipt =
            publish_compiler_module_handoff_v5(directory.path(), &producer, attempt, &handoff, b)
                .unwrap();
        let (subject, charge) = Subject::from_publication(receipt, &handoff, b).unwrap();
        b.reserve_storage(charge.retained_storage()).unwrap();
        let handoff_limit = handoff.canonical_bytes().len();
        let capability = RustcInvocationCapabilityV1::create(descriptor).unwrap();
        let inputs = compiler::PublicationInputs {
            argv: argv.into_iter().map(|s| CString::new(s).unwrap()).collect(),
            environment: environment
                .entries()
                .iter()
                .map(|e| CString::new(format!("{}={}", e.key(), e.value())).unwrap())
                .collect(),
            invocation: capability.try_clone_for_transfer().unwrap(),
            backend: File::open(backend_path).unwrap(),
            artifact: File::open(directory.path()).unwrap(),
        };
        (
            Self {
                directory,
                subject,
                handoff_limit,
            },
            inputs,
        )
    }
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn changed_invocation(original: &Descriptor) -> Descriptor {
    let mut argv: Vec<String> = original.rustc().argv().map(str::to_owned).collect();
    // Canonical invocation admission requires its authenticated backend last.
    argv.insert(argv.len() - 1, "--cfg=published_invocation_mismatch".into());
    Descriptor::new(
        RustcInvocationDescriptorV2::new(
            *original.rustc_executable_sha256(),
            *original.codegen_backend_sha256(),
            RustcUnitV2::new(original.rustc().working_directory(), argv).unwrap(),
            original.compile_environment().clone(),
        )
        .unwrap(),
        *original.compiler_closure(),
    )
    .unwrap()
}

// Reframe through public content codecs; never patch offsets or construct an
// observation, accepted proof, launch token, or protected compiler admission.
fn reframe(descriptor: &Descriptor, b: &mut Budget<'_>) -> Handoff {
    b.with_prepaid_scope(b.storage(), 0, 0, FIXTURE_STORAGE, |b| {
        Ok::<_, Resource>(reframe_contents(descriptor, b))
    })
    .unwrap()
}

fn reframe_contents(descriptor: &Descriptor, b: &mut Budget<'_>) -> Handoff {
    b.reserve_storage(TEMPLATE.len() + METADATA).unwrap();
    b.charge_work(decode_work(TEMPLATE.len()).unwrap()).unwrap();
    let template = Handoff::decode_owned(TEMPLATE.to_vec()).unwrap();
    let original = template.capsule();
    let invocation = encode_descriptor_v3(descriptor).unwrap();
    let input = MetadataInputs {
        invocation: &invocation,
        rustc_inventory: original.rustc_identity_inventory().canonical_preimage(),
        rustc_preflight: original.rustc_preflight_plan().canonical_preimage(),
        semantic_target_layout: original.semantic_target_layout_bytes(),
        native_lowering: original.native_lowering().canonical_bytes(),
        final_module_commitment: original.final_module_commitment_bytes(),
    };
    let layout = MetadataLayout::new::<Resource>(input).unwrap();
    let mut metadata = vec![0; layout.encoded_len()];
    for (range, bytes) in [
        (layout.invocation_range(), input.invocation),
        (layout.rustc_inventory_range(), input.rustc_inventory),
        (layout.rustc_preflight_range(), input.rustc_preflight),
        (
            layout.semantic_target_layout_range(),
            input.semantic_target_layout,
        ),
        (layout.native_lowering_range(), input.native_lowering),
        (
            layout.final_module_commitment_range(),
            input.final_module_commitment,
        ),
    ] {
        metadata[range].copy_from_slice(bytes);
    }
    seal_metadata(layout, &mut metadata, FIXTURE_STORAGE, |n| b.charge_work(n)).unwrap();
    let roster = original.policy_roster_bytes();
    let layout = EnvelopeLayout::new::<Resource>(metadata.len(), roster.len()).unwrap();
    let mut envelope = vec![0; layout.encoded_len()];
    envelope[layout.metadata_range()].copy_from_slice(&metadata);
    envelope[layout.policy_roster_range()].copy_from_slice(roster);
    seal_envelope(layout, &mut envelope, FIXTURE_STORAGE, |n| b.charge_work(n)).unwrap();
    let carrier = original.carrier_bytes();
    let layout = CapsuleLayout::new::<Resource>(envelope.len(), carrier.len()).unwrap();
    let mut capsule = vec![0; layout.encoded_len()];
    capsule[layout.metadata_range()].copy_from_slice(&envelope);
    capsule[layout.carrier_range()].copy_from_slice(carrier);
    let capsule_identity =
        seal_capsule(layout, &mut capsule, FIXTURE_STORAGE, |n| b.charge_work(n)).unwrap();
    let module = template.module_handoff();
    let layout = OuterLayout::new(capsule.len(), module.canonical_bytes().len()).unwrap();
    let mut bytes = vec![0; layout.encoded_len()];
    bytes[layout.capsule_range()].copy_from_slice(&capsule);
    bytes[layout.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    seal_outer(
        layout,
        &mut bytes,
        capsule_identity,
        module.identity(),
        |n| b.charge_work(n),
    )
    .unwrap();
    b.reserve_storage(bytes.capacity() + METADATA).unwrap();
    b.charge_work(decode_work(bytes.len()).unwrap()).unwrap();
    Handoff::decode_owned(bytes).unwrap()
}

#[test]
fn inert_reframing_changes_invocation_only_and_grants_no_authority() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(
        &mut work,
        fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5,
    );
    b.reserve_storage(FIXTURE_STORAGE).unwrap();
    b.charge_work(8192 * MAX_DESCRIPTOR_BYTES_V3).unwrap();
    b.charge_work(decode_work(TEMPLATE.len()).unwrap()).unwrap();
    b.reserve_storage(TEMPLATE.len() + METADATA).unwrap();
    let original = Handoff::decode_owned(TEMPLATE.to_vec()).unwrap();
    let descriptor = changed_invocation(original.capsule().invocation());
    let reframed = reframe(&descriptor, &mut b);
    b.reserve_storage(reframed.backing_capacity() + METADATA)
        .unwrap();
    assert_eq!(reframed.capsule().invocation(), &descriptor);
    assert_ne!(reframed.identity(), original.identity());
    assert_eq!(
        reframed.module_handoff().canonical_bytes(),
        original.module_handoff().canonical_bytes()
    );
    assert_eq!(
        reframed.capsule().carrier_bytes(),
        original.capsule().carrier_bytes()
    );
    assert_eq!(
        reframed.capsule().policy_roster_bytes(),
        original.capsule().policy_roster_bytes()
    );
    assert!(!reframed.grants_authority());
    assert!(!reframed.capsule().grants_authority());
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}
