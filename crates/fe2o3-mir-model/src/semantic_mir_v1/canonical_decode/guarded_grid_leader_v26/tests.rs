use super::*;

fn record() -> SemanticGuardedGridLeaderV1 {
    let body = |i: u32| SemanticGuardedGridBodyIdentityV1 {
        function: SemanticFunctionIdV1(i),
        source: SemanticFunctionIdentityV1([10 + i as u8; 32]),
        abi: SemanticAbiIdentityV1([20 + i as u8; 32]),
        body: [30 + i as u8; 32],
    };
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1(2),
        SemanticKernelBindingIdentityV1([40; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1([41; 32]),
        SemanticTypeIdentityV1([42; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1([43; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1([44; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1([45; 32]),
    )
    .unwrap();
    SemanticGuardedGridLeaderV1::from_encoded_parts(
        [body(0), body(1), body(2), body(3), body(4)],
        SemanticGuardedGridLeaderTypesV1::new(std::array::from_fn(|i| SemanticTypeIdV1(i as u32))),
        SemanticGuardedGridLeaderSourceV1 {
            caller: SemanticFunctionIdV1(2),
            call_block: SemanticBlockIdV1(8),
            grid_getter: SemanticFunctionIdV1(3),
            grid_current: SemanticFunctionIdV1(4),
            grid_call_block: SemanticBlockIdV1(6),
        },
        SemanticGuardedGridLeaderBodyV1 {
            guard: SemanticBlockIdV1(0),
            issuer: SemanticBlockIdV1(1),
            some: SemanticBlockIdV1(2),
            none: SemanticBlockIdV1(3),
            exit: SemanticBlockIdV1(4),
            receiver: SemanticLocalIdV1(1),
            result: SemanticLocalIdV1(0),
            issued: SemanticLocalIdV1(4),
            issuer_callable: SemanticCallableIdV1(1),
        },
        provenance,
        SemanticTypeIdentityV1([46; 32]),
    )
    .unwrap()
}
fn bytes() -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(817);
    SemanticDefinedCapabilityContractV1::GuardedGridLeader(record())
        .encode(&mut writer)
        .unwrap();
    writer.finish()
}
fn decode(
    bytes: &[u8],
    version: SemanticMirWireVersionV1,
) -> Result<SemanticDefinedCapabilityContractV1, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = version;
    let record = decoder.defined_capability_contract()?;
    decoder.finish()?;
    Ok(record)
}
#[test]
fn guarded_grid_v26_inert_payload_roundtrip_and_exact_byte_budget() {
    let bytes = bytes();
    assert_eq!((bytes.len(), bytes[0]), (817, 10));
    assert_eq!(
        decode(&bytes, SemanticMirWireVersionV1::V26).unwrap(),
        SemanticDefinedCapabilityContractV1::GuardedGridLeader(record())
    );
    let mut writer = CanonicalWriterV1::new(816);
    assert!(matches!(
        SemanticDefinedCapabilityContractV1::GuardedGridLeader(record()).encode(&mut writer),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
}
#[test]
fn guarded_grid_v26_truncation_suffix_zero_identity_and_cross_body_reject() {
    let original = bytes();
    for end in 0..original.len() {
        assert!(
            matches!(
                decode(&original[..end], SemanticMirWireVersionV1::V26),
                Err(SemanticMirDecodeErrorV1::UnexpectedEnd { .. })
            ),
            "end {end}"
        );
    }
    let mut suffix = original.clone();
    suffix.push(0);
    assert!(matches!(
        decode(&suffix, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::TrailingBytes { .. })
    ));
    for range in [5..37, 37..69, 69..101, 201..205] {
        let mut changed = original.clone();
        changed[range].fill(0);
        assert_eq!(
            decode(&changed, SemanticMirWireVersionV1::V26),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidFunctionAbi
            ))
        );
    }
}
#[test]
fn guarded_grid_old_versions_reject_tag_ten_without_reading_its_payload() {
    let original = bytes();
    for raw in 20..=25 {
        assert!(matches!(
            decode(&original, SemanticMirWireVersionV1::from_u16(raw).unwrap()),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "defined capability contract",
                value: 10,
                ..
            })
        ));
    }
    let mut decoder = CanonicalDecoderV1::new(&original[1..], SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert!(matches!(
        decoder.guarded_grid_leader_payload_v26(),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V25,
                required: SemanticMirWireVersionV1::V26
            }
        ))
    ));
}

#[test]
fn guarded_grid_payload_cannot_be_relabelled_as_phase_tag_nine() {
    let mut changed = bytes();
    changed[0] = 9;
    // The closed phase incoming commitment rejects these Grid body bytes
    // before any phase recipe can be constructed.
    assert_eq!(
        decode(&changed, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::InvalidFunctionAbi
        ))
    );
    assert!(matches!(
        decode(&changed, SemanticMirWireVersionV1::V25),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "defined capability contract",
            value: 9,
            ..
        })
    ));
}
