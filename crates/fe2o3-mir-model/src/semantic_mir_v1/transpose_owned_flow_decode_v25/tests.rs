use super::*;

fn row() -> SemanticTransposeOwnedFlowV1 {
    let call = |f, b| SemanticOwnedSourceCallSiteV1 { function: SemanticFunctionIdV1(f), block: SemanticBlockIdV1(b) };
    SemanticTransposeOwnedFlowV1::from_encoded_parts(
        SemanticTransposeOwnedFlowSitesV1 {
            issue: call(0, 0),
            capture: SemanticOwnedSourceStatementSiteV1 { function: SemanticFunctionIdV1(0), block: SemanticBlockIdV1(1), statement: 0 },
            capture_field: 0,
            matrix_call: call(0, 1), closure_call: call(1, 0), stage: call(2, 0), publish: call(0, 2), workgroup_local: SemanticLocalIdV1(0),
        },
        vec![call(0, 3)],
        [(SemanticFunctionIdV1(0), [1; 32]), (SemanticFunctionIdV1(1), [2; 32]), (SemanticFunctionIdV1(2), [3; 32])],
        [4; 32],
    ).unwrap()
}

#[test]
fn common_v25_footer_payload_roundtrip_size_and_every_truncation() {
    let row = row();
    let mut writer = CanonicalWriterV1::new(4096);
    transpose_owned_flow_v25::encode_footer(&mut writer, SemanticMirWireVersionV1::V25, &[row.clone()]).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 4 + 204 + 8);
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert_eq!(decoder.transpose_owned_flows_v25(4).unwrap(), [row]);
    decoder.finish().unwrap();
    for end in 0..bytes.len() {
        let mut decoder = CanonicalDecoderV1::new(&bytes[..end], SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V25;
        assert!(decoder.transpose_owned_flows_v25(4).is_err(), "truncation {end}");
    }
}

#[test]
fn common_v25_footer_count_order_and_aggregate_borrow_bounds() {
    let mut writer = CanonicalWriterV1::new(4096);
    writer.u32(2).unwrap();
    row().encode(&mut writer).unwrap();
    row().encode(&mut writer).unwrap();
    let bytes = writer.finish();
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert!(matches!(decoder.transpose_owned_flows_v25(8), Err(SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::InvalidFunctionAbi))));
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert!(decoder.transpose_owned_flows_v25(1).is_err());
    let limits = SemanticMirLimitsV1::default().with_limit(SemanticMirResourceV1::CallArguments, 1).unwrap();
    let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert!(matches!(decoder.transpose_owned_flows_v25(8), Err(SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::CallArguments, .. }))));
}

#[test]
fn common_v25_defined_nine_stays_unregistered_and_eight_is_unchanged() {
    for raw in 20..=25 {
        let mut decoder = CanonicalDecoderV1::new(&[9], SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::from_u16(raw).unwrap();
        assert!(matches!(decoder.defined_capability_contract(), Err(SemanticMirDecodeErrorV1::InvalidTag { context: "defined capability contract", value: 9, .. })));
    }
    let record = crate::semantic_mir_v1::defined_reusable_lds_v1::tests::record_fixture();
    let contract = SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record);
    let mut writer = CanonicalWriterV1::new(4096);
    contract.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes[0], 8);
    for version in [SemanticMirWireVersionV1::V24, SemanticMirWireVersionV1::V25] {
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert_eq!(decoder.defined_capability_contract().unwrap(), contract);
        decoder.finish().unwrap();
    }
}
