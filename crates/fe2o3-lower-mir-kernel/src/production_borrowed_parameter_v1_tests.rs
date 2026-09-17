use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, VerifiedCanonicalKernelIrModuleV12};
use fe2o3_mir_model::semantic_mir_v1::*;

// Entry-view tests, not emitter or borrowed-storage replay admission. Both the
// semantic model and the immutable native graph pass their real admission APIs.
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const SCALAR_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SLICE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const INNER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const SHARED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const UNIQUE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);
const ARGUMENT: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(1);
const VALUES: [ValueId; 3] = [ValueId(17), ValueId(3), ValueId(11)];
const WORK_PREFIX: usize = 19;

fn declaration(
    ty: SemanticTypeIdV1,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    let tag = u8::try_from(ty.index() + 1).unwrap();
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn reference_type(
    ty: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
    size: u64,
    alignment: u64,
) -> SemanticTypeDeclV1 {
    declaration(
        ty,
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
                pointee,
                SemanticPointerKindV1::Reference,
                mutability,
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
                    match mutability {
                        SemanticMutabilityV1::Immutable => {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        }
                        SemanticMutabilityV1::Mutable => {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        }
                    },
                    size,
                    alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn source_types() -> Vec<SemanticTypeDeclV1> {
    let aggregate_layout = |size, offsets| {
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap()
    };
    vec![
        declaration(
            UNIT,
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
        declaration(
            U32,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        reference_type(SCALAR_REF, U32, SemanticMutabilityV1::Mutable, 4, 4),
        declaration(
            SLICE,
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
        ),
        declaration(
            SLICE_REF,
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
                    SLICE,
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
            INNER,
            aggregate_layout(16, vec![0, 0, 8, 16]),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![UNIT, U32, SCALAR_REF, UNIT]).unwrap(),
            ),
        ),
        declaration(
            ENV,
            aggregate_layout(32, vec![0, 0, 16]),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![UNIT, INNER, SLICE_REF]).unwrap(),
            ),
        ),
        reference_type(SHARED, ENV, SemanticMutabilityV1::Immutable, 32, 8),
        reference_type(UNIQUE, ENV, SemanticMutabilityV1::Mutable, 32, 8),
    ]
}

fn source_abi(
    reference: Option<SemanticTypeIdV1>,
    convention: SemanticCanonAbiV1,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionAbiV1 {
    let mut arguments: Vec<_> = reference
        .into_iter()
        .map(|ty| {
            let shared = ty == SHARED;
            let attributes = SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    shared,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                32,
                Some(8),
            )
            .unwrap();
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(attributes),
            ))
        })
        .collect();
    let source_ownership = if reference.is_none() {
        let first = SemanticAbiValueAttributesV1::new(
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
        let second = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        arguments.push(SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SLICE_REF,
            SemanticAbiPassModeV1::Pair { first, second },
        )));
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    } else {
        ownership
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([if reference.is_some() { 40 } else { 30 }; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        convention,
        match convention {
            SemanticCanonAbiV1::Rust => SemanticExternAbiV1::Rust,
            SemanticCanonAbiV1::C => SemanticExternAbiV1::C { unwind: false },
            SemanticCanonAbiV1::GpuKernel => SemanticExternAbiV1::GpuKernel,
            _ => unreachable!(),
        },
        false,
        false,
        1,
        arguments,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![source_ownership])
    .unwrap()
}

fn source_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}

fn source_function(
    tag: u8,
    role: SemanticFunctionRoleV1,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source,
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(index, &(ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([index as u8 + 1; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn admitted_source(
    reference: SemanticTypeIdV1,
    convention: SemanticCanonAbiV1,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> AdmittedInertSemanticMirV1 {
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, kind| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let unit = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let scalar = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let call_argument = match reference {
        SHARED => SemanticOperandV1::Copy(place(7, SHARED)),
        UNIQUE => SemanticOperandV1::Move(place(6, UNIQUE)),
        _ => panic!("fixture requires an exact shared or unique environment reference"),
    };
    // The slice is a real input; the stored unique reference points to a live
    // initialized scalar. Move the unique carriers into the nested environment.
    let mut initialization = vec![
        assign(2, U32, SemanticRvalueKindV1::Use(scalar(7))),
        assign(
            3,
            SCALAR_REF,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(2, U32),
            },
        ),
        assign(
            4,
            INNER,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        unit(),
                        scalar(11),
                        SemanticOperandV1::Move(place(3, SCALAR_REF)),
                        unit(),
                    ],
                )
                .unwrap(),
            ),
        ),
        assign(
            5,
            ENV,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![
                        unit(),
                        SemanticOperandV1::Move(place(4, INNER)),
                        SemanticOperandV1::Copy(place(1, SLICE_REF)),
                    ],
                )
                .unwrap(),
            ),
        ),
        assign(
            6,
            UNIQUE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(5, ENV),
            },
        ),
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(7)),
        ),
        assign(
            7,
            SHARED,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(6),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV)
                            .unwrap(),
                    ],
                    ENV,
                )
                .unwrap(),
            },
        ),
    ];
    // Both reference types belong to this real reborrow chain. End the shared
    // loan before transferring the unique receiver to the mutable helper.
    if reference == UNIQUE {
        initialization.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(7)),
        ));
    }
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(HELPER.index()),
            vec![call_argument],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let root = source_function(
        30,
        SemanticFunctionRoleV1::KernelRoot,
        source_abi(None, SemanticCanonAbiV1::GpuKernel, ownership),
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (SLICE_REF, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Temporary),
            (SCALAR_REF, SemanticLocalRoleV1::Temporary),
            (INNER, SemanticLocalRoleV1::Temporary),
            (ENV, SemanticLocalRoleV1::Temporary),
            (UNIQUE, SemanticLocalRoleV1::Temporary),
            (SHARED, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            source_block(30, initialization, call),
            source_block(31, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"borrowed_parameter_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([90; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let helper = source_function(
        40,
        SemanticFunctionRoleV1::InternalHelper,
        source_abi(Some(reference), convention, ownership),
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (reference, SemanticLocalRoleV1::Argument(0)),
            (reference, SemanticLocalRoleV1::Temporary),
        ],
        vec![source_block(40, vec![], SemanticTerminatorKindV1::Return)],
    );
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types(),
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
    .admit_current_production(SemanticMirLimitsV1::default())
    .expect("entry fixture must satisfy actual semantic/type/ABI admission")
}

fn parameter_types(reference: SemanticTypeIdV1) -> Vec<Type> {
    let access = if reference == SHARED {
        AccessMode::ReadOnly
    } else {
        assert_eq!(reference, UNIQUE);
        AccessMode::ReadWrite
    };
    vec![
        Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Private, access),
        Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Private, access),
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ]
}

fn verified_graph(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let (graph, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .expect("positive and hostile signatures must remain real, verified KIR");
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    (graph, receipt.retained_storage())
}

struct Fixture {
    source: AdmittedInertSemanticMirV1,
    graph: VerifiedCanonicalKernelIrModuleV12,
    graph_storage: usize,
    instance: SemanticKirFunctionCorrespondenceV1,
    rows: Vec<SemanticKirBorrowedParameterBindingV1>,
}

impl Fixture {
    fn new(reference: SemanticTypeIdV1) -> Self {
        let ownership = if reference == SHARED {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        };
        let source = admitted_source(reference, SemanticCanonAbiV1::Rust, ownership);
        let instance = SemanticKirFunctionCorrespondenceV1 {
            correspondence_owner: ROOT,
            semantic_function: HELPER,
            kernel_ir_function: FunctionId::new("borrowed_parameter_helper"),
            role: SemanticKirFunctionRoleV1::InternalHelper,
        };
        let rows = [
            (
                U32,
                vec![1, 1],
                SemanticKirBorrowedParameterTransportV1::ScalarAddress,
            ),
            (
                SCALAR_REF,
                vec![1, 2],
                SemanticKirBorrowedParameterTransportV1::ReferenceValue,
            ),
            (
                SLICE_REF,
                vec![2],
                SemanticKirBorrowedParameterTransportV1::SliceValue,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(slot, (semantic_component_type, projection, transport))| {
            SemanticKirBorrowedParameterBindingV1 {
                correspondence_owner: ROOT,
                semantic_function: HELPER,
                semantic_local: ARGUMENT,
                reference_type: reference,
                semantic_component_type,
                projection: projection.into_boxed_slice(),
                transport,
                kernel_ir_value: VALUES[slot],
            }
        })
        .collect();
        let mut root = BasicBlock::new(BlockId(0));
        root.terminator = Some(Terminator::Return { values: vec![] });
        let mut helper = root.clone();
        helper.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(99), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(Constant::U32(0)),
        ));
        let mut module = Module::new("borrowed-parameter-view");
        module.functions = vec![
            Function::kernel_entry(
                "borrowed_parameter_root",
                Signature::new(vec![], vec![]),
                vec![],
                vec![root],
            ),
            Function::internal_helper(
                "borrowed_parameter_helper",
                Signature::new(parameter_types(reference), vec![]),
                VALUES.to_vec(),
                vec![helper],
            ),
        ];
        module.kernels.push(Kernel::new(
            "borrowed_parameter_root",
            "borrowed_parameter_root",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        let (graph, graph_storage) = verified_graph(&module);
        Self {
            source,
            graph,
            graph_storage,
            instance,
            rows,
        }
    }

    fn floor(&self) -> usize {
        self.graph_storage + 23
    }

    fn with_view<'w, R>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        visit: impl for<'s> FnOnce(
            &mut ProductionArgumentViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_parameter_correspondence_v1(
            &self.source,
            &self.instance,
            self.graph
                .module()
                .function(&self.instance.kernel_ir_function)
                .unwrap(),
            ArgumentTraceV1 {
                direct: &[],
                components: &[],
                borrowed: &self.rows,
                ignored: &[],
            },
            budget,
            visit,
        )
    }

    fn reject_rows(&self, description: &str) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(self.floor()).unwrap();
        let mut entered = false;
        let result = self.with_view(&mut budget, |_| {
            entered = true;
            Ok(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "{description}: {result:?}"
        );
        assert!(
            !entered,
            "{description}: rejected rows must never expose a view"
        );
        assert_eq!(budget.storage(), self.floor(), "{description}");
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Coverage {
    Zero,
    Components(usize, usize),
    Parameter(usize),
}

fn nodes(
    view: &mut ProductionArgumentViewV1<'_, '_>,
) -> Result<
    Vec<(
        SemanticTypeIdV1,
        Vec<ProductionArgumentProjectionV1>,
        Coverage,
    )>,
    ProductionSemanticKirErrorV1,
> {
    let mut result = Vec::new();
    view.visit_nodes(|node| {
        assert_eq!(node.source_argument(), 0);
        assert_eq!(node.adjusted_argument(), Some(0));
        assert_eq!(node.local_binding(), Some((ARGUMENT, node.source_path())));
        assert!(node.ignored_local_binding().is_none());
        let coverage = match node.coverage() {
            ProductionArgumentCoverageV1::Zero => Coverage::Zero,
            ProductionArgumentCoverageV1::Components { first, end } => {
                Coverage::Components(first, end)
            }
            ProductionArgumentCoverageV1::Parameter(parameter) => {
                assert_eq!(parameter.value(), VALUES[parameter.slot()]);
                let ProductionArgumentTraceV1::Borrowed(row) = parameter.trace() else {
                    panic!("borrowed fields must not masquerade as direct/by-value rows");
                };
                assert_eq!(row.kernel_ir_value(), parameter.value());
                Coverage::Parameter(parameter.slot())
            }
            ProductionArgumentCoverageV1::WithinAtomicParameter(_) => {
                panic!("borrowed referents are not one atomic physical parameter")
            }
        };
        result.push((node.semantic_type(), node.source_path().to_vec(), coverage));
        Ok(())
    })?;
    Ok(result)
}

#[test]
fn borrowed_view_preserves_exact_reference_abi_transports_and_nested_unit_paths() {
    use ProductionArgumentProjectionV1::{Dereference as D, Field as F};
    for reference in [SHARED, UNIQUE] {
        let mut fixture = Fixture::new(reference);
        // Trace order is not parameter order, and neither is native ValueId order.
        fixture.rows.rotate_left(1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(fixture.floor()).unwrap();
        fixture
            .with_view(&mut budget, |view| {
                assert_eq!(view.association(), &fixture.instance);
                let sources: Vec<_> = view.source_arguments()?.collect();
                let adjusted: Vec<_> = view.adjusted_arguments()?.collect();
                assert_eq!(sources.len(), 1);
                assert_eq!(sources[0].ty(), reference);
                assert_eq!(adjusted.len(), 1);
                let mapped = adjusted[0];
                assert_eq!(mapped.local(), ARGUMENT);
                assert_eq!(mapped.abi().ty(), reference);
                assert!(mapped.abi().value().adjusted().is_none());
                assert!(mapped.abi().value().pointee_override().is_none());
                assert!(matches!(
                    mapped.abi().mode(),
                    SemanticAbiPassModeV1::Direct(_)
                ));
                assert_eq!(mapped.tuple_field(), None);
                assert_eq!(mapped.local_field(), None);
                assert_eq!(
                    mapped.source_ownership(),
                    if reference == SHARED {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    } else {
                        SemanticSourceArgumentOwnershipV1::UniqueBorrow
                    }
                );
                for (slot, ty) in parameter_types(reference).iter().enumerate() {
                    let physical = view.physical(slot)?.unwrap();
                    assert_eq!(physical.slot(), slot);
                    assert_eq!(physical.value(), VALUES[slot]);
                    assert_eq!(physical.ty(), ty);
                    let ProductionArgumentTraceV1::Borrowed(row) = physical.trace() else {
                        panic!("expected the exact borrowed row");
                    };
                    assert_eq!(
                        row,
                        fixture
                            .rows
                            .iter()
                            .find(|row| row.kernel_ir_value() == VALUES[slot])
                            .unwrap()
                    );
                    assert_eq!(row.correspondence_owner(), ROOT);
                    assert_eq!(row.semantic_function(), HELPER);
                    assert_eq!(row.semantic_local(), ARGUMENT);
                    assert_eq!(row.reference_type(), reference);
                    assert_eq!(
                        row.semantic_component_type(),
                        [U32, SCALAR_REF, SLICE_REF][slot]
                    );
                    assert_eq!(row.projection(), [&[1, 1][..], &[1, 2], &[2]][slot]);
                    assert_eq!(
                        row.transport(),
                        [
                            SemanticKirBorrowedParameterTransportV1::ScalarAddress,
                            SemanticKirBorrowedParameterTransportV1::ReferenceValue,
                            SemanticKirBorrowedParameterTransportV1::SliceValue,
                        ][slot]
                    );
                }
                assert!(view.physical(3)?.is_none());
                assert!(view.ignored_local(ARGUMENT)?.is_none());
                assert_eq!(
                    nodes(view)?,
                    vec![
                        (UNIT, vec![D, F(0)], Coverage::Zero),
                        (UNIT, vec![D, F(1), F(0)], Coverage::Zero),
                        (U32, vec![D, F(1), F(1)], Coverage::Parameter(0)),
                        (SCALAR_REF, vec![D, F(1), F(2)], Coverage::Parameter(1)),
                        (UNIT, vec![D, F(1), F(3)], Coverage::Zero),
                        (INNER, vec![D, F(1)], Coverage::Components(0, 2)),
                        (SLICE_REF, vec![D, F(2)], Coverage::Parameter(2)),
                        (ENV, vec![D], Coverage::Components(0, 3)),
                        (reference, vec![], Coverage::Components(0, 3)),
                    ]
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), fixture.floor());
    }
}

#[test]
fn borrowed_view_rejects_each_changed_row_identity_and_transport() {
    type Mutation = fn(&mut Vec<SemanticKirBorrowedParameterBindingV1>);
    let mutations: &[(&str, Mutation)] = &[
        ("owner", |rows| rows[0].correspondence_owner = HELPER),
        ("function", |rows| rows[0].semantic_function = ROOT),
        ("non-argument local", |rows| {
            rows[0].semantic_local = SemanticLocalIdV1::from_index(2)
        }),
        ("missing local", |rows| {
            rows[0].semantic_local = SemanticLocalIdV1::from_index(99)
        }),
        ("reference type", |rows| rows[0].reference_type = UNIQUE),
        ("component type", |rows| {
            rows[0].semantic_component_type = SCALAR_REF
        }),
        ("field", |rows| {
            rows[0].projection = vec![1, 2].into_boxed_slice()
        }),
        ("Unit field", |rows| {
            rows[0].projection = vec![1, 0].into_boxed_slice()
        }),
        ("empty field path", |rows| {
            rows[0].projection = vec![].into_boxed_slice()
        }),
        ("field byte offset", |rows| {
            rows[1].projection = vec![1, 8].into_boxed_slice()
        }),
        ("scalar address as reference", |rows| {
            rows[0].transport = SemanticKirBorrowedParameterTransportV1::ReferenceValue
        }),
        ("stored reference as address", |rows| {
            rows[1].transport = SemanticKirBorrowedParameterTransportV1::ScalarAddress
        }),
        ("slice as address", |rows| {
            rows[2].transport = SemanticKirBorrowedParameterTransportV1::ScalarAddress
        }),
        ("slice as reference", |rows| {
            rows[2].transport = SemanticKirBorrowedParameterTransportV1::ReferenceValue
        }),
        ("defined non-parameter value", |rows| {
            rows[0].kernel_ir_value = ValueId(99)
        }),
        ("absent native value", |rows| {
            rows[0].kernel_ir_value = ValueId(100)
        }),
        ("same-typed parameter swap", |rows| {
            rows[0].kernel_ir_value = VALUES[1];
            rows[1].kernel_ir_value = VALUES[0];
        }),
        ("missing row", |rows| {
            rows.pop();
        }),
        ("extra duplicate row", |rows| rows.push(rows[0].clone())),
        ("duplicate replacing a row", |rows| {
            rows[1] = rows[0].clone()
        }),
    ];
    let mut fixture = Fixture::new(SHARED);
    let original = fixture.rows.clone();
    for &(name, mutate) in mutations {
        fixture.rows.clone_from(&original);
        mutate(&mut fixture.rows);
        fixture.reject_rows(name);
    }
}

#[test]
fn borrowed_view_rejects_verified_wrong_signatures_including_ro_to_rw() {
    let scalar = Type::Scalar(ScalarType::U32);
    let cases = [
        ("address became scalar", 0, scalar.clone()),
        (
            "wrong pointee",
            0,
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
        ),
        (
            "wrong address space",
            1,
            Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly),
        ),
        (
            "scalar RO to RW",
            0,
            Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        (
            "stored reference RO to RW",
            1,
            Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        (
            "slice RO to RW",
            2,
            Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
        ),
        (
            "whole slice became pointer",
            2,
            Type::pointer(scalar, AddressSpace::Global, AccessMode::ReadOnly),
        ),
    ];
    let mut fixture = Fixture::new(SHARED);
    let original = fixture.graph.module().clone();
    for (name, slot, ty) in cases {
        let mut module = original.clone();
        module.functions[1].signature.parameters[slot] = ty;
        (fixture.graph, fixture.graph_storage) = verified_graph(&module);
        fixture.reject_rows(name);
    }
}

#[test]
fn borrowed_view_requires_rust_reference_ownership_not_just_pointer_types() {
    for reference in [SHARED, UNIQUE] {
        let mut fixture = Fixture::new(reference);
        let correct = if reference == SHARED {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        };
        let opposite = if reference == SHARED {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        };
        for (convention, ownership) in [
            (SemanticCanonAbiV1::C, correct),
            (
                SemanticCanonAbiV1::Rust,
                SemanticSourceArgumentOwnershipV1::Unspecified,
            ),
            (
                SemanticCanonAbiV1::Rust,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ),
            (SemanticCanonAbiV1::Rust, opposite),
        ] {
            fixture.source = admitted_source(reference, convention, ownership);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(fixture.floor()).unwrap();
            let mut entered = false;
            let result = fixture.with_view(&mut budget, |_| {
                entered = true;
                Ok(())
            });
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "{convention:?}/{ownership:?}: {result:?}"
            );
            assert!(!entered);
            assert_eq!(budget.storage(), fixture.floor());
        }
    }
}

fn visit_twice(
    view: &mut ProductionArgumentViewV1<'_, '_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let retained = view.budget.storage();
    let before = view.budget.work();
    let first = nodes(view)?;
    let after = view.budget.work();
    let peak = view.budget.peak_storage();
    assert_eq!(view.budget.storage(), retained);
    assert_eq!(nodes(view)?, first);
    assert!(after > before);
    assert_eq!(view.budget.work() - after, after - before);
    assert_eq!(view.budget.peak_storage(), peak);
    assert_eq!(view.budget.storage(), retained);
    Ok(())
}

#[test]
fn borrowed_view_exact_and_one_short_cumulative_budgets_restore_live_floor() {
    let fixture = Fixture::new(SHARED);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(fixture.floor()).unwrap();
    budget.charge_work(WORK_PREFIX).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    fixture.with_view(&mut budget, visit_twice).unwrap();
    let first_work = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), fixture.floor());
    fixture.with_view(&mut budget, visit_twice).unwrap();
    let exact_work = budget.work();
    assert_eq!(exact_work - first_work, first_work - WORK_PREFIX);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.storage(), fixture.floor());

    for (work_limit, storage_limit, failure) in [
        (exact_work, peak, None),
        (exact_work - 1, peak, Some("work")),
        (exact_work, peak - 1, Some("storage")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(fixture.floor()).unwrap();
        budget.charge_work(WORK_PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut entered = 0;
        let mut result = Ok(());
        for _ in 0..2 {
            result = fixture.with_view(&mut budget, |view| {
                entered += 1;
                visit_twice(view)
            });
            assert_eq!(budget.storage(), fixture.floor());
            if result.is_err() {
                break;
            }
        }
        match failure {
            None => {
                result.unwrap();
                assert_eq!(entered, 2);
                assert_eq!(budget.work(), exact_work);
                assert_eq!(budget.peak_storage(), peak);
            }
            Some("work") => {
                assert_eq!(entered, 2, "exhaustion must happen during the second view");
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            }
            Some("storage") => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
            }
            _ => unreachable!(),
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), fixture.floor());
    }
}

#[test]
fn borrowed_view_visitor_and_live_storage_failures_restore_nested_and_outer_floors() {
    let fixture = Fixture::new(SHARED);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut baseline = ArgumentBudgetV1::new(&mut work, usize::MAX);
    baseline.reserve_storage(fixture.floor()).unwrap();
    fixture.with_view(&mut baseline, visit_twice).unwrap();
    let peak = baseline.peak_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, peak);
    budget.reserve_storage(fixture.floor()).unwrap();
    let result: Result<(), _> = fixture.with_view(&mut budget, |view| {
        let retained = view.budget.storage();
        let work_before = view.budget.work();
        let mut visited = 0;
        let error = view.visit_nodes(|_| {
            visited += 1;
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        });
        assert!(matches!(
            error,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(visited, 1);
        assert_eq!(view.budget.storage(), retained);
        assert!(view.budget.work() > work_before);
        let bytes = peak - retained;
        view.budget.reserve_storage(bytes)?;
        let payload = vec![0_u8; bytes];
        let error = view.visit_nodes(|_| panic!("no scratch space remains for traversal"));
        let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Storage(error),
        )) = error
        else {
            panic!("live consumer payload must exhaust traversal storage");
        };
        assert_eq!(error.limit(), peak);
        assert!(error.actual() > peak);
        assert_eq!(view.budget.failed_storage(), Some(error.actual()));
        assert_eq!(view.budget.storage(), peak);
        assert_eq!(view.budget.peak_storage(), peak);
        drop(payload);
        view.budget.release_storage(bytes)?;
        visit_twice(view)?;
        assert_eq!(view.budget.storage(), retained);
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), fixture.floor());
}
