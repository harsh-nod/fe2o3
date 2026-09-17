use super::*;

fn realtime_request() -> InertSemanticMirRequestV1 {
    let mut request = non_scan_trap_request();
    let ty = SemanticTypeIdV1::from_index(1);
    request.types[1] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(130)),
        SemanticLayoutIdentityV1(identity(131)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    } = &mut request.callables[1]
    else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Realtime64;
    binding.abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(132)),
        SemanticLayoutIdentityV1(identity(133)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(ty, request.functions[0].abi.return_value.mode.clone()),
    )
    .unwrap();
    let function = &mut request.functions[0];
    let mut locals = function.locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(141)),
        ty,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    function.locals = locals.into_boxed_slice();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], ty).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    function.blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(call);
    let mut blocks = function.blocks.to_vec();
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(142)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Return,
            ),
        )
        .unwrap(),
    );
    function.blocks = blocks.into_boxed_slice();
    request
}

#[test]
fn realtime_v32_exact_round_trip_and_legacy_rejection() {
    let limits = SemanticMirLimitsV1::default();
    let request = realtime_request();
    let admitted = request.clone().admit_current_production(limits).unwrap();
    assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V32);
    let exact = request.clone().admit_exact_v32(limits).unwrap();
    assert_eq!(admitted.canonical_encoding(), exact.canonical_encoding());
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
        admitted.canonical_encoding(),
        limits,
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    let production = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        admitted.canonical_encoding(),
        limits,
    )
    .unwrap();
    assert_eq!(
        production.canonical_encoding(),
        admitted.canonical_encoding()
    );
    assert!(request.clone().admit_exact_v28(limits).is_err());
    assert!(request.admit_exact_v29(limits).is_err());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
            admitted.canonical_encoding(),
            limits,
        )
        .is_err()
    );
}

#[test]
fn realtime_v32_signature_is_exact_u64_without_arguments() {
    let mut request = realtime_request();
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &request.callables[1] else {
        unreachable!()
    };
    assert!(compiler_intrinsic_signature_matches(
        &request,
        SemanticCompilerIntrinsicOperationV1::Realtime64,
        &binding.abi,
    ));
    let abi = request.functions[0].abi.clone();
    assert!(!compiler_intrinsic_signature_matches(
        &request,
        SemanticCompilerIntrinsicOperationV1::Realtime64,
        &abi,
    ));
    request.types[1].shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: true,
        bits: 64,
    });
    assert!(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn realtime_v32_has_one_new_tag_without_reopening_execution_capabilities() {
    assert_eq!(
        compiler_intrinsic_round_trip(
            SemanticCompilerIntrinsicOperationV1::Realtime64,
            SemanticMirWireVersionV1::V32,
        ),
        vec![87]
    );
    for tag in 69..=86 {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V32;
        assert!(decoder.compiler_intrinsic().is_err(), "tag {tag}");
    }
    let execution = SemanticCompilerIntrinsicOperationV1::Execution(
        SemanticExecutionOperationV29::ContextIssue {
            context: SemanticTypeIdV1(0),
        },
    );
    let request =
        version_selection_request([execution, SemanticCompilerIntrinsicOperationV1::Realtime64]);
    assert!(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(
        encode_compiler_intrinsic_operation(&mut writer, execution, SemanticMirWireVersionV1::V32)
            .is_err()
    );
}

#[test]
fn realtime_v32_does_not_change_no_clock_production_bytes() {
    let request = minimal_request();
    let limits = SemanticMirLimitsV1::default();
    let production = request.clone().admit_current_production(limits).unwrap();
    let legacy = request.admit_exact_v5(limits).unwrap();
    assert_eq!(production.wire_version(), SemanticMirWireVersionV1::V5);
    assert_eq!(production.canonical_encoding(), legacy.canonical_encoding());
}
