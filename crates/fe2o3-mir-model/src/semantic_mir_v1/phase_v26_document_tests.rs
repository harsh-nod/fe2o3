use super::*;
#[path = "phase_v26_document_tests/fixture.rs"]
mod fixture;
#[path = "phase_v26_document_tests/incoming_query_tests.rs"]
mod incoming_query_tests;

fn admitted() -> AdmittedInertSemanticMirV1 {
    let mut request = fixture::request();
    let _ = fixture::attach(&mut request);
    request.admit(SemanticMirLimitsV1::default()).unwrap()
}

fn version_selection_footer() -> SemanticTransposeOwnedFlowV1 {
    let call = |function| SemanticOwnedSourceCallSiteV1 {
        function: SemanticFunctionIdV1(function),
        block: SemanticBlockIdV1(0),
    };
    // Like common25's compositional minimum-version test, this row isolates
    // feature selection. It is not an admitted or live-authenticated transpose.
    SemanticTransposeOwnedFlowV1::from_encoded_parts(
        SemanticTransposeOwnedFlowSitesV1 {
            issue: call(0),
            capture: SemanticOwnedSourceStatementSiteV1 {
                function: SemanticFunctionIdV1(0),
                block: SemanticBlockIdV1(0),
                statement: 0,
            },
            capture_field: 0,
            matrix_call: call(0),
            closure_call: call(1),
            stage: call(2),
            publish: call(0),
            workgroup_local: SemanticLocalIdV1(0),
        },
        vec![],
        [
            (SemanticFunctionIdV1(0), [1; 32]),
            (SemanticFunctionIdV1(1), [2; 32]),
            (SemanticFunctionIdV1(2), [3; 32]),
        ],
        [4; 32],
    )
    .unwrap()
}

#[test]
fn phase_v26_mixed_transpose_minimum_is_the_feature_maximum() {
    let baseline = minimum_wire_version(&fixture::request());
    assert!(baseline < SemanticMirWireVersionV1::V25);
    for (phase, transpose, expected) in [
        (false, false, baseline),
        (false, true, SemanticMirWireVersionV1::V25),
        (true, false, SemanticMirWireVersionV1::V26),
        (true, true, SemanticMirWireVersionV1::V26),
    ] {
        let mut request = fixture::request();
        if phase {
            let _ = fixture::attach(&mut request);
        }
        if transpose {
            request.transpose_owned_flows = vec![version_selection_footer()].into_boxed_slice();
        }
        assert_eq!(
            minimum_wire_version(&request),
            expected,
            "phase={phase}, transpose={transpose}"
        );
    }
}

#[test]
fn phase_v26_mixed_version_selection_does_not_admit_an_unbound_footer() {
    let mut request = fixture::request();
    let _ = fixture::attach(&mut request);
    request.transpose_owned_flows = vec![version_selection_footer()].into_boxed_slice();
    assert_eq!(
        minimum_wire_version(&request),
        SemanticMirWireVersionV1::V26
    );
    assert!(matches!(
        request.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn phase_v26_defined_bind_admits_and_roundtrips_current_and_exact() {
    let original = admitted();
    assert_eq!(original.wire_version(), SemanticMirWireVersionV1::V26);
    for decoded in [
        AdmittedInertSemanticMirV1::decode_exact_v26_canonical(
            original.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        ),
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            original.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        ),
    ] {
        let decoded = decoded.unwrap();
        assert_eq!(decoded.canonical_encoding(), original.canonical_encoding());
        assert_eq!(
            decoded.functions()[2].defined_capability_contract(),
            original.functions()[2].defined_capability_contract()
        );
    }
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v25_canonical(
            original.canonical_encoding(),
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn phase_v26_minimum_version_and_downgrade_fail_before_old_encoding() {
    let mut request = fixture::request();
    let _ = fixture::attach(&mut request);
    assert_eq!(
        minimum_wire_version(&request),
        SemanticMirWireVersionV1::V26
    );
    assert!(matches!(
        request.admit_exact_v25(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V25,
            required: SemanticMirWireVersionV1::V26
        })
    ));
    let original = admitted();
    let mut bytes = original.canonical_encoding().to_vec();
    bytes[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&25u16.to_le_bytes());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            &bytes,
            SemanticMirLimitsV1::default()
        ),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "defined capability contract",
            value: 9,
            ..
        })
    ));
}

#[test]
fn phase_v26_attachment_exclusion_keeps_fixed_v25_body_bytes() {
    let mut request = fixture::request();
    let before = canonical_semantic_source_body_sha256_v25(
        &request.functions[2],
        HARD_MAX_CANONICAL_BYTES_V1,
    )
    .unwrap();
    let caller = canonical_semantic_source_body_sha256_v25(
        &request.functions[1],
        HARD_MAX_CANONICAL_BYTES_V1,
    )
    .unwrap();
    let record = fixture::attach(&mut request);
    assert_eq!(*record.body_identity(), before.0);
    assert_eq!(
        canonical_semantic_source_body_sha256_v25(
            &request.functions[2],
            HARD_MAX_CANONICAL_BYTES_V1
        )
        .unwrap(),
        before
    );
    assert_eq!(
        canonical_semantic_source_body_sha256_v25(
            &request.functions[1],
            HARD_MAX_CANONICAL_BYTES_V1
        )
        .unwrap(),
        caller
    );
    assert_eq!(
        SEMANTIC_SOURCE_BODY_FRAGMENT_VERSION_V25,
        SemanticMirWireVersionV1::V25
    );
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(matches!(
        encode_function(
            &mut writer,
            &request.functions[2],
            SemanticMirWireVersionV1::V25
        ),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V25,
            required: SemanticMirWireVersionV1::V26
        })
    ));
}

#[test]
fn phase_v26_incoming_body_abi_reference_and_provenance_mutations_reject() {
    for mutation in 0..5 {
        let mut request = fixture::request();
        let _ = fixture::attach(&mut request);
        match mutation {
            0 => {
                let mut identity = [21; 32];
                identity[31] = 22;
                request.functions[1].identity = SemanticFunctionIdentityV1(identity);
            }
            1 => request.functions[2].abi.identity = SemanticAbiIdentityV1([222; 32]),
            2 => request.functions[2].locals[1].ty = fixture::STORAGE_REF,
            3 => {
                request.functions[0].export = Some(SemanticFunctionExportV1::Kernel(
                    SemanticKernelEntryV1::new(
                        SemanticLinkSymbolV1::new(b"changed_binding".to_vec()).unwrap(),
                        SemanticKernelBindingIdentityV1([211; 32]),
                        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
                    ),
                ))
            }
            4 => {
                let SemanticTerminatorKindV1::Call(call) =
                    &mut request.functions[1].blocks[0].terminator.kind
                else {
                    unreachable!()
                };
                call.unwind = SemanticUnwindActionV1::Continue;
            }
            _ => unreachable!(),
        }
        assert!(
            request.admit(SemanticMirLimitsV1::default()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn phase_v26_batched_attachment_budget_failure_is_atomic() {
    let mut request = fixture::request();
    let record = fixture::observe(&request);
    let mut work = 1;
    let result = attach_reusable_phase_contracts_v26(
        &mut request.functions,
        &request.callables,
        &request.types,
        &[record],
        &mut work,
    );
    assert!(result.is_err());
    assert!(
        request
            .functions
            .iter()
            .all(|function| function.defined_capability_contract().is_none())
    );
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    attach_reusable_phase_contracts_v26(
        &mut request.functions,
        &request.callables,
        &request.types,
        &[record],
        &mut work,
    )
    .unwrap();
    assert_eq!(
        request.functions[2].defined_capability_contract(),
        Some(&SemanticDefinedCapabilityContractV1::ReusablePhase(record))
    );
}

#[test]
fn phase_v26_defined_bind_uses_canonical_local_roles_not_raw_positions() {
    let mut request = fixture::request();
    let old_body = canonical_semantic_source_body_sha256_v25(
        &request.functions[2],
        HARD_MAX_CANONICAL_BYTES_V1,
    )
    .unwrap()
    .0;
    let locals = &mut request.functions[2].locals;
    locals.rotate_left(1);
    for (index, local) in locals.iter_mut().enumerate() {
        local.identity = SemanticLocalIdentityV1([index as u8 + 1; 32]);
    }
    assert_eq!(locals[2].role(), SemanticLocalRoleV1::Return);
    let record = fixture::attach(&mut request);
    assert_ne!(record.body_identity(), &old_body);
    let owner = request.admit(SemanticMirLimitsV1::default()).unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v26_canonical(
        owner.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let expansion = crate::SemanticCallExpansionV1::try_new(
        &decoded,
        crate::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    let bindings = expansion.defined_capability_bindings(&decoded).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        bindings[0].callee_arguments(),
        &[SemanticLocalIdV1(7), SemanticLocalIdV1(8)]
    );
    assert_eq!(bindings[0].callee_return(), SemanticLocalIdV1(9));
}

#[test]
fn phase_v26_footer_composition_preserves_exact_v25_facade_and_moves_tables() {
    assert!(matches!(
        admitted().with_transpose_owned_flows_v25(vec![], SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V25,
            required: SemanticMirWireVersionV1::V26
        })
    ));
    let original = admitted();
    let functions = original.functions().as_ptr();
    let types = original.types().as_ptr();
    let digest = *original.semantic_sha256().as_bytes();
    let final_owner = original
        .with_transpose_owned_flows_for_wire_version(
            vec![],
            SemanticMirWireVersionV1::V26,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(final_owner.functions().as_ptr(), functions);
    assert_eq!(final_owner.types().as_ptr(), types);
    assert_eq!(final_owner.semantic_sha256().as_bytes(), &digest);
    assert_eq!(final_owner.wire_version(), SemanticMirWireVersionV1::V26);
}

#[test]
fn phase_v26_replay_maps_arguments_and_retains_original_record() {
    use crate::semantic_direct_call_expansion_v1::{
        SemanticCallExpansionErrorV1, SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
    };
    let owner = admitted();
    let expansion =
        SemanticCallExpansionV1::try_new(&owner, SemanticCallExpansionLimitsV1::default()).unwrap();
    let bindings = expansion.defined_capability_bindings(&owner).unwrap();
    let [binding] = bindings.as_slice() else {
        panic!("expected one original binding")
    };
    assert_eq!(
        binding.contract(),
        *owner.functions()[2].defined_capability_contract().unwrap()
    );
    assert_eq!(binding.caller_function(), SemanticFunctionIdV1(1));
    assert_eq!(binding.call_block(), SemanticBlockIdV1(0));
    // The checked root retains its three wrapper locals before the body frame.
    assert_eq!(
        binding.arguments(),
        &[
            SemanticOperandV1::Copy(fixture::place(4, fixture::PHASE_REF)),
            SemanticOperandV1::Move(fixture::place(5, fixture::STORAGE_REF))
        ]
    );
    assert_eq!(
        binding.callee_arguments(),
        &[SemanticLocalIdV1(8), SemanticLocalIdV1(9)]
    );
    assert_eq!(binding.callee_return(), SemanticLocalIdV1(7));
    assert_eq!(binding.destination(), &fixture::place(6, fixture::LEASE));
    assert_ne!(binding.caller_instance(), binding.callee_instance());
    assert!(
        binding.retained_auxiliary_bytes().unwrap()
            >= 2 * std::mem::size_of::<SemanticOperandV1>()
                + 2 * std::mem::size_of::<SemanticLocalIdV1>()
    );
    let mut changed = fixture::request();
    changed.functions[1].identity = SemanticFunctionIdentityV1([21; 32].map(|v| v + 1));
    changed.functions[2].identity = SemanticFunctionIdentityV1([23; 32]);
    let _ = fixture::attach(&mut changed);
    let changed = changed.admit(SemanticMirLimitsV1::default()).unwrap();
    assert!(matches!(
        expansion.defined_capability_bindings(&changed),
        Err(SemanticCallExpansionErrorV1::SourceMismatch)
    ));
}
