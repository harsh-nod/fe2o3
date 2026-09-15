use super::*;

fn scalar() -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    )
}

fn aggregate(
    index: u8,
    fields: &[u32],
    offsets: &[u64],
    tuple: bool,
    pair: bool,
) -> SemanticTypeDeclV1 {
    let aggregate = SemanticAggregateTypeV1::new(fields.iter().copied().map(id).collect()).unwrap();
    SemanticTypeDeclV1::new(
        identity(index + 1),
        SemanticLayoutIdentityV1([index + 1; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(if pair { 16 } else { 0 }),
            if pair { 8 } else { 1 },
            if pair {
                SemanticBackendReprV1::ScalarPair {
                    first: scalar(),
                    second: scalar(),
                }
            } else {
                SemanticBackendReprV1::Memory { sized: true }
            },
            false,
            SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap(),
        )
        .unwrap(),
        if tuple {
            SemanticTypeShapeV1::Tuple(aggregate)
        } else {
            SemanticTypeShapeV1::Aggregate(aggregate)
        },
    )
}

fn fixture(
    format: SemanticGfx950LdsTransposeFormatV1,
) -> (
    InertSemanticMirRequestV1,
    SemanticFunctionAbiV1,
    SemanticGfx950TransposeContractV1,
) {
    // Component shape/ABI fixture only. Actual source custody, root claims and
    // complete transaction import are exercised by the compiler AMD callbacks.
    let types = vec![
        aggregate(0, &[], &[], false, false),
        SemanticTypeDeclV1::new(
            identity(2),
            SemanticLayoutIdentityV1([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(scalar()),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        aggregate(2, &[0], &[0], false, false),
        aggregate(3, &[0], &[0], false, false),
        aggregate(4, &[0, 0], &[0, 0], false, false),
        aggregate(5, &[0, 0], &[0, 0], false, false),
        aggregate(6, &[1, 1, 2, 0], &[0, 8, 16, 16], false, true),
        aggregate(7, &[1, 1, 3, 0], &[0, 8, 16, 16], false, true),
        aggregate(8, &[7, 5], &[0, 16], true, true),
        SemanticTypeDeclV1::new(
            identity(10),
            SemanticLayoutIdentityV1([10; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
    ];
    let request = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1([80; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![component_root()],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let pair = SemanticAbiPassModeV1::Pair {
        first: attributes,
        second: attributes,
    };
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1([81; 32]),
        SemanticLayoutIdentityV1([82; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![id(4), id(6)],
        id(8),
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                id(4),
                SemanticAbiPassModeV1::Ignore,
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(id(6), pair.clone())),
        ],
        SemanticAbiValueV1::new(id(8), pair),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let transpose = SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Publish {
            input_tile: id(4),
            input_workgroup: id(6),
            transition: id(8),
            output_workgroup: id(7),
            output_tile: id(5),
        },
        format,
        identity(21),
        Some(identity(22)),
    )
    .unwrap();
    (request, abi, transpose)
}

fn component_root() -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1([90; 32]),
        SemanticLayoutIdentityV1([91; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(id(9), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1([92; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1([93; 32]),
        SemanticMonomorphizationIdentityV1([94; 32]),
        SemanticGenericTypeArgumentsIdentityV1([95; 32]),
        SemanticConstGenericArgumentsIdentityV1([96; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1([97; 32]),
            id(9),
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1([98; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"transpose_component_root".to_vec()).unwrap(),
        contract(2).provenance().kernel_binding(),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ))
}

fn check_ordinary_abi(request: &InertSemanticMirRequestV1, abi: &SemanticFunctionAbiV1) {
    let mut context = ValidationContextV1 {
        request,
        limits: SemanticMirLimitsV1::default(),
        totals: ValidationTotalsV1::default(),
        work: 0,
    };
    for (index, ty) in request.types.iter().enumerate() {
        validate_type(&mut context, id(index as u32), ty).unwrap();
    }
    let binding = SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1([83; 32]),
        SemanticItemDefinitionIdentityV1([84; 32]),
        SemanticMonomorphizationIdentityV1([85; 32]),
        SemanticGenericTypeArgumentsIdentityV1([86; 32]),
        SemanticConstGenericArgumentsIdentityV1([87; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi.clone(),
    );
    validate_non_body_callable_abi(
        &mut context,
        SemanticMirLocationV1::Callable(SemanticCallableIdV1(0)),
        &binding,
    )
    .unwrap();
}

#[test]
fn publish_accepts_exact_rust_tuple_after_independent_layout_and_abi_validation() {
    for format in [
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
    ] {
        let (request, abi, transpose) = fixture(format);
        check_ordinary_abi(&request, &abi);
        assert!(abi_matches(&request, &abi, transpose));
        let base = contract(2);
        let common = SemanticExecutionCapabilityContractV1::new(
            SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose),
            SemanticExecutionCapabilitySignatureV1::new(&[id(4), id(6)], id(8)).unwrap(),
            base.provenance(),
            base.workgroup_brand().unwrap(),
            base.epoch_before().unwrap(),
            base.epoch_after(),
            base.source_identity(),
        )
        .unwrap();
        assert!(compiler_intrinsic_signature_matches(
            &request,
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: common },
            &abi
        ));
        assert_eq!(common.obligations().bits(), transpose.obligations());
        let mut missing_root = request.clone();
        missing_root.functions = Box::new([]);
        assert!(!compiler_intrinsic_signature_matches(
            &missing_root,
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: common },
            &abi,
        ));
        let mut wrong_binding = request.clone();
        wrong_binding.functions[0] =
            component_root().with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"foreign_component_root".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1([99; 32]),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ));
        assert!(!compiler_intrinsic_signature_matches(
            &wrong_binding,
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: common },
            &abi,
        ));
        assert!(
            fields(&request, id(8)).is_none(),
            "struct-only accessor remains closed"
        );
    }
}

#[test]
fn publish_rejects_valid_layout_wrong_tuple_kind_order_arity_and_nominal_fields() {
    for replacement in [
        aggregate(8, &[7, 5], &[0, 16], false, true),
        aggregate(8, &[5, 7], &[0, 0], true, true),
        aggregate(8, &[7], &[0], true, true),
        aggregate(8, &[7, 5, 0], &[0, 16, 16], true, true),
        aggregate(8, &[6, 5], &[0, 16], true, true),
        aggregate(8, &[7, 4], &[0, 16], true, true),
    ] {
        let (mut request, abi, transpose) = fixture(SemanticGfx950LdsTransposeFormatV1::Fp4E2M1);
        request.types[8] = replacement;
        check_ordinary_abi(&request, &abi);
        assert!(!abi_matches(&request, &abi, transpose));
    }
}

#[test]
fn publish_tuple_change_does_not_admit_tuple_tiles_or_borrowed_workgroup_inputs() {
    let (request, abi, transpose) = fixture(SemanticGfx950LdsTransposeFormatV1::Fp8E4M3);
    for tile in [4, 5] {
        let mut changed = request.clone();
        changed.types[tile] = aggregate(tile as u8, &[0, 0], &[0, 0], true, false);
        check_ordinary_abi(&changed, &abi);
        assert!(!abi_matches(&changed, &abi, transpose));
    }
    for argument in 0..2 {
        let mut changed = abi.clone();
        changed.source_argument_ownership[argument] =
            SemanticSourceArgumentOwnershipV1::SharedBorrow;
        assert!(!abi_matches(&request, &changed, transpose));
    }
    let mut unwind = abi.clone();
    unwind.can_unwind = true;
    assert!(!abi_matches(&request, &unwind, transpose));
    let mut foreign = abi.clone();
    foreign.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false };
    assert!(!abi_matches(&request, &foreign, transpose));
}
