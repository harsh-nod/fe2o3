//! Synthetic ABI fixture only. Full executable source translation remains a
//! separate lower-continuation check; this fixture makes no body-equivalence claim.
use crate::source_argument_v1::*;
use crate::*;
use fe2o3_kernel_ir as kir;
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) struct SourceFixture {
    pub owner: ProductionSemanticSsaOwnerV1,
    pub association: SemanticKirFunctionCorrespondenceV1,
    direct: Vec<SemanticKirParameterBindingV1>,
}

impl SourceFixture {
    pub fn new(read: bool) -> Self {
        let id = SemanticTypeIdV1::from_index;
        let scalar = |bits, size, max| {
            SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, size),
                SemanticScalarValidityRangeV1::new(0, max),
            )
        };
        let pair = SemanticBackendReprV1::ScalarPair {
            first: SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX as u128),
            ),
            second: scalar(64, 8, u64::MAX as u128),
        };
        let layout = |size, align, repr| {
            SemanticTypeLayoutV1::new_with_backend_repr(size, align, repr, false).unwrap()
        };
        let ty = |tag: u8, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 10; 32]),
                layout,
                shape,
            )
        };
        let pointer = |mutable| {
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    id(2),
                    SemanticPointerKindV1::Reference,
                    mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            )
        };
        let reference = |tag, mutable| {
            let kind = match mutable {
                SemanticMutabilityV1::Mutable => {
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                }
                SemanticMutabilityV1::Immutable => {
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                }
            };
            ty(tag, layout(Some(16), 8, pair), pointer(mutable)).with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false)
                    .with_rustc_layout_is_noundef(true)
                    .with_scalar_pointee_info(
                        Some(SemanticAbiPointeeInfoV1::new(kind, 0, 4).unwrap()),
                        None,
                    ),
            )
        };
        let mut types = vec![
            ty(
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
            ty(
                2,
                layout(
                    Some(4),
                    4,
                    SemanticBackendReprV1::scalar(scalar(32, 4, u32::MAX as u128)),
                ),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            ty(
                3,
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    0,
                    4,
                    SemanticFieldsShapeV1::Array {
                        stride_bytes: 4,
                        count: 0,
                    },
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
                SemanticTypeShapeV1::Slice { element: id(1) },
            ),
            reference(4, SemanticMutabilityV1::Mutable),
        ];
        if read {
            types.push(reference(5, SemanticMutabilityV1::Immutable));
        }
        let count = if read { 2 } else { 1 };
        let arguments = (0..count)
            .map(|i| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    id(3 + i),
                    SemanticAbiPassModeV1::Pair {
                        first: SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                i != 0,
                                (i != 0).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                                true,
                                i != 0,
                                false,
                                true,
                            ),
                            SemanticAbiExtensionV1::None,
                            0,
                            Some(4),
                        )
                        .unwrap(),
                        second: SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                false, None, false, false, false, true,
                            ),
                            SemanticAbiExtensionV1::None,
                            0,
                            None,
                        )
                        .unwrap(),
                    },
                ))
            })
            .collect();
        let target = SemanticLayoutIdentityV1::from_sha256([30; 32]);
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([31; 32]),
            target,
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            count,
            arguments,
            SemanticAbiValueV1::new(id(0), SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(if read {
            vec![
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
            ]
        } else {
            vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow]
        })
        .unwrap();
        let source = SemanticSourceProvenanceV1::unavailable();
        let mut locals = vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([40; 32]),
            id(0),
            SemanticLocalRoleV1::Return,
            source,
        )];
        for i in 0..count {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([41 + i as u8; 32]),
                id(3 + i),
                SemanticLocalRoleV1::Argument(i),
                source,
            ));
        }
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([50; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([55; 32]),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"entry".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([56; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ));
        let root = SemanticFunctionIdV1::from_index(0);
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(target),
            types,
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![root],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        Self {
            owner,
            association: SemanticKirFunctionCorrespondenceV1 {
                correspondence_owner: root,
                semantic_function: root,
                kernel_ir_function: kir::FunctionId::new("entry"),
                role: SemanticKirFunctionRoleV1::KernelEntry,
            },
            direct: (0..count)
                .map(|i| SemanticKirParameterBindingV1 {
                    correspondence_owner: root,
                    semantic_function: root,
                    semantic_local: SemanticLocalIdV1::from_index(1 + i),
                    kernel_ir_value: kir::ValueId(i),
                })
                .collect(),
        }
    }

    pub fn relation<'source, 'work>(
        &'source self,
        module: &'source kir::Module,
        budget: &mut kir::CanonicalKernelIrVerificationResourceBudgetV1<'work>,
    ) -> ProductionSourceArgumentRelationV1<'source, 'work> {
        self.relation_with_association(module, &self.association, budget)
    }

    pub fn relation_with_association<'source, 'work>(
        &'source self,
        module: &'source kir::Module,
        association: &'source SemanticKirFunctionCorrespondenceV1,
        budget: &mut kir::CanonicalKernelIrVerificationResourceBudgetV1<'work>,
    ) -> ProductionSourceArgumentRelationV1<'source, 'work> {
        self.owner
            .check_source_arguments_v1(
                kir::verify_module_ref(module).unwrap(),
                association,
                ArgumentTraceV1 {
                    direct: &self.direct,
                    components: &[],
                    ignored: &[],
                },
                budget,
            )
            .unwrap()
    }
}
