fn source_domain_width_owner_v30(
    signed: bool,
    bits: u16,
    default_true: bool,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;

    let base = global_native_cross_descriptor_copy_owner_v18();
    let source = base.source_semantic();
    let function = &source.functions()[0];
    let mut types = source.types().to_vec();
    let discriminator = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    let bytes = u64::from(bits / 8);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    ));
    for declaration in &mut types {
        let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
            continue;
        };
        *declaration = SemanticTypeDeclV1::new(
            declaration.identity(),
            declaration.layout_identity(),
            declaration.layout().clone(),
            SemanticTypeShapeV1::Enum {
                discriminant: discriminator,
                variants: variants.clone(),
            },
        )
        .with_rustc_abi_properties(declaration.abi_properties())
        .with_rust_type_kind(declaration.rust_type_kind());
    }
    let mut locals = function.locals().to_vec();
    let mut blocks = function.blocks().to_vec();
    let mut guards = 0;
    for block in &mut blocks {
        let mut statements = block.statements().to_vec();
        let mut changed = None;
        for statement in &mut statements {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(_) = assignment.value().kind() else {
                continue;
            };
            let local = assignment.destination().local();
            assert!(assignment.destination().projections().is_empty());
            let old = &locals[local.index() as usize];
            locals[local.index() as usize] =
                SemanticLocalDeclV1::new(old.identity(), discriminator, old.role(), old.source());
            *statement = SemanticStatementV1::new(
                statement.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(local, vec![], discriminator).unwrap(),
                    SemanticRvalueV1::new(discriminator, assignment.value().kind().clone()),
                )),
            );
            changed = Some(local);
        }
        let Some(local) = changed else {
            continue;
        };
        let SemanticTerminatorKindV1::SwitchInt { targets, .. } = block.terminator().kind() else {
            panic!("original issued discriminant switch");
        };
        assert_eq!(targets.values().len(), 1);
        assert_eq!(targets.values()[0].value(), 1);
        let targets = if default_true {
            SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        targets.otherwise().target(),
                    ),
                )],
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    targets.values()[0].edge().target(),
                ),
            )
            .unwrap()
        } else {
            targets.clone()
        };
        *block = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            statements,
            SemanticTerminatorV1::new(
                block.terminator().source(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(local, vec![], discriminator).unwrap(),
                    ),
                    targets,
                },
            ),
        )
        .unwrap();
        guards += 1;
    }
    assert_eq!(
        guards, 2,
        "both genuine descriptor guards change representation"
    );
    let root = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(function.kernel_entry().unwrap().clone());
    let request = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(request, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn source_domain_widths_v30() -> [(bool, u16, fe2o3_kernel_ir::ScalarType); 8] {
    use fe2o3_kernel_ir::ScalarType::*;
    [
        (false, 8, U8),
        (false, 16, U16),
        (false, 32, U32),
        (false, 64, U64),
        (true, 8, I8),
        (true, 16, I16),
        (true, 32, I32),
        (true, 64, I64),
    ]
}

fn run_source_domain_width_v30(
    signed: bool,
    bits: u16,
    scalar: fe2o3_kernel_ir::ScalarType,
    default_true: bool,
    fault: Option<(bool, u8)>,
    work: usize,
    storage: usize,
    observed: &std::cell::Cell<[usize; 2]>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let owner = source_domain_width_owner_v30(signed, bits, default_true);
    let abi = issued_descriptor_role_abi_v18(&owner);
    run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        work,
        storage,
        |original, optimized, budget| {
            slice_view_v1::test_source_domain_width_v30(
                original,
                optimized,
                budget,
                scalar,
                default_true,
                fault,
                observed,
            )
        },
    )
}

#[test]
fn source_domain_join_legacy_switch_all_integer_widths_reads_and_stores() {
    for (signed, bits, scalar) in source_domain_widths_v30() {
        for default_true in [false, true] {
            let observed = std::cell::Cell::new([0; 2]);
            let (result, _, _) = run_source_domain_width_v30(
                signed,
                bits,
                scalar,
                default_true,
                None,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                &observed,
            );
            result.unwrap();
            assert_eq!(observed.get(), [1, 1], "{scalar:?}, default={default_true}");
        }
    }
}

#[test]
fn source_domain_join_legacy_switch_widths_reject_exact_guard_substitutions() {
    for (signed, bits, scalar) in source_domain_widths_v30() {
        for writing in [false, true] {
            for fault in 0..4 {
                let observed = std::cell::Cell::new([0; 2]);
                let result = run_source_domain_width_v30(
                    signed,
                    bits,
                    scalar,
                    false,
                    Some((writing, fault)),
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    &observed,
                )
                .0;
                assert_eq!(
                    observed.get(),
                    [1, 1],
                    "positive baseline precedes mutation"
                );
                let expected = match (writing, fault) {
                    (false, 3) => "pending global read changed exact local domain",
                    (false, _) => "pending global read changed exact control transport",
                    (true, 3) => "slice Store exact domain differs",
                    (true, _) => "slice Store guard transport differs",
                };
                assert!(
                    matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
                    "{scalar:?} writing={writing} fault={fault}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn source_domain_join_legacy_switch_widths_preserve_exact_transaction_limits() {
    for (signed, bits, scalar) in source_domain_widths_v30() {
        let observed = std::cell::Cell::new([0; 2]);
        let (result, work, storage) = run_source_domain_width_v30(
            signed,
            bits,
            scalar,
            false,
            None,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        );
        result.unwrap();
        assert_eq!(observed.get(), [1, 1]);
        observed.set([0; 2]);
        let (result, exact_work, exact_storage) = run_source_domain_width_v30(
            signed, bits, scalar, false, None, work, storage, &observed,
        );
        result.unwrap();
        assert_eq!(
            (exact_work, exact_storage, observed.get()),
            (work, storage, [1, 1])
        );
        let work_short = run_source_domain_width_v30(
            signed,
            bits,
            scalar,
            false,
            None,
            work - 1,
            storage,
            &std::cell::Cell::new([0; 2]),
        )
        .0;
        assert!(
            matches!(work_short, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
            if error.limit() == work - 1 && error.actual() > error.limit()),
            "{work_short:?}"
        );
        let storage_short = run_source_domain_width_v30(
            signed,
            bits,
            scalar,
            false,
            None,
            work,
            storage - 1,
            &std::cell::Cell::new([0; 2]),
        )
        .0;
        assert!(
            matches!(storage_short, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
            if error.limit() == storage - 1 && error.actual() > error.limit()),
            "{storage_short:?}"
        );
    }
}
