use super::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
fn ty(i: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(i)
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let layout = |size, backend| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            if size == 0 { 1 } else { 8 },
            backend,
            false,
        )
        .unwrap()
    };
    let decl = |i: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([i; 32]),
            SemanticLayoutIdentityV1::from_sha256([i; 32]),
            layout,
            shape,
        )
    };
    vec![
        decl(
            1,
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
        decl(
            2,
            layout(
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
            ),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        decl(
            3,
            layout(
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(1),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}

fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}

// A synthetic ordinary-reference component, not a device capability issuer.
fn owner() -> ProductionSemanticSsaOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let assign = |local, kind, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, kind),
                SemanticRvalueV1::new(ty(kind), value),
            )),
        )
    };
    let statements = vec![
        assign(
            1,
            1,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty(1),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 8).unwrap()),
            ))),
        ),
        assign(
            2,
            2,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, 1),
            },
        ),
        assign(
            3,
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, 2))),
        ),
        assign(
            4,
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, 2))),
        ),
        SemanticStatementV1::new(source, SemanticStatementKindV1::Nop),
    ];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let body = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([21; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let locals = [0, 1, 2, 2, 2]
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([30 + i as u8; 32]),
                ty(kind),
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![body],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"reference_use_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([45; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let semantic = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn operand(body: &SemanticFunctionDeclV1, statement: usize) -> &SemanticOperandV1 {
    let SemanticStatementKindV1::Assign(a) = body.blocks()[0].statements()[statement].kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::Use(value) = a.value().kind() else {
        panic!()
    };
    value
}

#[test]
fn reference_receipt_preserves_exact_original_copy_move_event_windows() {
    let owner = owner();
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    for statement in [2, 3] {
        let site = ProductionSemanticSsaSourceSiteV1::new(
            SemanticBlockIdV1::from_index(0),
            Some(statement),
        );
        let operand = operand(view.body(), statement as usize);
        let direct = query.operand_use(site, operand, &mut || true).unwrap();
        let actual = ssa_use(&query, site, operand, &mut |_| Ok(())).unwrap();
        assert_eq!(actual.site, site);
        assert_eq!(actual.ty, ty(2));
        assert_eq!(actual.variable, direct.variable().get());
        assert_eq!(actual.value, direct.value());
        assert_eq!(
            actual.event_range,
            [direct.event_range().start, direct.event_range().end]
        );
        assert_eq!(actual.agreeing_uses, direct.agreeing_uses());
        assert!(actual.agreeing_uses > 0);
    }
}

#[test]
fn changed_operand_site_owner_and_constant_never_gain_source_receipt() {
    let owner = owner();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(0), Some(2));
    let original = operand(view.body(), 2);
    let clone = original.clone();
    let other_owner = self::owner();
    let other = other_owner.execution_view_for_root(ROOT).unwrap();
    for value in [&clone, operand(other.body(), 2), operand(view.body(), 0)] {
        let result = ssa_use(&query, site, value, &mut |_| Ok(()));
        assert!(
            matches!(
                result,
                Err(E::Unsupported {
                    detail: "BF16 guarded event lost its original scalar SSA source",
                    ..
                })
            ),
            "{result:?}"
        );
    }
    let wrong = ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(0), Some(4));
    let result = ssa_use(&query, wrong, original, &mut |_| Ok(()));
    assert!(
        matches!(
            result,
            Err(E::Unsupported {
                detail: "BF16 guarded event lost its original scalar SSA source",
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn reference_query_preserves_shared_work_error_and_exact_success_boundary() {
    use fe2o3_lower_mir_kernel::ProductionSemanticKirResourceV1;
    let owner = owner();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(0), Some(2));
    let operand = operand(view.body(), 2);
    let mut count = 0usize;
    ssa_use(&query, site, operand, &mut |n| {
        count += n;
        Ok(())
    })
    .unwrap();
    assert!(count > 1);
    let mut remaining = count - 1;
    let result = ssa_use(&query, site, operand, &mut |n| {
        remaining = remaining.checked_sub(n).ok_or(E::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual: count,
            limit: count - 1,
        })?;
        Ok(())
    });
    assert!(
        matches!(result,Err(E::ResourceLimit{resource:ProductionSemanticKirResourceV1::AnalysisWork,actual,limit})
        if actual==count && limit==count-1),
        "{result:?}"
    );
    assert_eq!(remaining, 0);
}

#[test]
fn reference_edge_rejects_raw_mutable_metadata_width_space_and_pointee_substitution() {
    for case in 0..8 {
        let mut types = types();
        let p = SemanticPointerTypeV1::new_with_kind(
            if case == 6 {
                ty(0)
            } else if case == 7 {
                ty(99)
            } else {
                ty(1)
            },
            if case == 1 {
                SemanticPointerKindV1::Raw
            } else {
                SemanticPointerKindV1::Reference
            },
            if case == 2 {
                SemanticMutabilityV1::Mutable
            } else {
                SemanticMutabilityV1::Immutable
            },
            if case == 3 { 1 } else { 0 },
            if case == 4 { 32 } else { 64 },
            if case == 5 {
                SemanticPointerMetadataV1::SliceLength
            } else {
                SemanticPointerMetadataV1::None
            },
        )
        .unwrap();
        types[2] = SemanticTypeDeclV1::new(
            types[2].identity(),
            types[2].layout_identity(),
            types[2].layout().clone(),
            SemanticTypeShapeV1::Pointer(p),
        );
        assert_eq!(
            exact_shared_edge(&types, ty(2), ty(1), SemanticPointerMetadataV1::None),
            case == 0,
            "case {case}"
        );
    }
}
