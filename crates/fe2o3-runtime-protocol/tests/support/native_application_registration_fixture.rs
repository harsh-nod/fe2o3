//! Codec-only signed records and a static ELF image. No service or native authority.
//! Import with a sibling `protocol` alias for fe2o3-runtime-protocol (or crate itself).
use super::protocol;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as AttestationStorage,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceLaunchManifestV3 as Launch,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use protocol::{
    NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationRegistrationInputsV1 as Inputs,
    NativeConditionalApplicationBindingV1 as Association,
    WorkerV3ApplicationHandoffChallengeV1 as Challenge, WorkerV3ApplicationIdentityV1 as Image,
    WorkerV3ApplicationInputOccurrenceV1 as Input, WorkerV3ApplicationOccurrenceV1 as Occurrence,
    WorkerV3ApplicationRegistrationDescriptorsV1 as Descriptors,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod attestation;
use self::attestation as fixture;
#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt;

fn retain<T, E: fmt::Debug>(result: Result<(T, AttestationStorage), E>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}
pub fn static_image() -> Vec<u8> {
    let mut image = vec![0; 4097];
    image[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (at, value) in [(16, 2_u16), (18, 62), (52, 64), (54, 56), (56, 4)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    image[20..24].copy_from_slice(&1_u32.to_le_bytes());
    image[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    image[32..40].copy_from_slice(&64_u64.to_le_bytes());
    for (index, (kind, flags, offset, address, size, alignment)) in [
        (6_u32, 4_u32, 64_u64, 0x400040_u64, 224_u64, 8_u64),
        (1, 4, 0, 0x400000, 288, 4096),
        (1, 5, 4096, 0x401000, 1, 4096),
        (0x6474e551, 6, 0, 0, 0, 16),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 64 + index * 56;
        image[at..at + 4].copy_from_slice(&kind.to_le_bytes());
        image[at + 4..at + 8].copy_from_slice(&flags.to_le_bytes());
        for (delta, value) in [
            (8, offset),
            (16, address),
            (32, size),
            (40, size),
            (48, alignment),
        ] {
            image[at + delta..at + delta + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    image[4096] = 0xc3;
    image
}
fn readiness(carriage: &Carriage) -> Vec<u8> {
    let fields = [
        b"record".as_slice(),
        b"claim",
        b"outer",
        b"transcript",
        carriage.canonical_bytes(),
    ];
    let length = 44 + fields.iter().map(|v| v.len()).sum::<usize>() + 32;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(b"F3CENV05");
    bytes.extend_from_slice(&5u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(length as u64).to_le_bytes());
    for value in fields {
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    for value in fields {
        bytes.extend_from_slice(value);
    }
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-WORKER-READINESS/V5\0");
    hash.update(&bytes);
    bytes.extend_from_slice(&hash.finalize());
    bytes
}
pub fn binding(app: u32, cargo: u32, uid: u32, gid: u32, b: &mut Budget<'_>) -> Binding {
    binding_and_readiness(app, cargo, uid, gid, b).0
}
pub fn binding_and_readiness(
    app: u32,
    cargo: u32,
    uid: u32,
    gid: u32,
    b: &mut Budget<'_>,
) -> (Binding, Vec<u8>) {
    let p = attestation::policy_wire(3);
    let r = attestation::request_wire(3);
    let s = receipt::receipt_wire(3);
    b.reserve_storage(p.len() + r.len() + s.len()).unwrap();
    let policy = retain(Policy::decode(&p, b), b);
    let request = retain(Request::decode(&r, b), b);
    let receipt = retain(Receipt::decode(&s, b), b);
    let publication = retain(Publication::new([0x81; 32], [0x82; 32], receipt, b), b);
    let ack = retain(Ack::new(&publication, [0x83; 32], b), b);
    let carriage = retain(Carriage::new(policy, request, publication, ack, b), b);
    let policy = retain(Policy::decode(&p, b), b);
    let launch = retain(
        Launch::new(
            Client::new(app, uid, gid).unwrap(),
            Service::new(2001, 2001).unwrap(),
            &policy,
            b,
        ),
        b,
    );
    let handoff = retain(
        Handoff::new(Client::new(cargo, uid, gid).unwrap(), launch, b),
        b,
    );
    let readiness = readiness(&carriage);
    b.reserve_storage(readiness.len()).unwrap();
    let association = Association::bind(&readiness, &handoff, &carriage, b).unwrap();
    b.reserve_storage(size_of::<Association>()).unwrap();
    let image = static_image();
    b.reserve_storage(image.len() + size_of::<Occurrence>() + 4 * size_of::<Input>())
        .unwrap();
    let occurrence = Occurrence::new(
        Image::from_sealed_static_elf_v1(&image).unwrap(),
        [10; 32],
        &(1..=4)
            .map(|slot| Input::new(slot, [slot as u8; 32]).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let (inputs, charge) = Inputs::new(
        &readiness,
        occurrence,
        Descriptors::new(10, 11, 12, 13).unwrap(),
        Challenge::from_bytes([9; 32]).unwrap(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (binding, charge) = Binding::new(handoff, association, inputs, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    (binding, readiness)
}
