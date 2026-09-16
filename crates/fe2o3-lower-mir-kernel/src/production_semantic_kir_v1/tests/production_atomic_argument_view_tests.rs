use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiPointeeInfoV1, SemanticAbiPointeeKindV1, SemanticMutabilityV1,
    SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticPointerTypeV1,
    SemanticTypeAbiPropertiesV1,
};

const PTR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const OPAQUE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);

fn atomic_argument_owner(length: u64) -> ProductionPreRankedKirOwnerV1 {
    let original = argument_owner_shape(true, ArgumentTupleShape::Mixed, false, true);
    let mut types = original.source_semantic().types().to_vec();
    let marker = &types[6];
    types[6] = SemanticTypeDeclV1::new(
        marker.identity(),
        marker.layout_identity(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(0, length),
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
        SemanticTypeShapeV1::Array {
            element: UNIT,
            length,
        },
    );
    let backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    ));
    let properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([16; 32]),
            SemanticLayoutIdentityV1::from_sha256([16; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, backend, false).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([17; 32]),
        SemanticLayoutIdentityV1::from_sha256([17; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([18; 32]),
            SemanticLayoutIdentityV1::from_sha256([18; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                backend,
                false,
                SemanticAggregateLayoutV1::new(vec![0, 0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ZERO, PTR, OPAQUE]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
    );
    let noop = noop_semantic_owner(&["atomic_argument_view"]);
    let semantic = noop.semantic();
    let function = &semantic.functions()[0];
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        function.abi().identity(),
        semantic.target().identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        [U32, CARRIER]
            .into_iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Direct(attrs),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
    ])
    .unwrap();
    let mut locals = function.locals().to_vec();
    for (index, ty) in [U32, CARRIER].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([100 + index as u8; 32]),
            ty,
            SemanticLocalRoleV1::Argument(index as u32),
            function.source(),
        ));
    }
    // Keep the reused tuple/PAIR fixture types within the admitted root closure.
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([102; 32]),
        TUPLE,
        SemanticLocalRoleV1::Temporary,
        function.source(),
    ));
    let function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        locals,
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(function.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    materialize_argument_view(
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    )
}

#[test]
fn complete_atomic_argument_view_preserves_markers_without_dereferencing() {
    use ProductionArgumentProjectionV1::{ArrayIndex as A, Field as F};
    for length in [2, 257] {
        let owner = atomic_argument_owner(length);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let root = SemanticFunctionIdV1::from_index(0);
        owner
            .with_checked_arguments_v1(root, root, &mut budget, |view| {
                let physical = view.physical(1)?.unwrap();
                assert_eq!(
                    physical.ty(),
                    &Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    )
                );
                let ProductionArgumentTraceV1::Direct(trace) = physical.trace() else {
                    panic!("the carrier must have one direct whole-local binding");
                };
                assert_eq!(trace.correspondence_owner(), root);
                assert_eq!(trace.semantic_function(), root);
                assert_eq!(trace.semantic_local(), SemanticLocalIdV1::from_index(2));
                assert_eq!(trace.kernel_ir_value(), physical.value());
                assert!(view.physical(2)?.is_none());
                assert!(
                    view.ignored_local(SemanticLocalIdV1::from_index(2))?
                        .is_none()
                );
                view.visit_nodes(|node| {
                    match node.coverage() {
                        ProductionArgumentCoverageV1::Parameter(row)
                        | ProductionArgumentCoverageV1::WithinAtomicParameter(row)
                            if node.source_argument() == 1 =>
                        {
                            assert_eq!(row, physical)
                        }
                        _ => (),
                    }
                    Ok(())
                })?;
                let nodes = collect_argument_nodes(view)?;
                let carrier = nodes
                    .iter()
                    .filter(|node| node.source == 1)
                    .collect::<Vec<_>>();
                let mut expected = vec![
                    (vec![F(0), F(0), F(0)], UNIT, ObservedCoverage::Zero),
                    (vec![F(0), F(0), F(1)], UNIT, ObservedCoverage::Zero),
                    (
                        vec![F(0), F(0)],
                        SemanticTypeIdV1::from_index(5),
                        ObservedCoverage::Zero,
                    ),
                ];
                expected.extend(
                    (0..length)
                        .map(|index| (vec![F(0), F(1), A(index)], UNIT, ObservedCoverage::Zero)),
                );
                expected.extend([
                    (
                        vec![F(0), F(1)],
                        SemanticTypeIdV1::from_index(6),
                        ObservedCoverage::Zero,
                    ),
                    (
                        vec![F(0), F(2)],
                        SemanticTypeIdV1::from_index(7),
                        ObservedCoverage::Zero,
                    ),
                    (vec![F(0)], ZERO, ObservedCoverage::Zero),
                    (vec![F(1)], PTR, ObservedCoverage::Within(1)),
                    (vec![F(2)], OPAQUE, ObservedCoverage::Zero),
                    (vec![], CARRIER, ObservedCoverage::Parameter(1)),
                ]);
                assert_eq!(
                    carrier
                        .iter()
                        .map(|node| (&node.path, node.ty, &node.coverage))
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(|(path, ty, coverage)| (path, *ty, coverage))
                        .collect::<Vec<_>>()
                );
                for node in carrier {
                    assert_eq!(node.adjusted, Some(1));
                    assert_eq!(
                        node.local,
                        Some((SemanticLocalIdV1::from_index(2), node.path.clone()))
                    );
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
