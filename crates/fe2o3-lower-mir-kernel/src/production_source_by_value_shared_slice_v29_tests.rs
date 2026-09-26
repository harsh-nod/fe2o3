use super::*;

#[path = "production_source_shared_slice_return_v29_tests.rs"]
mod shared_slice_return_v29;

#[derive(Clone, Copy, Debug)]
enum Leaf {
    Shared,
    Raw,
    Mutable,
    Thin,
    Global,
    Local32,
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Field,
    Array,
    Nested,
}

fn owner(leaf: Leaf, shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let template = descriptor_source_owner(DescriptorCase::READ);
    let semantic = template.source_semantic();
    let original = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let shared = original.abi().source_input_types()[0];
    let SemanticTypeShapeV1::Pointer(shared_pointer) = types[shared.index() as usize].shape()
    else {
        panic!("original shared slice reference");
    };
    let slice = shared_pointer.pointee();
    let leaf = if matches!(leaf, Leaf::Shared) {
        shared
    } else {
        let raw = matches!(leaf, Leaf::Raw);
        let thin = matches!(leaf, Leaf::Thin);
        let mutable = matches!(leaf, Leaf::Mutable);
        let (space, bytes) = match leaf {
            Leaf::Global => (1, 8),
            Leaf::Local32 => (3, 4),
            _ => (0, 8),
        };
        let first = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(space, bytes, bytes),
            SemanticScalarValidityRangeV1::new(u128::from(!raw), (1u128 << (bytes * 8)) - 1),
        );
        let second = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let id = declaration(
            &mut types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(if thin { bytes } else { 16 }),
                if thin { bytes } else { 8 },
                if thin {
                    SemanticBackendReprV1::scalar(first)
                } else {
                    SemanticBackendReprV1::scalar_pair(first, second)
                },
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    if thin { U32 } else { slice },
                    if raw {
                        SemanticPointerKindV1::Raw
                    } else {
                        SemanticPointerKindV1::Reference
                    },
                    if mutable {
                        SemanticMutabilityV1::Mutable
                    } else {
                        SemanticMutabilityV1::Immutable
                    },
                    space,
                    (bytes * 8) as u16,
                    if thin {
                        SemanticPointerMetadataV1::None
                    } else {
                        SemanticPointerMetadataV1::SliceLength
                    },
                )
                .unwrap(),
            ),
            None,
        );
        types[id.index() as usize] = types[id.index() as usize]
            .clone()
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            if raw {
                                SemanticAbiPointeeKindV1::Raw
                            } else if mutable {
                                SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                            } else {
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                            },
                            if thin { 4 } else { 0 },
                            if raw { 1 } else { 4 },
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            );
        id
    };
    let size = types[leaf.index() as usize].layout().size_bytes().unwrap();
    let alignment = types[leaf.index() as usize].layout().alignment_bytes();
    let argument = match shape {
        Shape::Field => aggregate(
            &mut types,
            vec![leaf],
            vec![0],
            size,
            alignment,
            SemanticBackendReprV1::memory(true),
            None,
        ),
        Shape::Array | Shape::Nested => {
            let array = declaration(
                &mut types,
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    size * 2,
                    alignment,
                    SemanticFieldsShapeV1::array(size, 2),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    alignment,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Array {
                    element: leaf,
                    length: 2,
                },
                None,
            );
            if matches!(shape, Shape::Array) {
                array
            } else {
                aggregate(
                    &mut types,
                    vec![leaf, array],
                    vec![0, size],
                    size * 3,
                    alignment,
                    SemanticBackendReprV1::memory(true),
                    None,
                )
            }
        }
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value_abi(&types, argument))],
        original.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let mut locals = vec![
        local(202, UNIT, SemanticLocalRoleV1::Return),
        local(203, argument, SemanticLocalRoleV1::Argument(0)),
    ];
    // Preserve the template's complete admitted type closure without adding
    // physical parameters or reading unrelated original types.
    locals.extend(
        original
            .locals()
            .iter()
            .enumerate()
            .map(|(index, original)| {
                local(
                    220 + index as u8,
                    original.ty(),
                    SemanticLocalRoleV1::Temporary,
                )
            }),
    );
    let root = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        abi,
        locals,
        vec![block(211, vec![], SemanticTerminatorKindV1::Return)],
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    super::super::fixtures::build(
        types,
        vec![root],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
    )
}

fn expected_paths(shape: Shape) -> Vec<Vec<SemanticKirParameterProjectionV1>> {
    use SemanticKirParameterProjectionV1::{ArrayIndex, Field};
    match shape {
        Shape::Field => vec![vec![Field(0)]],
        Shape::Array => vec![vec![ArrayIndex(0)], vec![ArrayIndex(1)]],
        Shape::Nested => vec![
            vec![Field(0)],
            vec![Field(1), ArrayIndex(0)],
            vec![Field(1), ArrayIndex(1)],
        ],
    }
}

#[test]
fn original_by_value_shared_slice_fields_arrays_and_nested_components_remain_generic() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        let owner = owner(Leaf::Shared, shape);
        let source = owner.source_semantic();
        let root = &source.functions()[0];
        let argument = root.abi().source_input_types()[0];
        let KernelParameterShapeV1::Components(components) =
            kernel_parameter_shape_v1(source, root, 0, argument).unwrap()
        else {
            panic!("by-value original aggregate keeps exact component structure");
        };
        let paths = expected_paths(shape);
        assert_eq!(components.len(), paths.len());
        for (ordinal, ((path, ty, physical, offset, words), expected)) in
            components.iter().zip(&paths).enumerate()
        {
            assert_eq!(path, expected);
            assert!(shared_slice_leaf_v1(source.types(), *ty));
            assert!(matches!(physical, Type::Slice(slice)
                if slice.address_space == AddressSpace::Generic && slice.access == AccessMode::ReadOnly));
            assert_eq!(*offset, ordinal as u64 * 16);
            let ParameterAbiLeafV1::SharedSlicePair { first, second } = words else {
                panic!("original slice data and metadata words remain correlated");
            };
            assert!(matches!(
                first.primitive(),
                SemanticBackendPrimitiveV1::Pointer {
                    address_space: 0,
                    size_bytes: 8,
                    alignment_bytes: 8
                }
            ));
            assert!(matches!(
                second.primitive(),
                SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 64,
                    alignment_bytes: 8
                }
            ));
        }
        // The ordinary pointer-free API gains no transport authority.
        let refused = lower_by_value_abi_components_v1(
            source.types(),
            root,
            root.abi().arguments()[0].value(),
            ParameterLeafPolicyV1::PointerFree,
        );
        assert!(matches!(
            refused,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "embedded pointer kernel arguments have no owned region binding",
                ..
            })
        ));
        for (ordinal, ty) in [(1, argument), (0, UNIT)] {
            let refused =
                lower_by_value_kernel_parameter_components_v1(source.types(), root, ordinal, ty);
            assert!(matches!(
                refused,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "aggregate kernel argument lacks exact by-value ABI ownership",
                    ..
                })
            ));
        }
    }
}

#[test]
fn original_by_value_raw_mutable_thin_and_non_generic_pointer_leaves_still_refuse() {
    for leaf in [
        Leaf::Raw,
        Leaf::Mutable,
        Leaf::Thin,
        Leaf::Global,
        Leaf::Local32,
    ] {
        for shape in [Shape::Field, Shape::Array, Shape::Nested] {
            // Full semantic/type/ABI admission occurs before the representation
            // query; malformed input or an earlier source refusal cannot pass.
            let owner = owner(leaf, shape);
            let source = owner.source_semantic();
            let root = &source.functions()[0];
            let argument = root.abi().source_input_types()[0];
            let refused = kernel_parameter_shape_v1(source, root, 0, argument);
            assert!(
                matches!(
                    refused,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "embedded pointer kernel arguments have no owned region binding",
                        ..
                    })
                ),
                "{leaf:?}, {shape:?}: exact embedded-pointer refusal required"
            );
        }
    }
}

#[test]
fn original_by_value_shared_slice_transport_reaches_consuming_root_without_global_binding() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let mut completed = false;
            with_pending_api_owner_v18(
                ModuleFixture::Ordinary,
                preexisting,
                &mut budget,
                || owner(Leaf::Shared, shape),
                |owner, launch, input, _, budget| {
                    let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
                    let roots = fixture.roots();
                    assert!(matches!(
                        roots[0].arguments[0].kind,
                        ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { .. }
                    ));
                    let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                        owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        ProductionSemanticKirLimitsV1::default(), budget).unwrap();
                    prepared.with_checked_source_v18(budget, |source, budget| {
                        let (_, root) = source.root(0, budget)?;
                        let canonical = source.canonical(budget)?;
                        let root = &canonical.module().functions[root];
                        assert_eq!(root.signature.parameters.len(), expected_paths(shape).len());
                        for ty in &root.signature.parameters {
                            assert!(matches!(ty, Type::Slice(slice)
                                if slice.address_space == AddressSpace::Generic && slice.access == AccessMode::ReadOnly));
                        }
                        completed = true;
                        Ok(())
                    }).unwrap();
                },
            );
            assert!(completed, "{shape:?}, preexisting={preexisting}");
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}
