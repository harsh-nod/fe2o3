#![cfg(test)]
//! Admitted component models, not original rustc/collector or conditional proofs.
use super::*;
use crate::collector::TypedArgumentListV1;
use crate::compiler_descriptor::{AccessMode, ScalarTypeV1, TypedDescriptorArgumentV1};
use fe2o3_artifacts::*;
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain,
    LaunchExtent, ScalarType, Signature, Terminator, ValueId, WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use reserved_fe2o3_symbols::KernelBindingIdV1;

#[allow(dead_code)]
mod existing {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/defined_helper_semantic_fixture_v1.rs"
    ));
}
#[allow(dead_code)]
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
pub(super) mod contracts;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn scalar(
    primitive: SemanticBackendPrimitiveV1,
    start: u128,
    end: u128,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(start, end))
}

fn types(unit: SemanticTypeDeclV1) -> Vec<SemanticTypeDeclV1> {
    use SemanticBackendReprV1 as Repr;
    use SemanticTypeShapeV1 as Shape;
    let reference_pointer = scalar(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        1,
        u64::MAX.into(),
    );
    // Raw pointer fields admit null; a reference's non-null niche is not theirs.
    let raw_pointer = scalar(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        0,
        u64::MAX.into(),
    );
    let word = scalar(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        0,
        u64::MAX.into(),
    );
    let pair = || {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            Repr::scalar_pair(reference_pointer, word),
            false,
        )
        .unwrap()
    };
    let declaration = |id, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([id; 32]),
            SemanticLayoutIdentityV1::from_sha256([id + 20; 32]),
            layout,
            shape,
        )
    };
    vec![
        unit,
        declaration(
            11,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                Repr::scalar(scalar(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    0,
                    u32::MAX.into(),
                )),
                false,
            )
            .unwrap(),
            Shape::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ),
        declaration(
            12,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::Array {
                    stride_bytes: 4,
                    count: 0,
                },
                SemanticRustcVariantsV1::Single { index: 0 },
                Repr::memory(false),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            Shape::Slice { element: ty(1) },
        ),
        declaration(
            13,
            pair(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(2),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
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
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
        declaration(
            14,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                Repr::scalar_pair(raw_pointer, word),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            Shape::Aggregate(SemanticAggregateTypeV1::new(vec![ty(5), ty(6)]).unwrap()),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
        declaration(
            15,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                Repr::scalar(raw_pointer),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(1),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
        declaration(
            16,
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, Repr::scalar(word), false)
                .unwrap(),
            Shape::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
    ]
}

pub(super) fn source(count: usize) -> Semantic {
    let original = existing::source(vec![existing::subtract()]);
    let types = types(original.types()[0].clone());
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared_attrs = SemanticAbiValueAttributesV1::new(
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
        Some(4),
    )
    .unwrap();
    let functions = (0..count)
        .map(|i| {
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([60 + i as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([70; 32]),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                3,
                [ty(3), ty(3), ty(4)]
                    .into_iter()
                    .map(|t| {
                        SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                            t,
                            SemanticAbiPassModeV1::Pair {
                                first: if t == ty(4) { attrs } else { shared_attrs },
                                second: attrs,
                            },
                        ))
                    })
                    .collect(),
                SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ])
            .unwrap();
            let locals = [ty(0), ty(3), ty(3), ty(4)]
                .into_iter()
                .enumerate()
                .map(|(n, t)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([80 + n as u8; 32]),
                        t,
                        if n == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Argument((n - 1) as u32)
                        },
                        provenance,
                    )
                })
                .collect();
            let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([90 + i as u8; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([100 + i as u8; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([110 + i as u8; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([120; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([121; 32]),
                provenance,
                abi,
                locals,
                SemanticBlockIdV1::from_index(0),
                vec![existing::block(
                    122,
                    vec![],
                    SemanticTerminatorKindV1::Return,
                )],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(format!("kernel{i}").into_bytes()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([13 + i as u8; 32]),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            ))
        })
        .collect();
    InertSemanticMirRequestV1::new(
        original.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        (0..count)
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn conditional_descriptor_v5_fixture_distinguishes_raw_and_reference_validity() {
    let source = source(1);
    for (index, start) in [(3, 1), (4, 0), (5, 0)] {
        let first = match *source.types()[index].layout().backend_repr() {
            SemanticBackendReprV1::Scalar(first)
            | SemanticBackendReprV1::ScalarPair { first, .. } => first,
            other => panic!("unexpected fixture representation: {other:?}"),
        };
        assert_eq!(
            first.valid_range(),
            Some(SemanticScalarValidityRangeV1::new(start, u64::MAX.into())),
        );
    }

    // Recreate the original bad raw-pointer leaf. Admission must reject it,
    // not just report a later descriptor/contract mismatch.
    let mut types = source.types().to_vec();
    let raw = &types[5];
    types[5] = SemanticTypeDeclV1::new(
        raw.identity(),
        raw.layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                1,
                u64::MAX.into(),
            )),
            false,
        )
        .unwrap(),
        raw.shape().clone(),
    )
    .with_rustc_abi_properties(raw.abi_properties());
    let invalid = InertSemanticMirRequestV1::new(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        source.functions().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default());
    assert!(matches!(
        invalid,
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn layout(output: bool) -> RustLayoutEvidenceV1 {
    let scalar = RustScalarElementTypeV1::F32;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(if output {
            RustSourceTypeShapeV1::disjoint_slice(scalar, RustDisjointIndexSpaceV1::Index1D)
        } else {
            RustSourceTypeShapeV1::shared_slice(scalar)
        }),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        vec![
            RustPhysicalComponentV1::new(
                0,
                8,
                8,
                RustPhysicalComponentKindV1::Pointer {
                    mutability: if output {
                        RustPointerMutabilityV1::Mut
                    } else {
                        RustPointerMutabilityV1::Const
                    },
                    pointee: scalar,
                },
            )
            .unwrap(),
            RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
        ],
    )
    .unwrap()
}

pub(super) fn roots(semantic: &Semantic) -> Vec<TypedDescriptorRootV1> {
    semantic
        .roots()
        .iter()
        .map(|root| {
            let function = &semantic.functions()[root.index() as usize];
            let entry = function.kernel_entry().unwrap();
            let name = String::from_utf8(entry.export_symbol().as_bytes().to_vec()).unwrap();
            TypedDescriptorRootV1 {
                logical_name: name.clone(),
                export_name: name,
                kernel_binding: KernelBindingIdV1::from_bytes(
                    *entry.kernel_binding_identity().as_bytes(),
                ),
                arguments: TypedArgumentListV1::new(
                    function
                        .abi()
                        .source_input_types()
                        .iter()
                        .enumerate()
                        .map(|(n, t)| {
                            let output = n == 2;
                            TypedDescriptorArgumentV1 {
                                name: format!("arg{n}"),
                                kind: if output {
                                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32)
                                } else {
                                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32)
                                },
                                access: if output {
                                    AccessMode::WriteOnly
                                } else {
                                    AccessMode::ReadOnly
                                },
                                offset: (n * 16) as u32,
                                layout: Some(layout(output)),
                                source_size: 16,
                                source_alignment: 8,
                                rustc_abi_class: RustcAbiClassV1::ScalarPair,
                                semantic_type_identity: semantic.types()[t.index() as usize]
                                    .identity(),
                            }
                        })
                        .collect(),
                )
                .unwrap(),
                explicit_argument_bytes: 48,
                kernarg_alignment_bytes: 8,
                source_launch: Some(
                    LaunchContract::new(
                        1,
                        BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                        Dimensions::new(128, 1, 1).unwrap(),
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            }
        })
        .collect()
}

pub(super) fn module(count: usize, profile: Profile) -> Module {
    let mut module = Module::new("conditional-descriptor-component");
    for i in 0..count {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let name = format!("kernel{i}");
        module.functions.push(Function::kernel_entry(
            name.clone(),
            Signature::new(
                [
                    KirAccess::ReadOnly,
                    KirAccess::ReadOnly,
                    KirAccess::WriteOnly,
                ]
                .into_iter()
                .map(|access| {
                    Type::slice(Type::Scalar(ScalarType::F32), AddressSpace::Global, access)
                })
                .collect(),
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            name.clone(),
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    dialect_amdgcn::bind_production_target_v1(&module, profile)
        .unwrap()
        .module()
        .clone()
}

pub(super) fn admit(module: &Module) -> Owner {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .unwrap()
        .0
}

pub(super) fn contract_bytes(
    semantic: &Semantic,
    profile: Profile,
    change: impl FnMut(&mut contracts::Fixture),
) -> Vec<Vec<u8>> {
    let mut change = change;
    contracts::with_custom_contracts(
        profile.device_target(),
        semantic.roots().len(),
        2,
        |fixture| {
            fixture.subjects.source_semantic_identity = *semantic.semantic_sha256().as_bytes();
            change(fixture);
        },
        |input| {
            input
                .contracts
                .iter()
                .map(|c| c.canonical_bytes().to_vec())
                .collect()
        },
    )
}
