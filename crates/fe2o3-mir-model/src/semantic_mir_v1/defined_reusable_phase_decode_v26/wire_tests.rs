//! Payload codec tests are inert; none of these synthetic identities grant source authority.
use super::*;
type Recipe = SemanticDefinedReusablePhaseRecipeV1;
#[path="wire_tests/guarded_grid_substitution.rs"]
mod guarded_grid_substitution;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1(index)
}
fn brands() -> SemanticPhaseBrandsV1 {
    SemanticPhaseBrandsV1 {
        root_brand: ty(10),
        outer_workgroup_brand: ty(11),
        phase_brand: ty(12),
        dynamic_epoch: ty(13),
    }
}
fn reference(index: u32, kind: SemanticPhaseReferenceKindV1) -> SemanticPhaseReferenceV1 {
    SemanticPhaseReferenceV1 {
        reference: ty(index),
        pointee: ty(index + 1),
        kind,
    }
}
fn callable(index: u32) -> SemanticPhaseCallableV1 {
    SemanticPhaseCallableV1 {
        callable: SemanticCallableIdV1(index),
        identity: SemanticFunctionIdentityV1([21; 32]),
        abi: SemanticAbiIdentityV1([22; 32]),
    }
}
fn recipes() -> [Recipe; 5] {
    use SemanticPhaseReferenceKindV1::{Shared, Unique};
    [
        Recipe::OwnerConvert {
            workgroup: ty(1),
            owner: ty(2),
            root_brand: ty(10),
            outer_workgroup_brand: ty(11),
            input_epoch: ty(13),
        },
        Recipe::Issue {
            owner_reference: reference(1, Unique),
            owner: ty(2),
            phase_workgroup: ty(3),
            brands: brands(),
        },
        Recipe::WithPhase {
            owner_reference: reference(1, Unique),
            owner: ty(2),
            closure: ty(3),
            call_tuple: ty(4),
            phase_workgroup: ty(5),
            completion: ty(6),
            result_pair: ty(7),
            result: ty(8),
            drop_result: ty(9),
            brands: brands(),
            issue: callable(1),
            invoke: callable(2),
            drop_completion: callable(3),
            issue_block: SemanticBlockIdV1(0),
            invoke_block: SemanticBlockIdV1(1),
            drop_block: SemanticBlockIdV1(2),
            relay: SemanticPhaseCompletionRelayV1 {
                closure_function: SemanticFunctionIdV1(2),
                closure_body: [23; 32],
                finish: callable(4),
                finish_call_block: SemanticBlockIdV1(0),
                finish_normal_target: SemanticBlockIdV1(1),
                closure_pack_block: SemanticBlockIdV1(1),
                closure_pack_statement: 0,
                closure_return_block: SemanticBlockIdV1(1),
                completion_field: SemanticPhaseCompletionFieldV1::ErasedZstConstant {
                    canonical_operand: [24; 32],
                },
                wrapper_drop: SemanticPhaseCompletionDropV1::ErasedZstConstant {
                    canonical_operand: [25; 32],
                },
            },
        },
        Recipe::Bind {
            phase_reference: reference(1, Shared),
            storage_reference: reference(3, Unique),
            phase_workgroup: ty(2),
            reusable_storage: ty(4),
            phase_lds: ty(5),
            element: ty(6),
            uninitialized_marker: ty(7),
            storage_marker: ty(8),
            thread_marker: ty(9),
            brands: brands(),
            elements: 64,
        },
        Recipe::Finish {
            workgroup_before_barrier: ty(1),
            workgroup_after_barrier: ty(2),
            completion: ty(3),
            brands: brands(),
            input_epoch: ty(4),
            advanced_epoch: ty(5),
            barrier: callable(1),
            barrier_block: SemanticBlockIdV1(0),
            return_block: SemanticBlockIdV1(1),
        },
    ]
}
fn record(recipe: Recipe) -> SemanticDefinedReusablePhaseV1 {
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1(0),
        SemanticKernelBindingIdentityV1([31; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1([32; 32]),
        SemanticTypeIdentityV1([33; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1([34; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1([35; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1([36; 32]),
    )
    .unwrap();
    SemanticDefinedReusablePhaseV1::from_encoded_parts(
        SemanticFunctionIdV1(1),
        SemanticFunctionIdentityV1([41; 32]),
        SemanticAbiIdentityV1([42; 32]),
        [43; 32],
        provenance,
        SemanticPhaseIncomingCommitmentV1::from_encoded_parts(2, [44; 32]).unwrap(),
        [45; 32],
        recipe,
    )
    .unwrap()
}
fn encoded(record: SemanticDefinedReusablePhaseV1) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(4096);
    SemanticDefinedCapabilityContractV1::ReusablePhase(record)
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
    let row = decoder.defined_capability_contract()?;
    decoder.finish()?;
    Ok(row)
}

#[test]
fn phase_v26_all_five_payloads_roundtrip_and_exact_byte_bounds() {
    for (recipe, expected) in recipes().into_iter().zip([386, 399, 829, 436, 478]) {
        let row = record(recipe);
        let bytes = encoded(row);
        assert_eq!(bytes[0], 9);
        assert_eq!(bytes.len(), expected);
        assert_eq!(
            decode(&bytes, SemanticMirWireVersionV1::V26).unwrap(),
            SemanticDefinedCapabilityContractV1::ReusablePhase(row)
        );
        let mut writer = CanonicalWriterV1::new(expected as u64);
        SemanticDefinedCapabilityContractV1::ReusablePhase(row)
            .encode(&mut writer)
            .unwrap();
        assert_eq!(writer.finish(), bytes);
        let mut writer = CanonicalWriterV1::new(expected as u64 - 1);
        assert!(matches!(
            SemanticDefinedCapabilityContractV1::ReusablePhase(row).encode(&mut writer),
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::CanonicalBytes,
                ..
            })
        ));
    }
}

#[test]
fn phase_v26_every_payload_truncation_and_trailing_byte_rejects() {
    for recipe in recipes() {
        let mut bytes = encoded(record(recipe));
        for end in 0..bytes.len() {
            assert!(
                decode(&bytes[..end], SemanticMirWireVersionV1::V26).is_err(),
                "recipe {recipe:?}, end {end}"
            );
        }
        bytes.push(0);
        assert!(matches!(
            decode(&bytes, SemanticMirWireVersionV1::V26),
            Err(SemanticMirDecodeErrorV1::TrailingBytes { .. })
        ));
    }
}

#[test]
fn phase_v26_old_versions_reject_tag_nine_before_reading_payload() {
    let bytes = encoded(record(recipes()[0]));
    for raw in 20..=25 {
        let version = SemanticMirWireVersionV1::from_u16(raw).unwrap();
        assert!(
            matches!(
                decode(&bytes, version),
                Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "defined capability contract",
                    value: 9,
                    ..
                })
            ),
            "{version:?}"
        );
    }
    let mut decoder = CanonicalDecoderV1::new(&bytes[1..], SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V25;
    assert!(matches!(
        decoder.reusable_phase_payload(),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V25,
                required: SemanticMirWireVersionV1::V26
            }
        ))
    ));
}

#[test]
fn phase_v26_closed_tags_reject_substitutions() {
    let owner = encoded(record(recipes()[0]));
    let mut changed = owner;
    changed[0] = 11;
    assert!(matches!(
        decode(&changed, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "defined capability contract",
            value: 11,
            ..
        })
    ));
    changed[0] = 9;
    changed[365] = 5;
    assert!(matches!(
        decode(&changed, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "defined reusable phase recipe",
            value: 5,
            ..
        })
    ));
    let mut issue = encoded(record(recipes()[1]));
    issue[374] = 2;
    assert!(matches!(
        decode(&issue, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "phase reference kind",
            value: 2,
            ..
        })
    ));
    let mut wrapper = encoded(record(recipes()[2]));
    wrapper[763] = 2;
    assert!(matches!(
        decode(&wrapper, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "phase completion field",
            value: 2,
            ..
        })
    ));
    wrapper[763] = 1;
    wrapper[796] = 2;
    assert!(matches!(
        decode(&wrapper, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "phase completion drop",
            value: 2,
            ..
        })
    ));
}

#[test]
fn phase_v26_zero_identity_incoming_bound_and_type_bound_reject() {
    let bytes = encoded(record(recipes()[0]));
    for range in [5..37, 37..69, 69..101, 301..333, 333..365] {
        let mut changed = bytes.clone();
        changed[range.clone()].fill(0);
        assert!(
            decode(&changed, SemanticMirWireVersionV1::V26).is_err(),
            "{range:?}"
        );
    }
    for count in [0, u32::MAX] {
        let mut changed = bytes.clone();
        changed[297..301].copy_from_slice(&count.to_le_bytes());
        assert!(matches!(
            decode(&changed, SemanticMirWireVersionV1::V26),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidFunctionAbi
            ))
        ));
    }
    let mut changed = bytes;
    changed[366..370].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode(&changed, SemanticMirWireVersionV1::V26),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::InvalidFunctionAbi
        ))
    ));
}

#[test]
fn phase_v26_visitors_cover_all_declared_edges_and_stop_at_error() {
    for (recipe, (types, calls)) in
        recipes()
            .into_iter()
            .zip([(5, 0), (8, 0), (14, 4), (15, 0), (9, 1)])
    {
        let mut found = Vec::new();
        recipe
            .try_visit_types(|id| {
                found.push(id);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(found.len(), types, "{recipe:?}");
        let mut found = Vec::new();
        recipe
            .try_visit_callables(|id| {
                found.push(id);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(found.len(), calls, "{recipe:?}");
        let mut visited = 0;
        assert_eq!(
            recipe.try_visit_types(|_| {
                visited += 1;
                Err(17)
            }),
            Err(17)
        );
        assert_eq!(visited, 1);
    }
}

#[test]
fn phase_v26_retained_completion_fields_have_no_erased_digest_bytes() {
    let Recipe::WithPhase { mut relay, .. } = recipes()[2] else {
        unreachable!()
    };
    relay.completion_field = SemanticPhaseCompletionFieldV1::RetainedFinishResult;
    relay.wrapper_drop = SemanticPhaseCompletionDropV1::RetainedPairField;
    let mut recipe = recipes()[2];
    let Recipe::WithPhase { relay: slot, .. } = &mut recipe else {
        unreachable!()
    };
    *slot = relay;
    let row = record(recipe);
    let bytes = encoded(row);
    assert_eq!(bytes.len(), 765);
    assert_eq!(
        decode(&bytes, SemanticMirWireVersionV1::V26).unwrap(),
        SemanticDefinedCapabilityContractV1::ReusablePhase(row)
    );
}

#[test]
fn phase_v26_old_tag_eight_is_not_reencoded_as_nine() {
    let record = crate::semantic_mir_v1::defined_reusable_lds_v1::tests::record_fixture();
    let contract = SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record);
    assert_eq!(
        contract.minimum_wire_version(),
        SemanticMirWireVersionV1::V24
    );
    let mut writer = CanonicalWriterV1::new(4096);
    contract.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes[0], 8);
    for version in [
        SemanticMirWireVersionV1::V24,
        SemanticMirWireVersionV1::V25,
        SemanticMirWireVersionV1::V26,
    ] {
        assert_eq!(decode(&bytes, version).unwrap(), contract);
    }
}
