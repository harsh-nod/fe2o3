use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_compiler_lineage::derive_semantic_target_layout_identity_v1;
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV5;
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalFormalMemoryAdmissionEvidenceV4,
    InertCanonicalMirToKirCorrespondenceEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV4,
    ProductionFormalMemoryOwnerV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    InertProductionMiddleEndEvidenceV5, PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5,
    PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5, ProductionSemanticMirLimitsV1,
    ProductionSemanticMirOwnerV1,
};
use fe2o3_proof_contracts::DigestV1;
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1, ProductionMirPlironVerusExecutionClaimsV1,
};
use sha2::{Digest, Sha256};

const PRODUCTION_RUSTC_LLVM_TARGET: &str = "amdgcn-amd-amdhsa";
const PRODUCTION_AMDHSA_DATA_LAYOUT: &str = "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";
const PRODUCTION_TARGET_CPU: &str = "gfx942";
const PRODUCTION_TARGET_FEATURES: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";

fn production_target_layout_identity() -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(
        derive_semantic_target_layout_identity_v1(
            PRODUCTION_RUSTC_LLVM_TARGET,
            PRODUCTION_AMDHSA_DATA_LAYOUT,
            64,
            PRODUCTION_TARGET_CPU,
            PRODUCTION_TARGET_FEATURES,
        )
        .unwrap()
        .sha256(),
    )
}

pub(crate) struct CanonicalCompilerProofInputsV3 {
    semantic_mir: Vec<u8>,
    middle_end: Vec<u8>,
    kernel_ir: Vec<u8>,
    correspondence: Vec<u8>,
    formal_memory: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSourceIsaKernelFamilyV1 {
    Elementwise,
    WorkgroupCollective,
    Tiled,
}

impl CanonicalCompilerProofInputsV3 {
    pub(crate) fn semantic_mir(&self) -> &[u8] {
        &self.semantic_mir
    }

    pub(crate) fn middle_end(&self) -> &[u8] {
        &self.middle_end
    }

    pub(crate) fn kernel_ir(&self) -> &[u8] {
        &self.kernel_ir
    }

    pub(crate) fn correspondence(&self) -> &[u8] {
        &self.correspondence
    }

    pub(crate) fn formal_memory(&self) -> &[u8] {
        &self.formal_memory
    }
}

#[allow(
    dead_code,
    reason = "shared support is compiled by V4-only integration tests"
)]
pub(crate) fn canonical_compiler_proof_inputs_v3(seed: u8) -> CanonicalCompilerProofInputsV3 {
    canonical_compiler_proof_inputs(seed, semantic_owner(seed), false)
}

#[allow(
    dead_code,
    reason = "shared support is compiled by V3-only integration tests"
)]
pub(crate) fn canonical_compiler_proof_inputs_v4(seed: u8) -> CanonicalCompilerProofInputsV3 {
    canonical_compiler_proof_inputs(seed, semantic_owner(seed), true)
}

#[allow(
    dead_code,
    reason = "shared support is compiled by V3-only integration tests"
)]
pub(crate) fn canonical_compiler_proof_inputs_v4_with_induction(
    seed: u8,
) -> CanonicalCompilerProofInputsV3 {
    canonical_compiler_proof_inputs(
        seed,
        semantic_induction_owner(seed, false, ProductionSourceIsaKernelFamilyV1::Elementwise),
        true,
    )
}

#[allow(dead_code, reason = "shared sourceful V4 finalizer fixture")]
pub(crate) fn canonical_compiler_proof_inputs_v4_with_sourceful_induction(
    seed: u8,
) -> CanonicalCompilerProofInputsV3 {
    canonical_compiler_proof_inputs_v4_with_sourceful_family(
        seed,
        ProductionSourceIsaKernelFamilyV1::Elementwise,
    )
}

#[allow(
    dead_code,
    reason = "shared source/ISA kernel-family acceptance fixture"
)]
pub(crate) fn canonical_compiler_proof_inputs_v4_with_sourceful_family(
    seed: u8,
    family: ProductionSourceIsaKernelFamilyV1,
) -> CanonicalCompilerProofInputsV3 {
    canonical_compiler_proof_inputs(seed, semantic_induction_owner(seed, true, family), true)
}

fn canonical_compiler_proof_inputs(
    seed: u8,
    semantic_owner: ProductionSemanticMirOwnerV1,
    lossless_correspondence: bool,
) -> CanonicalCompilerProofInputsV3 {
    let semantic_kir = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let semantic = semantic_kir.semantic().semantic();
    let semantic_mir = semantic.canonical_encoding().to_vec();
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let (kernel_ir, correspondence) = if lossless_correspondence {
        let report = analyze_semantic_u32_induction_no_overflow_v1(
            semantic,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let correspondence =
            InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(&semantic_kir, &report)
                .unwrap()
                .canonical_bytes()
                .to_vec();
        (
            semantic_kir
                .canonical_kernel_ir_v8()
                .canonical_bytes()
                .to_vec(),
            correspondence,
        )
    } else {
        let kernel_ir =
            VerifiedCanonicalKernelIrV5::from_module(semantic_kir.module().clone()).unwrap();
        let correspondence =
            InertCanonicalMirToKirCorrespondenceEvidenceV3::from_live_owner(&semantic_kir)
                .unwrap()
                .canonical_bytes()
                .to_vec();
        (kernel_ir.into_canonical_bytes(), correspondence)
    };
    let formal_owner = ProductionFormalMemoryOwnerV1::try_admit(semantic_kir).unwrap();
    let formal_memory = if lossless_correspondence {
        InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&formal_owner)
            .unwrap()
            .into_canonical_bytes()
    } else {
        InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(&formal_owner)
            .unwrap()
            .into_canonical_bytes()
    };

    CanonicalCompilerProofInputsV3 {
        semantic_mir,
        middle_end: middle_end_v5_bytes(semantic_identity, seed),
        kernel_ir,
        correspondence,
        formal_memory,
    }
}

/// Builds internally valid signed aggregate evidence for cross-crate transport tests.
///
/// The fixture key is public test data and grants no compiler or runtime authority.
#[allow(
    dead_code,
    reason = "shared support is compiled by proof-input tests that do not all construct V4 evidence"
)]
pub(crate) fn canonical_verus_execution_evidence_v1(middle_end: &[u8], seed: u8) -> Vec<u8> {
    let middle_end = InertProductionMiddleEndEvidenceV5::decode(middle_end).unwrap();
    let binding = FunctionalRefinementBindingV2::new(
        SafeReferenceKindV2::SourceAndMir,
        proof_digest(10, seed),
        proof_digest(11, seed),
        proof_digest(12, seed),
        proof_digest(13, seed),
        proof_digest(14, seed),
        proof_digest(15, seed),
    )
    .unwrap();
    let toolchain = VerusToolchainIdentityV2::new(
        proof_digest(20, seed),
        proof_digest(21, seed),
        proof_digest(22, seed),
        proof_digest(23, seed),
        proof_digest(24, seed),
    )
    .unwrap();
    let signing = SigningKey::from_bytes(&[42_u8.wrapping_add(seed); 32]);
    let verifying_key = signing.verifying_key().to_bytes();
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying_key,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        proof_digest(30, seed),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
        proof_digest(1, seed),
        proof_digest(2, seed),
        DigestV1::from_untrusted_bytes(*middle_end.identity().sha256()),
        proof_digest(4, seed),
        proof_digest(5, seed),
        binding,
        imported.signer_identity(),
        toolchain,
        imported.execution_identity(),
        imported.receipt_identity().digest(),
        1,
    )
    .unwrap();
    CanonicalProductionMirPlironVerusExecutionEvidenceV1::new(claims, verifying_key, wire)
        .unwrap()
        .canonical_bytes()
        .to_vec()
}

#[allow(
    dead_code,
    reason = "used only by the optional shared V4 evidence constructor"
)]
fn proof_digest(tag: u8, seed: u8) -> DigestV1 {
    let value = tag.wrapping_add(seed);
    DigestV1::from_untrusted_bytes([if value == 0 { 1 } else { value }; 32])
}

fn bytes(tag: u8, seed: u8) -> [u8; 32] {
    [tag.wrapping_add(seed); 32]
}

fn unit_type(seed: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4, seed)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4, seed)),
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
    )
}

fn block(
    tag: u8,
    seed: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn semantic_owner(seed: u8) -> ProductionSemanticMirOwnerV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let layout = production_target_layout_identity();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2, seed)),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Nop,
    );
    let block0 = block(10, seed, vec![statement], SemanticTerminatorKindV1::Return);
    let block1 = block(
        11,
        seed,
        vec![],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(0),
        )),
    );
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2, seed)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3, seed)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(1),
        vec![block0, block1],
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(format!("proof_input_test_{seed}").into_bytes()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5, seed)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        vec![unit_type(seed)],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[allow(
    dead_code,
    clippy::too_many_lines,
    reason = "used only by the shared checked-induction V4 fixture"
)]
fn semantic_induction_owner(
    seed: u8,
    sourceful: bool,
    family: ProductionSourceIsaKernelFamilyV1,
) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let checked_u32 = SemanticTypeIdV1::from_index(3);
    let induction = SemanticLocalIdV1::from_index(1);
    let bound = SemanticLocalIdV1::from_index(2);
    let predicate = SemanticLocalIdV1::from_index(3);
    let checked_result = SemanticLocalIdV1::from_index(4);

    let scalar_type = |tag, size, shape, primitive, maximum| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag, seed)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag, seed)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    primitive,
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        )
    };
    let u32_type = scalar_type(
        101,
        4,
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        },
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        u128::from(u32::MAX),
    );
    let bool_type = scalar_type(
        102,
        1,
        SemanticScalarTypeV1::Bool,
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        1,
    );
    let checked_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(103, seed)),
        SemanticLayoutIdentityV1::from_sha256(bytes(103, seed)),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![u32_ty, bool_ty]).unwrap()),
    );
    let place = |local, ty| SemanticPlaceV1::new(local, vec![], ty).unwrap();
    let field = |local, index, ty| {
        SemanticPlaceV1::new(
            local,
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let copy = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let constant = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            u32_ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let source = |tag: u8| {
        if !sourceful {
            return SemanticSourceProvenanceV1::unavailable();
        }
        let byte_start = u64::from(tag) * 8;
        let byte_end = if tag == 3 { byte_start } else { byte_start + 4 };
        let origin = SemanticSourceOriginV1::new(
            SemanticSourceFileIdentityV1::from_sha256(bytes(200, seed)),
            byte_start,
            byte_end,
            u32::from(tag) + 1,
            1,
            u32::from(tag) + 1,
            5,
        )
        .unwrap();
        SemanticSourceProvenanceV1::new(None, Some(origin))
    };
    let assign = |tag, destination, ty, value| {
        SemanticStatementV1::new(
            source(tag),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let semantic_block = |tag, statements, terminator| block(tag, seed, statements, terminator);

    let initialization = assign(
        1,
        place(induction, u32_ty),
        u32_ty,
        SemanticRvalueKindV1::Use(constant(0)),
    );
    let guard = assign(
        2,
        place(predicate, bool_ty),
        bool_ty,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy(induction, u32_ty),
            right: copy(bound, u32_ty),
        },
    );
    let checked_add = assign(
        4,
        place(checked_result, checked_u32),
        checked_u32,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Add,
            copy(induction, u32_ty),
            constant(1),
        )),
    );
    let update = assign(
        5,
        place(induction, u32_ty),
        u32_ty,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(checked_result, 0, u32_ty))),
    );
    let (exit_statements, exit_terminator, extra_blocks, extra_locals, callables) = match family {
        ProductionSourceIsaKernelFamilyV1::Elementwise => (
            Vec::new(),
            SemanticTerminatorKindV1::Return,
            Vec::new(),
            Vec::new(),
            vec![SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(0),
            )],
        ),
        ProductionSourceIsaKernelFamilyV1::WorkgroupCollective => {
            let barrier_result = SemanticLocalIdV1::from_index(5);
            let return_edge = SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(5),
            );
            let barrier = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                Vec::new(),
                Some(SemanticCallDestinationV1::new(
                    place(barrier_result, unit),
                    return_edge,
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            let callable_abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256(bytes(140, seed)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250, seed)),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                Vec::new(),
                SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            let callable = SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256(bytes(140, seed)),
                    SemanticItemDefinitionIdentityV1::from_sha256(bytes(141, seed)),
                    SemanticMonomorphizationIdentityV1::from_sha256(bytes(142, seed)),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(143, seed)),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(144, seed)),
                    source(6),
                    callable_abi,
                ),
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(
                    145, seed,
                )),
            };
            (
                Vec::new(),
                SemanticTerminatorKindV1::Call(barrier),
                vec![semantic_block(
                    115,
                    Vec::new(),
                    SemanticTerminatorKindV1::Return,
                )],
                vec![SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256(bytes(146, seed)),
                    unit,
                    SemanticLocalRoleV1::Temporary,
                    source(6),
                )],
                vec![
                    SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                    callable,
                ],
            )
        }
        ProductionSourceIsaKernelFamilyV1::Tiled => {
            let tile_row = SemanticLocalIdV1::from_index(5);
            let tile_column = SemanticLocalIdV1::from_index(6);
            (
                vec![
                    assign(
                        6,
                        place(tile_row, u32_ty),
                        u32_ty,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Divide,
                            left: copy(induction, u32_ty),
                            right: constant(16),
                        },
                    ),
                    assign(
                        7,
                        place(tile_column, u32_ty),
                        u32_ty,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Remainder,
                            left: copy(induction, u32_ty),
                            right: constant(16),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Return,
                Vec::new(),
                vec![
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256(bytes(147, seed)),
                        u32_ty,
                        SemanticLocalRoleV1::Temporary,
                        source(6),
                    ),
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256(bytes(148, seed)),
                        u32_ty,
                        SemanticLocalRoleV1::Temporary,
                        source(7),
                    ),
                ],
                vec![SemanticCallableDeclV1::defined(
                    SemanticFunctionIdV1::from_index(0),
                )],
            )
        }
    };
    let mut blocks = vec![
        semantic_block(
            110,
            vec![initialization],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        semantic_block(
            111,
            vec![guard],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(predicate, bool_ty),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        semantic_block(
            112,
            vec![
                SemanticStatementV1::new(source(3), SemanticStatementKindV1::Nop),
                checked_add,
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(field(checked_result, 1, bool_ty)),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: copy(induction, u32_ty),
                    right: constant(1),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        semantic_block(
            113,
            vec![update],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        semantic_block(114, exit_statements, exit_terminator),
    ];
    blocks.extend(extra_blocks);

    let direct_u32 = SemanticAbiValueV1::new(
        u32_ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(120, seed)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250, seed)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_u32)],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag, seed)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    };
    let mut locals = vec![
        local(130, unit, SemanticLocalRoleV1::Return),
        local(131, u32_ty, SemanticLocalRoleV1::Temporary),
        local(132, u32_ty, SemanticLocalRoleV1::Argument(0)),
        local(133, bool_ty, SemanticLocalRoleV1::Temporary),
        local(134, checked_u32, SemanticLocalRoleV1::Temporary),
    ];
    locals.extend(extra_locals);
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(121, seed)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(122, seed)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(123, seed)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(124, seed)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(125, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(format!("proof_induction_{seed}").into_bytes()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(126, seed)),
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
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(production_target_layout_identity()),
        vec![unit_type(seed), u32_type, bool_type, checked_type],
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn middle_end_v5_bytes(source_semantic_identity: [u8; 32], seed: u8) -> Vec<u8> {
    const IDENTITY_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0";
    const RANKED_IR: &[u8] = b"func @proof_input_test {\n  kernel.return\n}\n";
    const PASS_TAGS: [u8; 8] = [1, 2, 3, 4, 8, 5, 6, 7];
    const DECLARED_LENGTH_OFFSET: usize = 12;

    let mut encoded = Vec::new();
    encoded.extend_from_slice(b"F2MEV5\0\0");
    encoded.extend_from_slice(&5_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u64.to_le_bytes());
    encoded.extend_from_slice(&0_u32.to_le_bytes());
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5);
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5);
    encoded.push(1);
    encoded.push(1);
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&source_semantic_identity);
    encoded.extend_from_slice(&bytes(90, seed));
    encoded.extend_from_slice(&(RANKED_IR.len() as u32).to_le_bytes());
    encoded.extend_from_slice(RANKED_IR);
    encoded.push(PASS_TAGS.len() as u8);
    for tag in PASS_TAGS {
        encoded.push(tag);
        encoded.push(1);
        encoded.extend_from_slice(&0_u32.to_le_bytes());
        encoded.push(0);
        encoded.push(0);
        encoded.extend_from_slice(&0_u16.to_le_bytes());
    }
    for _ in 0..4 + 6 + 10 + 2 {
        encoded.extend_from_slice(&0_u64.to_le_bytes());
    }
    encoded.extend_from_slice(&bytes(91, seed));

    let total_length = encoded.len() + 32;
    encoded[DECLARED_LENGTH_OFFSET..DECLARED_LENGTH_OFFSET + 8]
        .copy_from_slice(&(total_length as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((encoded.len() as u64).to_le_bytes());
    digest.update(&encoded);
    encoded.extend_from_slice(&<[u8; 32]>::from(digest.finalize()));
    assert_eq!(encoded.len(), total_length);
    encoded
}
