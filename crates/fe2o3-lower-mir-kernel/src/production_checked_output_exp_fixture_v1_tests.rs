const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const CONTEXT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn exp_argument(ty: SemanticTypeIdV1) -> SemanticAbiArgumentV1 {
    let value = if ty == CONTEXT_REF {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        true,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    } else {
        fp_direct(ty)
    };
    SemanticAbiArgumentV1::source(value)
}

// Ordinary semantic construction inputs only. Every changed request gets fresh
// admission, SSA, materialization and ranked/source correspondence below.
fn exp_receipt(helper: bool) -> ProductionMaterializedRankedModuleReceiptV1 {
    let base = fp_receipt(32, FpRecipe::Negate, helper);
    let source = base.materialized.semantic_ssa.source_semantic();
    let mut types = source.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([5; 32]),
        SemanticLayoutIdentityV1::from_sha256([5; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([6; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    CONTEXT,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        1,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let mut functions = source.functions().to_vec();
    let current = SemanticCallableIdV1::from_index(functions.len() as u32);
    let exp = SemanticCallableIdV1::from_index(functions.len() as u32 + 1);
    let old = &functions[usize::from(helper)];
    let mut locals = old.locals().to_vec();
    let context = locals.len() as u32;
    for (tag, ty) in [(240, CONTEXT), (241, CONTEXT_REF)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let call = |callee, arguments, destination, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                callee,
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let old_statements = old.blocks()[0].statements();
    let block_tag = if helper { 91 } else { 31 };
    let blocks = vec![
        block(
            block_tag,
            if helper {
                vec![]
            } else {
                vec![old_statements[0].clone()]
            },
            call(current, vec![], context, CONTEXT, 1),
        ),
        block(
            block_tag + 1,
            vec![assignment(
                context + 1,
                CONTEXT_REF,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(context, CONTEXT),
                },
            )],
            call(
                exp,
                vec![
                    value(context + 1, CONTEXT_REF),
                    value(if helper { 1 } else { 2 }, FP_VALUE),
                ],
                if helper { 0 } else { 5 },
                FP_VALUE,
                2,
            ),
        ),
        block(
            block_tag + 2,
            if helper {
                vec![]
            } else {
                vec![old_statements[2].clone()]
            },
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let mut function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = old.kernel_entry() {
        function = function.with_kernel_entry(entry.clone());
    }
    functions[usize::from(helper)] = function;
    let mut callables: Vec<_> = (0..functions.len())
        .map(|ordinal| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(ordinal as u32))
        })
        .collect();
    for (tag, inputs, output, operation) in [
        (
            200,
            vec![],
            CONTEXT,
            SemanticCompilerIntrinsicOperationV1::MathContextCurrent { context: CONTEXT },
        ),
        (
            201,
            vec![CONTEXT_REF, FP_VALUE],
            FP_VALUE,
            SemanticCompilerIntrinsicOperationV1::MathF32 {
                context: CONTEXT,
                function: SemanticF32MathFunctionV1::Exp,
            },
        ),
    ] {
        let ownership = if inputs.is_empty() {
            vec![]
        } else {
            vec![
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            inputs.len() as u32,
            inputs.into_iter().map(exp_argument).collect(),
            if output == CONTEXT {
                SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore)
            } else {
                fp_direct(output)
            },
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap();
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        });
    }
    let request = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        source.roots().to_vec(),
    )
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let mut roots = base.roots.into_vec();
    if !helper {
        roots[0].access_sources[0].semantic_block = 2;
        roots[0].access_sources[0].semantic_statement = Some(0);
    }
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        materialized,
        roots,
    )
    .unwrap()
}
