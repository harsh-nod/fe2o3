mod common;

use common::*;
use fe2o3_semantic_trace::*;

fn header_v2(version: KernelIrWireVersionV2) -> TraceHeaderV2 {
    let baseline = header(TraceCompletenessV1::Complete);
    TraceHeaderV2::new(
        baseline.producer().clone(),
        baseline.execution_kind(),
        KernelIrIdentityClaimV2::exact_canonical_claim(version, identity(9), 4_096).unwrap(),
        baseline.semantic_mir(),
        baseline.lineage(),
        baseline.artifact(),
        baseline.dispatch(),
        baseline.launch(),
        baseline.bounds(),
        baseline.completeness(),
        baseline.boundaries(),
    )
    .unwrap()
}

fn sample_trace_v2(version: KernelIrWireVersionV2) -> TraceEnvelopeV2 {
    TraceEnvelopeV2::new(header_v2(version), sample_events()).unwrap()
}

#[test]
fn exact_v9_and_v10_claims_round_trip_canonically() {
    for version in [KernelIrWireVersionV2::V9, KernelIrWireVersionV2::V10] {
        let trace = sample_trace_v2(version);
        let first = encode_trace_v2(&trace).unwrap();
        let decoded = decode_trace_v2(&first).unwrap();
        assert_eq!(decoded, trace);
        assert_eq!(encode_trace_v2(&decoded).unwrap(), first);
        assert_eq!(&first[..10], b"FE2O3TR2\x02\x00");
        assert_eq!(decoded.header().kernel_ir_claim().wire_version(), version);
        assert_eq!(
            decoded.header().kernel_ir_claim().digest().as_bytes(),
            &[9; 32]
        );
        assert_eq!(decoded.header().kernel_ir_claim().canonical_len(), 4_096);
    }
}

#[test]
fn v1_and_v2_envelopes_are_not_cross_decoded() {
    assert_eq!(
        decode_trace_v1(&encode_trace_v2(&sample_trace_v2(KernelIrWireVersionV2::V10)).unwrap()),
        Err(TraceDecodeErrorV1::InvalidMagic)
    );
    assert_eq!(
        decode_trace_v2(&encode_trace_v1(&sample_trace()).unwrap()),
        Err(TraceDecodeErrorV1::InvalidMagic)
    );
}

#[test]
fn v2_rejects_v7_unknown_policy_zero_identity_and_trailing_bytes() {
    let bytes = encode_trace_v2(&sample_trace_v2(KernelIrWireVersionV2::V10)).unwrap();
    let claim = claim_offset(&bytes);

    let mut v7 = bytes.clone();
    v7[claim..claim + 2].copy_from_slice(&KERNEL_IR_WIRE_VERSION_V7.to_le_bytes());
    assert_eq!(
        decode_trace_v2(&v7),
        Err(TraceDecodeErrorV1::UnsupportedKernelIrClaim {
            wire_version: KERNEL_IR_WIRE_VERSION_V7,
            identity_policy: KERNEL_IR_IDENTITY_POLICY_V1,
        })
    );

    let mut policy = bytes.clone();
    policy[claim + 2..claim + 4].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_trace_v2(&policy),
        Err(TraceDecodeErrorV1::UnsupportedKernelIrClaim {
            wire_version: KERNEL_IR_WIRE_VERSION_V10,
            identity_policy: 2,
        })
    );

    let mut zero = bytes.clone();
    zero[claim + 4..claim + 36].fill(0);
    assert_eq!(
        decode_trace_v2(&zero),
        Err(TraceDecodeErrorV1::Validation(
            TraceValidationErrorV1::ZeroIdentity
        ))
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_trace_v2(&trailing),
        Err(TraceDecodeErrorV1::TrailingBytes(1))
    );
}

fn claim_offset(bytes: &[u8]) -> usize {
    let needle = [
        KERNEL_IR_WIRE_VERSION_V10.to_le_bytes().as_slice(),
        KERNEL_IR_IDENTITY_POLICY_V1.to_le_bytes().as_slice(),
        &[9; 32],
        4_096_u64.to_le_bytes().as_slice(),
    ]
    .concat();
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("unique exact V10 KIR claim")
}
