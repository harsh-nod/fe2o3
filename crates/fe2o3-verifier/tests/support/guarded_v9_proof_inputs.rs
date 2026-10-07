//! Synthetic source identities and signing data; real ranked checks, lowering and formal admission.

use super::compiler_proof_inputs_v3::{
    CanonicalCompilerProofInputsV3, canonical_compiler_proof_inputs_from_owner,
    production_target_layout_identity,
};
use dialect_kernel::AccessKindAttr;
use fe2o3_kernel_ir::OperationKind;
use fe2o3_lower_mir_kernel::{
    ProductionRankedAccessSourceV1, ProductionRankedSemanticProjectionReceiptV1,
    ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1 as ROp, ProductionRankedTerminatorV1 as RTerm,
    ProductionRankedValueIdV1 as RId, ProductionRankedValueV1 as RValue,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v1,
};

const UNIT: u32 = 0;
const U32: u32 = 1;
const BOOL: u32 = 2;
const U64: u32 = 3;
const MARKER: u32 = 4;
const RAW_POINTER: u32 = 5;
const WITNESS: u32 = 6;
const OUTPUT: u32 = 7;
const OUTPUT_REF: u32 = 8;

fn id(seed: u8, tag: u8) -> [u8; 32] {
    let mut result = [tag; 32];
    result[0] = seed;
    result
}
fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn attributes(non_null: bool, boolean: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, non_null, false, false, true),
        if boolean {
            SemanticAbiExtensionV1::ZeroExtend
        } else {
            SemanticAbiExtensionV1::None
        },
        0,
        None,
    )
    .unwrap()
}
fn value(kind: u32) -> SemanticAbiValueV1 {
    let mode = match kind {
        UNIT => SemanticAbiPassModeV1::Ignore,
        OUTPUT => SemanticAbiPassModeV1::Pair {
            first: attributes(false, false),
            second: attributes(false, false),
        },
        OUTPUT_REF => SemanticAbiPassModeV1::Direct(attributes(true, false)),
        _ => SemanticAbiPassModeV1::Direct(attributes(false, kind == BOOL)),
    };
    SemanticAbiValueV1::new(ty(kind), mode)
}
fn abi(seed: u8, tag: u8, kernel: bool, inputs: &[u32], output: u32) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(id(seed, tag)),
        production_target_layout_identity(),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        inputs.len() as u32,
        inputs
            .iter()
            .map(|kind| SemanticAbiArgumentV1::source(value(*kind)))
            .collect(),
        value(output),
    )
    .unwrap()
}
fn declaration(
    seed: u8,
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(id(seed, tag)),
        SemanticLayoutIdentityV1::from_sha256(id(seed, tag)),
        layout,
        shape,
    )
}
fn scalar_backend(bits: u16, bytes: u64, maximum: u128) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, bits, bytes),
        SemanticScalarValidityRangeV1::new(0, maximum),
    )
}
fn aggregate_layout(
    bytes: u64,
    backend: SemanticBackendReprV1,
    offsets: Vec<u64>,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::aggregate_with_backend_repr(
        Some(bytes),
        8,
        backend,
        false,
        SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
    )
    .unwrap()
}
fn types(seed: u8) -> Vec<SemanticTypeDeclV1> {
    let u64_backend = scalar_backend(64, 8, u64::MAX.into());
    let raw_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let reference_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let scalar = |tag, bits, bytes, maximum, shape| {
        declaration(
            seed,
            tag,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(scalar_backend(bits, bytes, maximum)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        )
    };
    let aggregate =
        |fields| SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap());
    vec![
        declaration(
            seed,
            10,
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
        scalar(
            11,
            32,
            4,
            u32::MAX.into(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar(12, 8, 1, 1, SemanticScalarTypeV1::Bool),
        scalar(
            13,
            64,
            8,
            u64::MAX.into(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        declaration(
            seed,
            14,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(0),
                1,
                SemanticBackendReprV1::memory(true),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Opaque,
        ),
        declaration(
            seed,
            15,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(raw_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(U32),
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
            seed,
            16,
            aggregate_layout(8, SemanticBackendReprV1::scalar(u64_backend), vec![0, 8]),
            aggregate(vec![ty(U64), ty(MARKER)]),
        ),
        declaration(
            seed,
            17,
            aggregate_layout(
                16,
                SemanticBackendReprV1::ScalarPair {
                    first: raw_backend,
                    second: u64_backend,
                },
                vec![0, 8, 16],
            ),
            aggregate(vec![ty(RAW_POINTER), ty(U64), ty(MARKER)]),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
        declaration(
            seed,
            18,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(reference_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(OUTPUT),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
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
                        SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                        0,
                        1,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    ]
}
fn callable(
    seed: u8,
    tag: u8,
    abi: SemanticFunctionAbiV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(id(seed, tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(id(seed, tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(id(seed, tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(id(seed, tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(id(seed, tag)),
            source(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(id(seed, tag)),
    }
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    local: u32,
    kind: u32,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(local, kind),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn block(
    seed: u8,
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(id(seed, tag)),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}

pub(super) fn inputs(seed: u8) -> CanonicalCompilerProofInputsV3 {
    let borrow = SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(3, OUTPUT_REF),
            SemanticRvalueV1::new(
                ty(OUTPUT_REF),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1, OUTPUT),
                },
            ),
        )),
    );
    let constant = SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(U32),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
    ));
    let blocks = vec![
        block(seed, 30, vec![], call(1, vec![], 2, WITNESS, 1)),
        block(
            seed,
            31,
            vec![borrow],
            call(
                2,
                vec![
                    SemanticOperandV1::Move(place(3, OUTPUT_REF)),
                    SemanticOperandV1::Move(place(2, WITNESS)),
                    constant,
                ],
                4,
                BOOL,
                2,
            ),
        ),
        block(seed, 32, vec![], SemanticTerminatorKindV1::Return),
    ];
    let locals = [UNIT, OUTPUT, WITNESS, OUTPUT_REF, BOOL]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(id(seed, 40 + index as u8)),
                ty(kind),
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source(),
            )
        })
        .collect();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(id(seed, 50)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(id(seed, 50)),
        SemanticMonomorphizationIdentityV1::from_sha256(id(seed, 50)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(id(seed, 50)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(id(seed, 50)),
        source(),
        abi(seed, 51, true, &[OUTPUT], UNIT)
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
            .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"v9_write_only".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(id(seed, 52)),
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
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        callable(
            seed,
            53,
            abi(seed, 53, false, &[], WITNESS),
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: ty(WITNESS),
                raw_index: ty(U64),
            },
        ),
        callable(
            seed,
            54,
            abi(seed, 54, false, &[OUTPUT_REF, WITNESS, U32], BOOL),
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                disjoint_slice: ty(OUTPUT),
                witness: ty(WITNESS),
                element: ty(U32),
                raw_index: ty(U64),
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
            },
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(production_target_layout_identity()),
        types(seed),
        vec![],
        vec![],
        vec![],
        vec![root],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let local = |n| RValue::Local(RId::new(n));
    let recipe = ProductionRankedKernelV1::new(
        "v9_write_only",
        1,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ROp::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [0, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: false,
                    },
                    ROp::View {
                        result: RId::new(0),
                        element_width: 32,
                        writable: true,
                        shape: vec![0],
                        dynamic_extents: vec![RValue::Argument(0)],
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ROp::InvocationIndex {
                        result: RId::new(1),
                        dimension: 0,
                        launch_extent: 0,
                    },
                ],
                RTerm::IndexLessThan {
                    lhs: local(1),
                    rhs: RValue::Argument(0),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![ROp::Access {
                    kind: AccessKindAttr::Write,
                    view: local(0),
                    indices: vec![local(1)],
                }],
                RTerm::Return,
            ),
            ProductionRankedBlockV1::new(vec![], RTerm::Return),
        ],
    )
    .unwrap();
    let diagnostic = format!("{recipe:#?}");
    let ranked = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("v9_write_only_module", recipe).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let receipt =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            semantic,
            ranked,
            diagnostic,
            vec![ProductionRankedAccessSourceV1::new(1, None, 0, 1, 0)],
        )
        .unwrap();
    let owner = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
        1,
    )
    .unwrap();
    owner.verify_equivalence().unwrap();
    assert!(owner.canonical_kernel_ir_v9().is_some());
    assert!(owner.canonical_kernel_ir_v8().is_none());
    assert!(owner.retains_mandatory_generic_checks());
    assert_eq!(
        owner
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|op| matches!(op.kind, OperationKind::GuardedStore { .. }))
            .count(),
        1
    );
    let (inputs, formal) = canonical_compiler_proof_inputs_from_owner(seed, owner, true);
    formal.verify_equivalence().unwrap();
    inputs
}
