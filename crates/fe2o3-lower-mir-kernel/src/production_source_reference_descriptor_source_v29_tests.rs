// Freshly admitted ordinary source, through the same module/root entry as
// nominal kernels. No synthetic canonical owner or handwritten source receipt.
use fe2o3_mir_model::semantic_mir_v1::{SemanticMemoryLoadV1, SemanticMemoryStoreV1};
#[derive(Clone, Copy, Debug)]
struct DescriptorCase {
    write: bool,
    explicit: bool,
    metadata_length: bool,
    looped: bool,
    foreign_extent: bool,
    changed_index: bool,
    bypass: bool,
    atomic: bool,
}
impl DescriptorCase {
    const READ: Self = Self {
        write: false,
        explicit: false,
        metadata_length: false,
        looped: false,
        foreign_extent: false,
        changed_index: false,
        bypass: false,
        atomic: false,
    };
}

fn descriptor_source_owner(case: DescriptorCase) -> ProductionSemanticSsaOwnerV1 {
    let original = module_fixture_owner(ModuleFixture::Ordinary);
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(
        types
            .iter()
            .position(|ty| {
                matches!(
                    ty.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                )
            })
            .unwrap() as u32,
    );
    let word = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
        None,
    );
    let slice = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: U32 },
        None,
    );
    let pointer = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                slice,
                SemanticPointerKindV1::Reference,
                if case.write {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
        None,
    );
    types[pointer.index() as usize] = types[pointer.index() as usize]
        .clone()
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        if case.write {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        } else {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
    let scalar = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let first = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            (!case.write).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            !case.write,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let pointer_abi = || {
        SemanticAbiValueV1::new(
            pointer,
            SemanticAbiPassModeV1::Pair {
                first,
                second: scalar,
            },
        )
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        vec![
            SemanticAbiArgumentV1::source(pointer_abi()),
            SemanticAbiArgumentV1::source(pointer_abi()),
            SemanticAbiArgumentV1::source(value_abi(&types, word)),
        ],
        value_abi(&types, UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(
        vec![
            if case.write {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            };
            2
        ]
        .into_iter()
        .chain([SemanticSourceArgumentOwnershipV1::ByValue])
        .collect(),
    )
    .unwrap();
    let copy = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let constant = |ty, bits, bytes| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, bytes).unwrap()),
        ))
    };
    let holder = if case.foreign_extent { 2 } else { 1 };
    let metadata = if case.metadata_length {
        SemanticRvalueKindV1::Length(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(holder),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice)
                        .unwrap(),
                ],
                slice,
            )
            .unwrap(),
        )
    } else {
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: copy(holder, pointer),
        }
    };
    let selected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let volatility = if case.atomic {
        SemanticVolatilityV1::NonVolatile
    } else {
        SemanticVolatilityV1::Volatile
    };
    let atomic = case.atomic.then_some(
        fe2o3_mir_model::semantic_mir_v1::SemanticAtomicAccessV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticAtomicOrderingV1::Relaxed,
            fe2o3_mir_model::semantic_mir_v1::SemanticAtomicScopeV1::Device,
        ),
    );
    let effect = if case.write {
        if case.explicit {
            SemanticStatementV1::new(
                source.functions()[0].blocks()[0].statements()[0].source(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    selected,
                    constant(U32, 17, 4),
                    volatility,
                    atomic,
                )),
            )
        } else {
            assign(selected, SemanticRvalueKindV1::Use(constant(U32, 17, 4)))
        }
    } else {
        assign(
            place(6, U32),
            if case.explicit {
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(selected, volatility, atomic))
            } else {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(selected))
            },
        )
    };
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let mut effects = Vec::new();
    if case.changed_index {
        effects.push(assign(
            place(3, word),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: copy(3, word),
                right: constant(word, 1, 8),
            },
        ));
    }
    effects.push(effect);
    if case.looped {
        effects.push(assign(
            place(3, word),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: copy(3, word),
                right: constant(word, 1, 8),
            },
        ));
    }
    let guard = SemanticTerminatorKindV1::Assert {
        condition: copy(5, boolean),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: copy(4, word),
            index: copy(3, word),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let mut blocks = vec![
        block(
            211,
            vec![
                assign(place(4, word), metadata),
                assign(
                    place(5, boolean),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: copy(3, word),
                        right: copy(4, word),
                    },
                ),
            ],
            if case.bypass {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
            } else {
                guard
            },
        ),
        block(
            212,
            effects,
            if case.looped {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0))
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
    ];
    // The write-only case has no read temporary in its exact source roster.
    let mut locals = vec![
        local(202, UNIT, SemanticLocalRoleV1::Return),
        local(203, pointer, SemanticLocalRoleV1::Argument(0)),
        local(204, pointer, SemanticLocalRoleV1::Argument(1)),
        local(205, word, SemanticLocalRoleV1::Argument(2)),
        local(206, word, SemanticLocalRoleV1::Temporary),
        local(207, boolean, SemanticLocalRoleV1::Temporary),
    ];
    if !case.write {
        locals.push(local(208, U32, SemanticLocalRoleV1::Temporary));
    }
    let function = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        abi,
        locals,
        std::mem::take(&mut blocks),
    )
    .with_kernel_entry(source.functions()[0].kernel_entry().unwrap().clone());
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn runtime_descriptor_source_fixtures_have_exact_reachable_rosters_and_original_ssa() {
    for case in [
        DescriptorCase::READ,
        DescriptorCase {
            write: true,
            ..DescriptorCase::READ
        },
        DescriptorCase {
            looped: true,
            ..DescriptorCase::READ
        },
        DescriptorCase {
            foreign_extent: true,
            ..DescriptorCase::READ
        },
    ] {
        let owner = descriptor_source_owner(case);
        assert_eq!(owner.source_semantic().functions().len(), 1);
        assert_eq!(
            owner.source_semantic().roots(),
            &[SemanticFunctionIdV1::from_index(0)]
        );
        assert_eq!(owner.source_semantic().types().len(), 6);
    }
}

include!("production_descriptor_failure_tail_v1766_tests.rs");
include!("production_source_slice_reborrow_events_v29_tests.rs");
