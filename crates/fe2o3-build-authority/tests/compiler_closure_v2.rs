#[path = "../src/compiler_closure.rs"]
#[allow(dead_code)]
mod compiler_closure;

use compiler_closure::{
    CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1, COMPILER_CLOSURE_IDENTITY_DOMAIN_V1,
    COMPILER_CLOSURE_IDENTITY_DOMAIN_V2, CompilerClosureDigestFieldV2, CompilerClosureErrorV2,
    CompilerClosureV1, CompilerClosureV2, derive_compiler_closure_identity_v2,
};

const PINS: [[u8; 32]; 6] = [
    [0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32], [0x55; 32], [0x66; 32],
];

fn closure(pins: [[u8; 32]; 6]) -> Result<CompilerClosureV2, CompilerClosureErrorV2> {
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5])
}

fn declared(
    pins: [[u8; 32]; 6],
    transition_protocol_version: u16,
    identity: [u8; 32],
) -> Result<CompilerClosureV2, CompilerClosureErrorV2> {
    CompilerClosureV2::from_pins_and_identity(
        pins[0],
        pins[1],
        pins[2],
        pins[3],
        pins[4],
        pins[5],
        transition_protocol_version,
        identity,
    )
}

#[test]
fn v2_domain_protocol_and_golden_identity_are_stable() {
    let closure = closure(PINS).unwrap();

    assert_eq!(
        COMPILER_CLOSURE_IDENTITY_DOMAIN_V2,
        b"fe2o3-compiler-closure-identity-v2\0"
    );
    assert_eq!(CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1, 1);
    assert_eq!(closure.cargo_executable_sha256(), PINS[0]);
    assert_eq!(closure.cargo_binding_trampoline_sha256(), PINS[1]);
    assert_eq!(closure.cargo_fe2o3_binding_wrapper_sha256(), PINS[2]);
    assert_eq!(closure.rustc_executable_sha256(), PINS[3]);
    assert_eq!(closure.rustc_runtime_tree_sha256(), PINS[4]);
    assert_eq!(closure.codegen_backend_sha256(), PINS[5]);
    assert_eq!(closure.cargo_binding_transition_protocol_version(), 1);
    assert_eq!(
        closure.identity_sha256(),
        [
            0x9c, 0x28, 0x98, 0x53, 0x25, 0x45, 0xab, 0xbc, 0x57, 0x7c, 0x9d, 0x6f, 0x20, 0x2e,
            0x7e, 0x31, 0x82, 0xee, 0x79, 0x5e, 0xc8, 0x87, 0xfc, 0xb0, 0x54, 0x0e, 0xb4, 0x10,
            0x71, 0x96, 0x77, 0xf9,
        ]
    );
    assert_eq!(
        closure.identity_sha256(),
        derive_compiler_closure_identity_v2(
            PINS[0], PINS[1], PINS[2], PINS[3], PINS[4], PINS[5], 1,
        )
    );
}

#[test]
fn every_content_pin_and_the_transition_protocol_are_identity_bound() {
    let baseline = closure(PINS).unwrap().identity_sha256();

    for index in 0..PINS.len() {
        let mut changed = PINS;
        changed[index][index] ^= 0x80;
        assert_ne!(closure(changed).unwrap().identity_sha256(), baseline);
    }
    assert_ne!(
        derive_compiler_closure_identity_v2(
            PINS[0], PINS[1], PINS[2], PINS[3], PINS[4], PINS[5], 2,
        ),
        baseline
    );
}

#[test]
fn noncanonical_protocol_zero_pins_and_mismatched_identity_fail_closed() {
    let identity = closure(PINS).unwrap().identity_sha256();
    assert_eq!(
        declared(PINS, 2, identity),
        Err(CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version: 2 })
    );

    for (index, field) in [
        CompilerClosureDigestFieldV2::CargoExecutable,
        CompilerClosureDigestFieldV2::CargoBindingTrampoline,
        CompilerClosureDigestFieldV2::CargoFe2o3BindingWrapper,
        CompilerClosureDigestFieldV2::RustcExecutable,
        CompilerClosureDigestFieldV2::RustcRuntimeTree,
        CompilerClosureDigestFieldV2::CodegenBackend,
    ]
    .into_iter()
    .enumerate()
    {
        let mut pins = PINS;
        pins[index] = [0; 32];
        assert_eq!(
            closure(pins),
            Err(CompilerClosureErrorV2::ZeroDigest { field })
        );
    }

    assert_eq!(
        declared(PINS, 1, [0; 32]),
        Err(CompilerClosureErrorV2::ZeroDigest {
            field: CompilerClosureDigestFieldV2::CompilerClosure,
        })
    );
    let mut mismatched = identity;
    mismatched[0] ^= 1;
    assert_eq!(
        declared(PINS, 1, mismatched),
        Err(CompilerClosureErrorV2::IdentityMismatch)
    );
}

#[test]
fn v1_domain_and_shared_golden_identity_are_unchanged() {
    assert_eq!(
        COMPILER_CLOSURE_IDENTITY_DOMAIN_V1,
        b"fe2o3-compiler-closure-identity-v1\0"
    );
    assert_eq!(
        CompilerClosureV1::new([0x05; 32], [0x06; 32], [0x07; 32], [0x08; 32])
            .unwrap()
            .identity_sha256(),
        [
            0x1f, 0xea, 0xcf, 0xc5, 0x87, 0x9b, 0x85, 0x3c, 0x7b, 0xa5, 0x5c, 0x34, 0x53, 0x93,
            0x98, 0xe8, 0x57, 0xc0, 0xf9, 0x7d, 0x68, 0x6c, 0xbb, 0x63, 0xcf, 0x99, 0x79, 0x5a,
            0x6a, 0xa0, 0x9e, 0xc9,
        ]
    );
}
