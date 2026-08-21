use std::ffi::OsString;

use fe2o3_artifacts::IdentityText;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, INVOCATION_DIGEST_DOMAIN_V3, InvocationDigestV2, InvocationDigestV3,
    MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
    encode_descriptor_v2, encode_descriptor_v3,
};
use fe2o3_worker_v2_bundle::{
    CompilerTransactionRecorderErrorV1, ExactCompilerInvocationV1, ExactCompilerInvocationV2,
    ExactCompilerToolV1, ExactWorkerToolV1,
};
use sha2::{Digest, Sha256};

const RUSTC_BYTES: &[u8] = b"protected rustc executable";
const BACKEND_BYTES: &[u8] = b"protected rustc_codegen_fe2o3 executable";
const BACKEND_INVOCATION_BYTES: &[u8] = b"canonical protected backend invocation";
const GFX942_XNACK_MINUS: &str = "gfx942:xnack-";

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn closure_for(rustc_bytes: &[u8], backend_bytes: &[u8]) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        digest(rustc_bytes),
        [0x55; 32],
        digest(backend_bytes),
    )
    .unwrap()
}

fn descriptor_v2(
    target: &str,
    rustc_sha256: [u8; 32],
    backend_sha256: [u8; 32],
) -> RustcInvocationDescriptorV2 {
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "protected_fixture".into(),
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
    RustcInvocationDescriptorV2::new(rustc_sha256, backend_sha256, rustc, environment).unwrap()
}

fn descriptor_v3(target: &str, closure: CompilerClosureV2) -> RustcInvocationDescriptorV3 {
    RustcInvocationDescriptorV3::new(
        descriptor_v2(
            target,
            closure.rustc_executable_sha256(),
            closure.codegen_backend_sha256(),
        ),
        closure,
    )
    .unwrap()
}

fn exact_tool(name: &str, bytes: &[u8], configuration: u8) -> ExactCompilerToolV1 {
    ExactCompilerToolV1::measure(text(name), text("test"), bytes, &[configuration]).unwrap()
}

fn exact_worker() -> ExactWorkerToolV1 {
    ExactWorkerToolV1::measure(
        b"protected worker executable",
        text("worker-build"),
        text("llvm-build"),
        text("rustc-codegen-fe2o3-worker-v2"),
        text("test"),
    )
    .unwrap()
}

fn measure(
    descriptor: &RustcInvocationDescriptorV3,
    rustc_bytes: &[u8],
    backend_bytes: &[u8],
    backend_invocation_bytes: &[u8],
) -> Result<ExactCompilerInvocationV2, CompilerTransactionRecorderErrorV1> {
    ExactCompilerInvocationV2::measure(
        &encode_descriptor_v3(descriptor).unwrap(),
        exact_tool("rustc", rustc_bytes, 0x41),
        exact_tool("rustc-codegen-fe2o3", backend_bytes, 0x42),
        exact_worker(),
        backend_invocation_bytes,
    )
}

#[test]
fn canonical_v3_is_bounded_measured_and_exposes_the_full_closure() {
    let closure = closure_for(RUSTC_BYTES, BACKEND_BYTES);
    let descriptor = descriptor_v3(GFX942_XNACK_MINUS, closure);
    let encoded = encode_descriptor_v3(&descriptor).unwrap();
    let measured = measure(
        &descriptor,
        RUSTC_BYTES,
        BACKEND_BYTES,
        BACKEND_INVOCATION_BYTES,
    )
    .unwrap();

    assert!(encoded.len() <= MAX_DESCRIPTOR_BYTES_V3);
    assert_eq!(
        measured.rustc_descriptor_identity().as_bytes(),
        &digest(&encoded)
    );
    assert_eq!(
        measured.backend_invocation_identity().as_bytes(),
        &digest(BACKEND_INVOCATION_BYTES)
    );
    assert_eq!(
        measured.rustc_invocation_identity(),
        InvocationDigestV3::calculate(&descriptor).unwrap()
    );
    assert_eq!(measured.compiler_closure(), closure);
    assert_eq!(
        measured.compiler_closure().cargo_executable_sha256(),
        [0x11; 32]
    );
    assert_eq!(
        measured
            .compiler_closure()
            .cargo_binding_trampoline_sha256(),
        [0x22; 32]
    );
    assert_eq!(
        measured
            .compiler_closure()
            .cargo_fe2o3_binding_wrapper_sha256(),
        [0x33; 32]
    );
    assert_eq!(
        measured.compiler_closure().rustc_executable_sha256(),
        digest(RUSTC_BYTES)
    );
    assert_eq!(
        measured.compiler_closure().rustc_runtime_tree_sha256(),
        [0x55; 32]
    );
    assert_eq!(
        measured.compiler_closure().codegen_backend_sha256(),
        digest(BACKEND_BYTES)
    );
    assert_eq!(
        measured
            .compiler_closure()
            .cargo_binding_transition_protocol_version(),
        1
    );
    assert_eq!(
        measured.compiler_closure().identity_sha256(),
        closure.identity_sha256()
    );
    assert_eq!(measured.amd_target(), GFX942_XNACK_MINUS);
    assert_eq!(
        measured.worker_tool().executable_identity().as_bytes(),
        &digest(b"protected worker executable")
    );

    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &[],
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::EmptyInput {
            field: "rustc invocation descriptor"
        })
    ));
    let oversized = vec![0; MAX_DESCRIPTOR_BYTES_V3 + 1];
    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &oversized,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));
}

#[test]
fn only_complete_canonical_v3_bytes_are_accepted() {
    let descriptor = descriptor_v3(GFX942_XNACK_MINUS, closure_for(RUSTC_BYTES, BACKEND_BYTES));
    let encoded = encode_descriptor_v3(&descriptor).unwrap();

    for length in 0..encoded.len() {
        assert!(matches!(
            ExactCompilerInvocationV2::measure(
                &encoded[..length],
                exact_tool("rustc", RUSTC_BYTES, 0x41),
                exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
                exact_worker(),
                BACKEND_INVOCATION_BYTES,
            ),
            Err(CompilerTransactionRecorderErrorV1::EmptyInput { .. })
                | Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
        ));
    }

    for offset in [10, 16] {
        let mut noncanonical = encoded.clone();
        noncanonical[offset] = 1;
        assert!(matches!(
            ExactCompilerInvocationV2::measure(
                &noncanonical,
                exact_tool("rustc", RUSTC_BYTES, 0x41),
                exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
                exact_worker(),
                BACKEND_INVOCATION_BYTES,
            ),
            Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
        ));
    }

    let mut trailing = encoded;
    trailing.push(0);
    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &trailing,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));
}

#[test]
fn v1_and_v2_descriptors_cannot_be_downgraded_or_cross_admitted() {
    let closure = closure_for(RUSTC_BYTES, BACKEND_BYTES);
    let descriptor_v2 = descriptor_v2(
        GFX942_XNACK_MINUS,
        closure.rustc_executable_sha256(),
        closure.codegen_backend_sha256(),
    );
    let encoded_v2 = encode_descriptor_v2(&descriptor_v2).unwrap();
    let descriptor_v3 = RustcInvocationDescriptorV3::new(descriptor_v2, closure).unwrap();
    let encoded_v3 = encode_descriptor_v3(&descriptor_v3).unwrap();

    assert!(
        ExactCompilerInvocationV1::measure(
            &encoded_v2,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        )
        .is_ok()
    );
    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &encoded_v2,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));
    assert!(matches!(
        ExactCompilerInvocationV1::measure(
            &encoded_v3,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));

    let mut downgraded_header = encoded_v3;
    downgraded_header[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &downgraded_header,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
    ));
}

#[test]
fn every_compiler_closure_role_is_bound_by_v3_measurement() {
    let baseline_closure = closure_for(RUSTC_BYTES, BACKEND_BYTES);
    let baseline = measure(
        &descriptor_v3(GFX942_XNACK_MINUS, baseline_closure),
        RUSTC_BYTES,
        BACKEND_BYTES,
        BACKEND_INVOCATION_BYTES,
    )
    .unwrap();

    for role in 0..6 {
        let mut rustc_bytes = RUSTC_BYTES.to_vec();
        let mut backend_bytes = BACKEND_BYTES.to_vec();
        let mut pins = [
            baseline_closure.cargo_executable_sha256(),
            baseline_closure.cargo_binding_trampoline_sha256(),
            baseline_closure.cargo_fe2o3_binding_wrapper_sha256(),
            baseline_closure.rustc_executable_sha256(),
            baseline_closure.rustc_runtime_tree_sha256(),
            baseline_closure.codegen_backend_sha256(),
        ];
        match role {
            3 => {
                rustc_bytes.push(0x83);
                pins[role] = digest(&rustc_bytes);
            }
            5 => {
                backend_bytes.push(0x85);
                pins[role] = digest(&backend_bytes);
            }
            _ => pins[role][role] ^= 0x80,
        }
        let changed_closure =
            CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap();
        let changed = measure(
            &descriptor_v3(GFX942_XNACK_MINUS, changed_closure),
            &rustc_bytes,
            &backend_bytes,
            BACKEND_INVOCATION_BYTES,
        )
        .unwrap();

        assert_eq!(changed.compiler_closure(), changed_closure, "role {role}");
        assert_ne!(
            changed.compiler_closure().identity_sha256(),
            baseline.compiler_closure().identity_sha256(),
            "role {role} did not affect closure identity"
        );
        assert_ne!(
            changed.rustc_descriptor_identity(),
            baseline.rustc_descriptor_identity(),
            "role {role} did not affect descriptor measurement"
        );
        assert_ne!(
            changed.rustc_invocation_identity(),
            baseline.rustc_invocation_identity(),
            "role {role} did not affect V3 invocation digest"
        );
    }
}

#[test]
fn descriptor_and_measured_tool_pin_mismatches_fail_closed() {
    let descriptor = descriptor_v3(GFX942_XNACK_MINUS, closure_for(RUSTC_BYTES, BACKEND_BYTES));
    let encoded = encode_descriptor_v3(&descriptor).unwrap();

    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &encoded,
            exact_tool("rustc", b"wrong rustc", 0x41),
            exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::RustcExecutableMismatch)
    ));
    assert!(matches!(
        ExactCompilerInvocationV2::measure(
            &encoded,
            exact_tool("rustc", RUSTC_BYTES, 0x41),
            exact_tool("rustc-codegen-fe2o3", b"wrong backend", 0x42),
            exact_worker(),
            BACKEND_INVOCATION_BYTES,
        ),
        Err(CompilerTransactionRecorderErrorV1::BackendExecutableMismatch)
    ));

    const CLOSURE_OFFSET: usize = 20;
    const V2_BODY_OFFSET: usize = CLOSURE_OFFSET + 2 + 6 * 32;
    for offset in [
        CLOSURE_OFFSET + 2 + 3 * 32,
        CLOSURE_OFFSET + 2 + 5 * 32,
        V2_BODY_OFFSET,
        V2_BODY_OFFSET + 32,
    ] {
        let mut mismatched = encoded.clone();
        mismatched[offset] ^= 1;
        assert!(matches!(
            ExactCompilerInvocationV2::measure(
                &mismatched,
                exact_tool("rustc", RUSTC_BYTES, 0x41),
                exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x42),
                exact_worker(),
                BACKEND_INVOCATION_BYTES,
            ),
            Err(CompilerTransactionRecorderErrorV1::InvalidRustcDescriptor)
        ));
    }
}

#[test]
fn only_exact_gfx942_xnack_minus_is_admitted() {
    for target in ["gfx942", "gfx942:xnack+", "gfx942:sramecc+:xnack-"] {
        let descriptor = descriptor_v3(target, closure_for(RUSTC_BYTES, BACKEND_BYTES));
        assert!(matches!(
            measure(
                &descriptor,
                RUSTC_BYTES,
                BACKEND_BYTES,
                BACKEND_INVOCATION_BYTES,
            ),
            Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget)
        ));
    }

    assert!(
        measure(
            &descriptor_v3(GFX942_XNACK_MINUS, closure_for(RUSTC_BYTES, BACKEND_BYTES),),
            RUSTC_BYTES,
            BACKEND_BYTES,
            BACKEND_INVOCATION_BYTES,
        )
        .is_ok()
    );
}

#[test]
fn v3_digest_is_recomputed_and_backend_invocation_has_an_independent_measurement() {
    let closure = closure_for(RUSTC_BYTES, BACKEND_BYTES);
    let descriptor = descriptor_v3(GFX942_XNACK_MINUS, closure);
    let encoded = encode_descriptor_v3(&descriptor).unwrap();
    let measured = measure(
        &descriptor,
        RUSTC_BYTES,
        BACKEND_BYTES,
        BACKEND_INVOCATION_BYTES,
    )
    .unwrap();

    let mut digest_transcript = Sha256::new();
    digest_transcript.update(INVOCATION_DIGEST_DOMAIN_V3);
    digest_transcript.update((encoded.len() as u64).to_le_bytes());
    digest_transcript.update(&encoded);
    assert_eq!(
        measured.rustc_invocation_identity().as_bytes(),
        &<[u8; 32]>::from(digest_transcript.finalize())
    );
    assert_ne!(
        measured.rustc_invocation_identity().as_bytes(),
        InvocationDigestV2::calculate(descriptor.descriptor_v2())
            .unwrap()
            .as_bytes()
    );

    let changed_backend_invocation = measure(
        &descriptor,
        RUSTC_BYTES,
        BACKEND_BYTES,
        b"different protected backend invocation",
    )
    .unwrap();
    assert_eq!(
        changed_backend_invocation.rustc_invocation_identity(),
        measured.rustc_invocation_identity()
    );
    assert_eq!(
        changed_backend_invocation.rustc_descriptor_identity(),
        measured.rustc_descriptor_identity()
    );
    assert_eq!(
        changed_backend_invocation.compiler_closure(),
        measured.compiler_closure()
    );
    assert_ne!(
        changed_backend_invocation.backend_invocation_identity(),
        measured.backend_invocation_identity()
    );

    assert!(matches!(
        measure(&descriptor, RUSTC_BYTES, BACKEND_BYTES, &[]),
        Err(CompilerTransactionRecorderErrorV1::EmptyInput {
            field: "backend invocation"
        })
    ));
}
