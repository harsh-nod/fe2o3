pub(super) fn owner() -> ProductionSemanticSsaOwnerV1 {
    let original = super::owner(Shape::RustCall);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let SemanticBackendReprV1::Scalar(scalar) =
        *types[U32.index() as usize].layout().backend_repr()
    else {
        unreachable!()
    };
    types[PAIR.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([219; 32]),
        SemanticLayoutIdentityV1::from_sha256([219; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::scalar_pair(scalar, scalar),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![CONTEXT, U32, U32]).unwrap()),
    );
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([203; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value_abi(&types, PAIR)); 2],
        ignored(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let original_root = &semantic.functions()[ROOT.index() as usize];
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        root_abi,
        original_root.locals().to_vec(),
        original_root.blocks().to_vec(),
    )
    .with_kernel_entry(original_root.kernel_entry().unwrap().clone());
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([205; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![UNIT, PAIR],
        U32,
        vec![
            SemanticAbiArgumentV1::source(ignored(UNIT)),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, ignored(CONTEXT)),
            SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(U32)),
            SemanticAbiArgumentV1::rust_call_tuple_field(2, direct(U32)),
        ],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let field = |field| SemanticLocalRoleV1::RustCallTupleField { argument: 1, field };
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        vec![
            local(230, U32, SemanticLocalRoleV1::Return),
            local(231, UNIT, SemanticLocalRoleV1::Argument(0)),
            local(232, U32, field(2)),
            local(233, CONTEXT, field(0)),
            local(234, U32, field(1)),
            local(235, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(
            240,
            vec![
                assign(
                    place(5, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, CONTEXT))),
                ),
                assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: SemanticOperandV1::Copy(place(4, U32)),
                        right: SemanticOperandV1::Copy(place(2, U32)),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(HELPER),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
