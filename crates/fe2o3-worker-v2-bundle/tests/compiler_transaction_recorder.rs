use std::ffi::OsString;

use fe2o3_artifacts::IdentityText;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2, encode_descriptor_v2,
};
use fe2o3_worker_v2_bundle::{
    AlphaZetaSemanticLayoutWitnessesV1, CompilerTransactionRecorderErrorV1,
    CompilerTransactionRecorderV1, CompilerTransactionStageV1, ExactCompilerInvocationV1,
    ExactCompilerSourceClosureV1, ExactCompilerSourceFileV1, ExactCompilerToolV1,
    ExactSemanticLayoutWitnessV1, Gfx942CompilerTargetV1, MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1,
    SEALED_COMPILER_TRANSACTION_MAGIC_V1, SEALED_COMPILER_TRANSACTION_VERSION_V1,
    SealedCompilerTransactionDecodeErrorV1, SealedCompilerTransactionV1,
};
use sha2::{Digest, Sha256};

const RUSTC_BYTES: &[u8] = b"exact rustc executable";
const BACKEND_BYTES: &[u8] = b"exact rustc_codegen_fe2o3 executable";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-COMPILER-TRANSACTION/V1\0";
const HEADER_BYTES: usize = 16;
const FRESHNESS_OFFSET: usize = HEADER_BYTES;
const MEASUREMENTS_OFFSET: usize = FRESHNESS_OFFSET + 32;
const FINAL_CHAIN_OFFSET: usize = MEASUREMENTS_OFFSET + (17 * 32);

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn source(seed: u8, reverse: bool) -> ExactCompilerSourceClosureV1 {
    let root = ExactCompilerSourceFileV1::measure(
        text("src/lib.rs"),
        &[b"alpha and zeta kernel roots: ".as_slice(), &[seed]].concat(),
    )
    .unwrap();
    let mut dependencies = vec![
        ExactCompilerSourceFileV1::measure(text("src/zeta.rs"), b"zeta source").unwrap(),
        ExactCompilerSourceFileV1::measure(text("src/alpha.rs"), b"alpha source").unwrap(),
    ];
    let mut features = vec![text("verify"), text("worker-v2")];
    if !reverse {
        dependencies.reverse();
        features.reverse();
    }
    ExactCompilerSourceClosureV1::new(root, dependencies, features).unwrap()
}

fn rustc_descriptor(rustc_bytes: &[u8], backend_bytes: &[u8], target: &str) -> Vec<u8> {
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "alpha_zeta".into(),
            "src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .unwrap();
    let environment = CompileEnvironmentV2::from_child_environment([
        (
            OsString::from("CARGO_CFG_TARGET_ARCH"),
            OsString::from("amdgcn"),
        ),
        (
            OsString::from("FE2O3_HSACO_DIR"),
            OsString::from("/workspace/fe2o3/target/fe2o3"),
        ),
        (OsString::from("FE2O3_TARGET"), OsString::from(target)),
        (
            OsString::from("FE2O3_VERIFY_KERNEL_IR"),
            OsString::from("1"),
        ),
    ])
    .unwrap();
    let descriptor = RustcInvocationDescriptorV2::new(
        Sha256::digest(rustc_bytes).into(),
        Sha256::digest(backend_bytes).into(),
        rustc,
        environment,
    )
    .unwrap();
    encode_descriptor_v2(&descriptor).unwrap()
}

fn invocation(
    rustc_config_seed: u8,
    backend_config_seed: u8,
    backend_invocation_seed: u8,
) -> ExactCompilerInvocationV1 {
    let descriptor = rustc_descriptor(RUSTC_BYTES, BACKEND_BYTES, "gfx942:sramecc+:xnack-");
    let rustc_tool = ExactCompilerToolV1::measure(
        text("rustc"),
        text("1.94.0-nightly"),
        RUSTC_BYTES,
        &[rustc_config_seed],
    )
    .unwrap();
    let backend_tool = ExactCompilerToolV1::measure(
        text("rustc_codegen_fe2o3"),
        text("0.1.0"),
        BACKEND_BYTES,
        &[backend_config_seed],
    )
    .unwrap();
    ExactCompilerInvocationV1::measure(
        &descriptor,
        rustc_tool,
        backend_tool,
        &[backend_invocation_seed],
    )
    .unwrap()
}

fn target(invocation: &ExactCompilerInvocationV1, reverse: bool) -> Gfx942CompilerTargetV1 {
    let mut capabilities = vec![text("alpha-zeta"), text("code-object-v5"), text("wave64")];
    if reverse {
        capabilities.reverse();
    }
    Gfx942CompilerTargetV1::for_invocation(invocation, capabilities).unwrap()
}

fn witnesses(alpha: u8, zeta: u8, reverse: bool) -> AlphaZetaSemanticLayoutWitnessesV1 {
    let mut values = vec![
        ExactSemanticLayoutWitnessV1::measure(text("zeta"), &[zeta]).unwrap(),
        ExactSemanticLayoutWitnessV1::measure(text("alpha"), &[alpha]).unwrap(),
    ];
    if reverse {
        values.reverse();
    }
    AlphaZetaSemanticLayoutWitnessesV1::new(values).unwrap()
}

#[derive(Clone, Copy)]
struct Seeds {
    source: u8,
    rustc_config: u8,
    backend_config: u8,
    backend_invocation: u8,
    alpha_witness: u8,
    zeta_witness: u8,
    kernel_ir: u8,
    worker_request: u8,
    worker_response: u8,
    raw_hsaco: u8,
    finalized_hsaco: u8,
    descriptor_source: u8,
    finalized_descriptor: u8,
    artifact: u8,
}

impl Default for Seeds {
    fn default() -> Self {
        Self {
            source: 0x10,
            rustc_config: 0x20,
            backend_config: 0x21,
            backend_invocation: 0x22,
            alpha_witness: 0x30,
            zeta_witness: 0x31,
            kernel_ir: 0x40,
            worker_request: 0x50,
            worker_response: 0x51,
            raw_hsaco: 0x60,
            finalized_hsaco: 0x61,
            descriptor_source: 0x70,
            finalized_descriptor: 0x71,
            artifact: 0x72,
        }
    }
}

fn sealed_with_order(seeds: Seeds, freshness: u8, reverse: bool) -> SealedCompilerTransactionV1 {
    let (mut recorder, source_checkpoint) =
        CompilerTransactionRecorderV1::begin([freshness; 32], source(seeds.source, reverse))
            .unwrap();
    let invocation = invocation(
        seeds.rustc_config,
        seeds.backend_config,
        seeds.backend_invocation,
    );
    let target = target(&invocation, reverse);
    let compiler_checkpoint = recorder
        .record_compiler(source_checkpoint, invocation)
        .unwrap();
    let target_checkpoint = recorder.record_target(compiler_checkpoint, target).unwrap();
    let semantic_checkpoint = recorder
        .record_semantic_layouts(
            target_checkpoint,
            witnesses(seeds.alpha_witness, seeds.zeta_witness, reverse),
        )
        .unwrap();
    let ir_checkpoint = recorder
        .record_kernel_ir(semantic_checkpoint, &[seeds.kernel_ir])
        .unwrap();
    let worker_checkpoint = recorder
        .record_worker_exchange(
            ir_checkpoint,
            &[seeds.worker_request],
            &[seeds.worker_response],
        )
        .unwrap();
    let raw_checkpoint = recorder
        .record_raw_hsaco(worker_checkpoint, &[seeds.raw_hsaco])
        .unwrap();
    let finalized_checkpoint = recorder
        .record_finalized_artifact(
            raw_checkpoint,
            &[seeds.finalized_hsaco],
            &[seeds.descriptor_source],
            &[seeds.finalized_descriptor],
            &[seeds.artifact],
        )
        .unwrap();
    recorder.seal(finalized_checkpoint).unwrap()
}

fn sealed(seeds: Seeds) -> SealedCompilerTransactionV1 {
    sealed_with_order(seeds, 0x80, false)
}

fn resign_record(bytes: &mut [u8]) {
    let prefix_len = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(RECORD_IDENTITY_DOMAIN);
    digest.update(&bytes[..prefix_len]);
    let identity: [u8; 32] = digest.finalize().into();
    bytes[prefix_len..].copy_from_slice(&identity);
}

#[test]
fn exact_alpha_zeta_transaction_round_trips_canonically() {
    let sealed = sealed(Seeds::default());
    let bytes = sealed.to_bytes();
    let decoded = SealedCompilerTransactionV1::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, sealed);
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(&bytes[..8], &SEALED_COMPILER_TRANSACTION_MAGIC_V1);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        SEALED_COMPILER_TRANSACTION_VERSION_V1
    );
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize,
        bytes.len()
    );
    assert!(bytes.len() <= MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1);
    assert_eq!(
        SealedCompilerTransactionV1::from_bytes_for_identity(&bytes, sealed.identity()).unwrap(),
        sealed
    );
    assert_eq!(
        sealed
            .evidence_capsule()
            .source_closure()
            .dependencies()
            .len(),
        2
    );
    assert_eq!(
        sealed.evidence_capsule().source_closure().features().len(),
        2
    );
}

#[test]
fn unordered_source_capability_and_witness_sets_canonicalize_identically() {
    let first = sealed_with_order(Seeds::default(), 0x80, false);
    let second = sealed_with_order(Seeds::default(), 0x80, true);
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.to_bytes(), second.to_bytes());
}

#[test]
fn missing_reordered_duplicate_stale_and_mixed_stages_fail_closed() {
    let (mut first, first_source) =
        CompilerTransactionRecorderV1::begin([0x80; 32], source(0x10, false)).unwrap();
    let (second, second_source) =
        CompilerTransactionRecorderV1::begin([0x81; 32], source(0x10, false)).unwrap();
    let first_invocation = invocation(0x20, 0x21, 0x22);
    let early_target = target(&first_invocation, false);

    assert!(matches!(
        first.record_target(first_source, early_target),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
            expected: CompilerTransactionStageV1::Compiler,
            actual: CompilerTransactionStageV1::Source
        })
    ));
    assert!(matches!(
        first.seal(first_source),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
            expected: CompilerTransactionStageV1::FinalizedArtifact,
            actual: CompilerTransactionStageV1::Source
        })
    ));
    assert!(matches!(
        first.record_compiler(second_source, invocation(0x20, 0x21, 0x22)),
        Err(CompilerTransactionRecorderErrorV1::MixedTransaction)
    ));

    let compiler_checkpoint = first
        .record_compiler(first_source, first_invocation)
        .unwrap();
    assert!(matches!(
        first.record_compiler(first_source, invocation(0x20, 0x21, 0x22)),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
            expected: CompilerTransactionStageV1::Source,
            actual: CompilerTransactionStageV1::Compiler
        })
    ));
    assert!(matches!(
        first.record_target(first_source, target(&invocation(0x20, 0x21, 0x22), false)),
        Err(CompilerTransactionRecorderErrorV1::StaleCheckpoint)
    ));
    assert_eq!(
        compiler_checkpoint.stage(),
        CompilerTransactionStageV1::Compiler
    );
    drop(second);
}

#[test]
fn every_exact_input_changes_the_sealed_identity() {
    let baseline = sealed(Seeds::default()).identity();
    let variants = [
        Seeds {
            source: 1,
            ..Seeds::default()
        },
        Seeds {
            rustc_config: 1,
            ..Seeds::default()
        },
        Seeds {
            backend_config: 1,
            ..Seeds::default()
        },
        Seeds {
            backend_invocation: 1,
            ..Seeds::default()
        },
        Seeds {
            alpha_witness: 1,
            ..Seeds::default()
        },
        Seeds {
            zeta_witness: 1,
            ..Seeds::default()
        },
        Seeds {
            kernel_ir: 1,
            ..Seeds::default()
        },
        Seeds {
            worker_request: 1,
            ..Seeds::default()
        },
        Seeds {
            worker_response: 1,
            ..Seeds::default()
        },
        Seeds {
            raw_hsaco: 1,
            ..Seeds::default()
        },
        Seeds {
            finalized_hsaco: 1,
            ..Seeds::default()
        },
        Seeds {
            descriptor_source: 1,
            ..Seeds::default()
        },
        Seeds {
            finalized_descriptor: 1,
            ..Seeds::default()
        },
        Seeds {
            artifact: 1,
            ..Seeds::default()
        },
    ];
    for variant in variants {
        assert_ne!(sealed(variant).identity(), baseline);
    }
    assert_ne!(
        sealed_with_order(Seeds::default(), 0x81, false).identity(),
        baseline
    );
}

#[test]
fn rustc_descriptor_must_be_canonical_gfx942_and_match_exact_binaries() {
    let descriptor = rustc_descriptor(RUSTC_BYTES, BACKEND_BYTES, "gfx942");
    let rustc_tool = ExactCompilerToolV1::measure(
        text("rustc"),
        text("nightly"),
        b"different rustc",
        b"config",
    )
    .unwrap();
    let backend_tool =
        ExactCompilerToolV1::measure(text("backend"), text("v1"), BACKEND_BYTES, b"config")
            .unwrap();
    assert!(matches!(
        ExactCompilerInvocationV1::measure(
            &descriptor,
            rustc_tool,
            backend_tool,
            b"backend invocation"
        ),
        Err(CompilerTransactionRecorderErrorV1::RustcExecutableMismatch)
    ));

    let rustc_tool =
        ExactCompilerToolV1::measure(text("rustc"), text("nightly"), RUSTC_BYTES, b"config")
            .unwrap();
    let backend_tool =
        ExactCompilerToolV1::measure(text("backend"), text("v1"), b"different backend", b"config")
            .unwrap();
    assert!(matches!(
        ExactCompilerInvocationV1::measure(
            &descriptor,
            rustc_tool,
            backend_tool,
            b"backend invocation"
        ),
        Err(CompilerTransactionRecorderErrorV1::BackendExecutableMismatch)
    ));

    let unsupported = rustc_descriptor(RUSTC_BYTES, BACKEND_BYTES, "gfx950");
    let rustc_tool =
        ExactCompilerToolV1::measure(text("rustc"), text("nightly"), RUSTC_BYTES, b"config")
            .unwrap();
    let backend_tool =
        ExactCompilerToolV1::measure(text("backend"), text("v1"), BACKEND_BYTES, b"config")
            .unwrap();
    assert!(matches!(
        ExactCompilerInvocationV1::measure(
            &unsupported,
            rustc_tool,
            backend_tool,
            b"backend invocation"
        ),
        Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget)
    ));

    let mut malformed = descriptor;
    malformed[0] ^= 1;
    let rustc_tool =
        ExactCompilerToolV1::measure(text("rustc"), text("nightly"), RUSTC_BYTES, b"config")
            .unwrap();
    let backend_tool =
        ExactCompilerToolV1::measure(text("backend"), text("v1"), BACKEND_BYTES, b"config")
            .unwrap();
    assert!(matches!(
        ExactCompilerInvocationV1::measure(
            &malformed,
            rustc_tool,
            backend_tool,
            b"backend invocation"
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));
}

#[test]
fn source_target_and_alpha_zeta_sets_reject_ambiguity() {
    let root = ExactCompilerSourceFileV1::measure(text("src/lib.rs"), b"root").unwrap();
    let duplicate = ExactCompilerSourceFileV1::measure(text("src/a.rs"), b"one").unwrap();
    let other = ExactCompilerSourceFileV1::measure(text("src/a.rs"), b"two").unwrap();
    assert!(matches!(
        ExactCompilerSourceClosureV1::new(root.clone(), vec![duplicate, other], vec![]),
        Err(CompilerTransactionRecorderErrorV1::DuplicateSourcePath)
    ));
    assert!(matches!(
        ExactCompilerSourceClosureV1::new(root.clone(), vec![root], vec![]),
        Err(CompilerTransactionRecorderErrorV1::RootRepeatedAsDependency)
    ));
    assert!(matches!(
        ExactCompilerSourceClosureV1::new(
            ExactCompilerSourceFileV1::measure(text("src/lib.rs"), b"root").unwrap(),
            vec![],
            vec![text("same"), text("same")]
        ),
        Err(CompilerTransactionRecorderErrorV1::DuplicateFeature)
    ));

    let invocation = invocation(1, 2, 3);
    assert!(matches!(
        Gfx942CompilerTargetV1::for_invocation(&invocation, vec![text("same"), text("same")]),
        Err(CompilerTransactionRecorderErrorV1::DuplicateCapability)
    ));
    let alpha = ExactSemanticLayoutWitnessV1::measure(text("alpha"), b"a").unwrap();
    assert!(matches!(
        AlphaZetaSemanticLayoutWitnessesV1::new(vec![alpha.clone(), alpha]),
        Err(CompilerTransactionRecorderErrorV1::DuplicateSemanticWitness)
    ));
    assert!(matches!(
        AlphaZetaSemanticLayoutWitnessesV1::new(vec![
            ExactSemanticLayoutWitnessV1::measure(text("alpha"), b"a").unwrap()
        ]),
        Err(CompilerTransactionRecorderErrorV1::MissingAlphaZetaWitnesses)
    ));
}

#[test]
fn truncation_trailing_header_mutation_and_reserved_zero_are_rejected() {
    let bytes = sealed(Seeds::default()).to_bytes();
    for length in 0..bytes.len() {
        assert!(
            SealedCompilerTransactionV1::from_bytes(&bytes[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&trailing),
        Err(SealedCompilerTransactionDecodeErrorV1::TrailingBytes)
    ));
    let mut magic = bytes.clone();
    magic[0] ^= 1;
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&magic),
        Err(SealedCompilerTransactionDecodeErrorV1::InvalidMagic)
    ));
    let mut version = bytes.clone();
    version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&version),
        Err(SealedCompilerTransactionDecodeErrorV1::UnknownVersion(2))
    ));
    let mut flags = bytes.clone();
    flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&flags),
        Err(SealedCompilerTransactionDecodeErrorV1::UnsupportedFlags(1))
    ));
    let mut zero = bytes;
    zero[MEASUREMENTS_OFFSET..MEASUREMENTS_OFFSET + 32].fill(0);
    resign_record(&mut zero);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&zero),
        Err(
            SealedCompilerTransactionDecodeErrorV1::ReservedZeroIdentity {
                field: "source tree"
            }
        )
    ));
}

#[test]
fn mutations_forged_measurements_and_checkpoint_changes_fail_closed() {
    let sealed = sealed(Seeds::default());
    let mut mutation = sealed.to_bytes();
    mutation[MEASUREMENTS_OFFSET] ^= 1;
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&mutation),
        Err(SealedCompilerTransactionDecodeErrorV1::RecordIdentityMismatch)
    ));

    let mut swapped = sealed.to_bytes();
    let rustc = MEASUREMENTS_OFFSET + (2 * 32);
    let backend = MEASUREMENTS_OFFSET + (4 * 32);
    for index in 0..32 {
        swapped.swap(rustc + index, backend + index);
    }
    resign_record(&mut swapped);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&swapped),
        Err(SealedCompilerTransactionDecodeErrorV1::MeasurementMismatch { .. })
    ));

    let mut supplemental = sealed.to_bytes();
    supplemental[MEASUREMENTS_OFFSET + 32] ^= 1;
    resign_record(&mut supplemental);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&supplemental),
        Err(SealedCompilerTransactionDecodeErrorV1::CheckpointMismatch)
    ));

    let mut chain = sealed.to_bytes();
    chain[FINAL_CHAIN_OFFSET] ^= 1;
    resign_record(&mut chain);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&chain),
        Err(SealedCompilerTransactionDecodeErrorV1::CheckpointMismatch)
    ));
}

#[test]
fn stale_valid_record_is_rejected_by_expected_identity() {
    let current = sealed(Seeds::default());
    let stale = sealed(Seeds {
        kernel_ir: 1,
        ..Seeds::default()
    });
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes_for_identity(&stale.to_bytes(), current.identity()),
        Err(SealedCompilerTransactionDecodeErrorV1::UnexpectedRecordIdentity)
    ));
}

#[test]
fn sealed_record_and_capsule_are_explicitly_inert() {
    let sealed = sealed(Seeds::default());
    assert!(!sealed.authenticates_producer());
    assert!(!sealed.grants_publication_authority());
    assert!(!sealed.grants_load_authority());
    assert!(!sealed.grants_launch_authority());
    assert!(!sealed.evidence_capsule().authenticates_producer());
    assert!(!sealed.evidence_capsule().grants_publication_authority());
    assert!(!sealed.evidence_capsule().grants_load_authority());
    assert!(!sealed.evidence_capsule().grants_launch_authority());
}

#[test]
fn finalized_descriptor_must_be_distinct_and_freshness_nonzero() {
    assert!(matches!(
        CompilerTransactionRecorderV1::begin([0; 32], source(1, false)),
        Err(CompilerTransactionRecorderErrorV1::ReservedZeroFreshness)
    ));

    let (mut recorder, source_checkpoint) =
        CompilerTransactionRecorderV1::begin([1; 32], source(1, false)).unwrap();
    let invocation = invocation(2, 3, 4);
    let target = target(&invocation, false);
    let compiler = recorder
        .record_compiler(source_checkpoint, invocation)
        .unwrap();
    let target_checkpoint = recorder.record_target(compiler, target).unwrap();
    let semantic = recorder
        .record_semantic_layouts(target_checkpoint, witnesses(5, 6, false))
        .unwrap();
    let ir = recorder.record_kernel_ir(semantic, b"ir").unwrap();
    let worker = recorder
        .record_worker_exchange(ir, b"request", b"response")
        .unwrap();
    let raw = recorder.record_raw_hsaco(worker, b"raw").unwrap();
    assert!(matches!(
        recorder.record_finalized_artifact(raw, b"final", b"same", b"same", b"artifact"),
        Err(CompilerTransactionRecorderErrorV1::DescriptorNotFinalized)
    ));
}
