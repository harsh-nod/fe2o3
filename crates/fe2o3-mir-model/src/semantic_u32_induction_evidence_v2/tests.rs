use super::*;
use crate::semantic_direct_call_expansion_v1::SemanticCallExpansionLimitsV1;
use crate::semantic_mir_v1::*;
use crate::semantic_u32_induction::{
    MAX_SEMANTIC_U32_INDUCTION_WORK_V1, analyze_semantic_u32_induction_no_overflow_v1,
};
use crate::semantic_u32_induction_evidence_v1::InertCanonicalSemanticU32InductionEvidenceV1;

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn identity(seed: u32, tag: u32) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&(seed + 1).to_le_bytes());
    bytes[4..8].copy_from_slice(&tag.to_le_bytes());
    bytes
}

fn types(seed: u32) -> Vec<SemanticTypeDeclV1> {
    let scalar = |size, align, primitive, maximum| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            align,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap()
    };
    [
        (
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
            scalar(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u128::from(u32::MAX),
            ),
        ),
        (
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            scalar(1, 1, SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
        ),
        (
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, BOOL]).unwrap()),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
        ),
        (
            SemanticTypeShapeV1::Unit,
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
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (shape, layout))| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(seed, index as u32 + 1)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, index as u32 + 10)),
            layout,
            shape,
        )
    })
    .collect()
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn field(local: u32, index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn block(
    seed: u32,
    index: u32,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(seed, 100 + index)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn function(
    seed: u32,
    role: SemanticFunctionRoleV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut local_types = vec![
        (UNIT, SemanticLocalRoleV1::Return),
        (U32, SemanticLocalRoleV1::Argument(0)),
    ];
    if role == SemanticFunctionRoleV1::KernelRoot {
        local_types.extend([
            (U32, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (PAIR, SemanticLocalRoleV1::Temporary),
            (UNIT, SemanticLocalRoleV1::Temporary),
        ]);
    }
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(seed, 20 + index as u32)),
                ty,
                role,
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    function_with_output(
        seed,
        role,
        locals,
        blocks,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
}

fn function_with_output(
    seed: u32,
    role: SemanticFunctionRoleV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
    output: SemanticAbiValueV1,
) -> SemanticFunctionDeclV1 {
    let argument = SemanticAbiValueV1::new(
        U32,
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
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(seed, 40)),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(seed, 41)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(seed, 42)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(seed, 43)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(seed, 44)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(identity(seed, 45)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, 46)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![argument],
            output,
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn loop_blocks(seed: u32, start: u32, induction: u32, step: u128) -> Vec<SemanticBasicBlockV1> {
    let predicate = induction + 1;
    let checked = induction + 2;
    vec![
        block(
            seed,
            start,
            vec![assign(
                induction,
                U32,
                SemanticRvalueKindV1::Use(constant(0)),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, start + 1)),
        ),
        block(
            seed,
            start + 1,
            vec![assign(
                predicate,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: copy(induction, U32),
                    right: copy(1, U32),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(predicate, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, start + 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, start + 2),
                )
                .unwrap(),
            },
        ),
        block(
            seed,
            start + 2,
            vec![assign(
                checked,
                PAIR,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    copy(induction, U32),
                    constant(step),
                )),
            )],
            SemanticTerminatorKindV1::Assert {
                condition: field(checked, 1, BOOL),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: copy(induction, U32),
                    right: constant(step),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, start + 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            seed,
            start + 3,
            vec![assign(
                induction,
                U32,
                SemanticRvalueKindV1::Use(field(checked, 0, U32)),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, start + 1)),
        ),
    ]
}

fn helper_call(
    callee: u32,
    target: u32,
    argument: SemanticOperandV1,
    output: SemanticPlaceV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(callee),
            vec![argument],
            Some(SemanticCallDestinationV1::new(
                output,
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn source(seed: u32, calls: bool, roots: u32, step: u128) -> AdmittedInertSemanticMirV1 {
    let mut functions = Vec::new();
    for root in 0..roots {
        let seed = seed * 10 + root;
        let mut blocks = loop_blocks(seed, 0, 2, step);
        if calls {
            for index in 4..6 {
                blocks.push(block(
                    seed,
                    index,
                    vec![],
                    helper_call(roots, index + 1, constant(index.into()), place(5, UNIT)),
                ));
            }
        }
        blocks.push(block(
            seed,
            blocks.len() as u32,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        functions.push(function(seed, SemanticFunctionRoleV1::KernelRoot, blocks));
    }
    if calls {
        let seed = seed * 10 + roots;
        functions.push(function(
            seed,
            SemanticFunctionRoleV1::InternalHelper,
            vec![block(seed, 0, vec![], SemanticTerminatorKindV1::Return)],
        ));
    }
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(
            seed, 200,
        ))),
        types(seed),
        vec![],
        vec![],
        vec![],
        functions,
        (0..roots).map(SemanticFunctionIdV1::from_index).collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

struct Fixture {
    source: AdmittedInertSemanticMirV1,
    expansion: SemanticCallExpansionV1,
    origins: InertCanonicalSemanticCallExpansionEvidenceV1,
    report: SemanticU32InductionNoOverflowReportV1,
}
impl Fixture {
    fn new(seed: u32, calls: bool, roots: u32, step: u128) -> Self {
        Self::from_source(source(seed, calls, roots, step))
    }
    fn from_source(source: AdmittedInertSemanticMirV1) -> Self {
        let expansion =
            SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
                .unwrap();
        let origins = InertCanonicalSemanticCallExpansionEvidenceV1::from_checked_expansion(
            &source, &expansion,
        )
        .unwrap();
        let report = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
            &source,
            &expansion,
            ROOT,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .unwrap();
        Self {
            source,
            expansion,
            origins,
            report,
        }
    }
    fn evidence(&self) -> InertCanonicalSemanticU32InductionEvidenceV2 {
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &self.source,
            &self.expansion,
            &self.origins,
            ROOT,
            &self.report,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .unwrap()
    }
    fn replay(
        &self,
        evidence: &InertCanonicalSemanticU32InductionEvidenceV2,
    ) -> Result<SemanticU32InductionNoOverflowReportV1> {
        evidence.verify_replay(
            &self.source,
            &self.expansion,
            &self.origins,
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
    }
}

fn admit_variant(
    original: &AdmittedInertSemanticMirV1,
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new(
        original.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn induction_v2_transparent_result_wrapper_retains_distinct_source_body() {
    let original = source(0, true, 2, 1);
    let result = SemanticTypeIdV1::from_index(4);
    let mut types = original.types().to_vec();
    let variants = (0..2)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                8,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                100 + u64::from(index),
                SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity(0, 5)),
        SemanticLayoutIdentityV1::from_sha256(identity(0, 14)),
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![UNIT]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
            ],
        )
        .unwrap(),
    ));
    let mut wrapper_locals = original.functions()[0].locals().to_vec();
    wrapper_locals[5] = SemanticLocalDeclV1::new(
        wrapper_locals[5].identity(),
        result,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    );
    let wrapper = function_with_output(
        0,
        SemanticFunctionRoleV1::KernelRoot,
        wrapper_locals,
        vec![
            block(
                0,
                0,
                vec![],
                helper_call(1, 1, copy(1, U32), place(5, result)),
            ),
            block(0, 1, vec![], SemanticTerminatorKindV1::Return),
        ],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    );
    let mut body_locals = original.functions()[1].locals().to_vec();
    body_locals[0] = SemanticLocalDeclV1::new(
        body_locals[0].identity(),
        result,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    );
    let mut body_blocks = original.functions()[1].blocks().to_vec();
    for (index, slot) in body_blocks.iter_mut().enumerate().take(6).skip(4) {
        *slot = block(
            1,
            index as u32,
            vec![],
            helper_call(2, index as u32 + 1, copy(1, U32), place(5, UNIT)),
        );
    }
    body_blocks[6] = block(
        1,
        6,
        vec![assign(
            0,
            result,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(0),
                vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))],
            )
            .unwrap(),
        )],
        SemanticTerminatorKindV1::Return,
    );
    let body = function_with_output(
        1,
        SemanticFunctionRoleV1::InternalHelper,
        body_locals,
        body_blocks,
        SemanticAbiValueV1::new(
            result,
            SemanticAbiPassModeV1::cast(
                false,
                SemanticAbiCastV1::new(
                    [None; 8],
                    None,
                    SemanticAbiUniformV1::from_rustc(
                        SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                        8,
                        false,
                    )
                    .unwrap(),
                    SemanticAbiValueAttributesV1::plain(),
                ),
            ),
        ),
    );
    let mut helper_blocks = loop_blocks(2, 0, 2, 1);
    helper_blocks.push(block(2, 4, vec![], SemanticTerminatorKindV1::Return));
    let helper = function(2, SemanticFunctionRoleV1::KernelRoot, helper_blocks)
        .with_role(SemanticFunctionRoleV1::InternalHelper);
    let f = Fixture::from_source(admit_variant(&original, types, vec![wrapper, body, helper]));
    let selection = f.source.select_kernel_body_for_root_v1(ROOT).unwrap();
    assert!(selection.has_transparent_result_wrapper());
    assert_eq!(selection.body(), SemanticFunctionIdV1::from_index(1));
    assert_eq!(f.report.certificates().len(), 3);
    let evidence = f.evidence();
    assert_eq!(evidence.root(), ROOT);
    assert_eq!(evidence.function(), 1);
    assert_eq!(f.report.function(), selection.body());
    assert_eq!(
        evidence.semantic_mir_sha256(),
        f.source.semantic_sha256().as_bytes()
    );
    let decoded =
        InertCanonicalSemanticU32InductionEvidenceV2::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(f.replay(&decoded).unwrap(), f.report);
    let mut substituted = evidence.canonical_bytes().to_vec();
    substituted[152..156].copy_from_slice(&ROOT.index().to_le_bytes());
    let substituted = InertCanonicalSemanticU32InductionEvidenceV2::decode(&substituted).unwrap();
    assert_eq!(
        f.replay(&substituted),
        Err(SemanticU32InductionEvidenceErrorV2::BindingMismatch)
    );
}

#[test]
fn induction_v2_rejects_swapped_certificate_order_and_helper_instance_bindings() {
    let original = source(0, true, 1, 1);
    let mut locals = original.functions()[0].locals().to_vec();
    for ty in [U32, BOOL, PAIR] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(0, 20 + locals.len() as u32)),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let mut blocks = loop_blocks(0, 0, 2, 1);
    blocks.extend(loop_blocks(0, 4, 6, 1));
    for index in 8..10 {
        blocks.push(block(
            0,
            index,
            vec![],
            helper_call(1, index + 1, constant(4), place(5, UNIT)),
        ));
    }
    blocks.push(block(0, 10, vec![], SemanticTerminatorKindV1::Return));
    let root = function_with_output(
        0,
        SemanticFunctionRoleV1::KernelRoot,
        locals,
        blocks,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    );
    let f = Fixture::from_source(admit_variant(
        &original,
        original.types().to_vec(),
        vec![root, original.functions()[1].clone()],
    ));
    assert_eq!(f.report.certificates().len(), 2);
    let evidence = f.evidence();
    assert_eq!(f.replay(&evidence).unwrap(), f.report);
    let mut reordered = evidence.canonical_bytes().to_vec();
    let second = HEADER_BYTES + CERTIFICATE_BYTES_V1;
    reordered[HEADER_BYTES..second].copy_from_slice(&evidence.canonical_bytes()[second..]);
    reordered[second..].copy_from_slice(&evidence.canonical_bytes()[HEADER_BYTES..second]);
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::decode(&reordered),
        Err(SemanticU32InductionEvidenceErrorV2::Codec(
            SemanticU32InductionEvidenceErrorV1::NonCanonical
        ))
    ));
    let view = f.expansion.root(ROOT).unwrap();
    for instance in &view.instances()[1..] {
        let argument = instance.local_start() + 1;
        let local = &view.body().locals()[argument as usize];
        assert_eq!(local.ty(), U32);
        let mut swapped = evidence.canonical_bytes().to_vec();
        let bound = HEADER_BYTES + 2 * 72;
        swapped[bound..bound + 4].copy_from_slice(&argument.to_le_bytes());
        swapped[bound + 4..bound + 36].copy_from_slice(local.identity().as_bytes());
        let swapped = InertCanonicalSemanticU32InductionEvidenceV2::decode(&swapped).unwrap();
        assert_eq!(
            f.replay(&swapped),
            Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
        );
    }
}

#[test]
fn expanded_helper_argument_bounds_use_exact_repeated_parameter_transfers() {
    let original = source(0, false, 2, 1);
    let caller = function(
        0,
        SemanticFunctionRoleV1::KernelRoot,
        vec![
            block(0, 0, vec![], helper_call(1, 1, constant(8), place(5, UNIT))),
            block(
                0,
                1,
                vec![],
                helper_call(1, 2, constant(16), place(5, UNIT)),
            ),
            block(0, 2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let source = admit_variant(
        &original,
        original.types().to_vec(),
        vec![
            caller,
            original.functions()[1]
                .clone()
                .with_role(SemanticFunctionRoleV1::InternalHelper),
        ],
    );
    let source_report =
        analyze_semantic_u32_induction_no_overflow_v1(&source, SemanticFunctionIdV1::from_index(1))
            .unwrap();
    assert_eq!(source_report.certificates().len(), 1);
    let f = Fixture::from_source(source);
    assert_eq!(f.report.checked_additions_examined(), 2);
    assert_eq!(f.report.certificates().len(), 2);
    let view = f.expansion.root(ROOT).unwrap();
    for instance in &view.instances()[1..] {
        let argument = SemanticLocalIdV1::from_index(instance.local_start() + 1);
        assert_eq!(
            view.body().locals()[argument.index() as usize].role(),
            SemanticLocalRoleV1::Temporary
        );
        let definitions = view
            .body()
            .blocks()
            .iter()
            .flat_map(|block| block.statements())
            .filter(|statement| {
                matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().local() == argument)
            })
            .count();
        assert_eq!(definitions, 1);
        assert_eq!(
            f.report
                .certificates()
                .iter()
                .filter(|certificate| certificate.bound().local() == argument)
                .count(),
            1
        );
    }
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &f.origins,
            ROOT,
            &source_report,
            SemanticU32InductionAnalysisLimitsV1::default()
        ),
        Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
    ));
    assert_eq!(f.replay(&f.evidence()).unwrap(), f.report);
}

#[test]
fn expanded_root_bound_forwarded_to_helper_preserves_original_argument() {
    let original = source(0, true, 1, 1);
    let mut blocks = original.functions()[0].blocks().to_vec();
    for (index, slot) in blocks.iter_mut().enumerate().take(6).skip(4) {
        *slot = block(
            0,
            index as u32,
            vec![],
            helper_call(1, index as u32 + 1, copy(1, U32), place(5, UNIT)),
        );
    }
    let root = function(0, SemanticFunctionRoleV1::KernelRoot, blocks);
    let source = admit_variant(
        &original,
        original.types().to_vec(),
        vec![root, original.functions()[1].clone()],
    );
    assert_eq!(
        analyze_semantic_u32_induction_no_overflow_v1(&source, ROOT)
            .unwrap()
            .certificates()
            .len(),
        1
    );
    let f = Fixture::from_source(source);
    assert_eq!(f.report.checked_additions_examined(), 1);
    assert_eq!(f.report.certificates().len(), 1);
    let view = f.expansion.root(ROOT).unwrap();
    assert_eq!(
        view.body().locals()[1].role(),
        SemanticLocalRoleV1::Argument(0)
    );
    let forwarded = view.body().blocks().iter().flat_map(|block| block.statements())
        .filter(|statement| {
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                    if place.local() == SemanticLocalIdV1::from_index(1)))
        }).count();
    assert_eq!(forwarded, 2);
    assert_eq!(f.replay(&f.evidence()).unwrap(), f.report);
}

fn helper_loop_source(
    depth: u32,
    repeated: bool,
    argument: SemanticOperandV1,
) -> AdmittedInertSemanticMirV1 {
    let original = source(0, false, 1, 1);
    let functions = (0..=depth)
        .map(|index| {
            let mut blocks = if index == depth {
                loop_blocks(index, 0, 2, 1)
            } else {
                (0..if repeated { 2 } else { 1 })
                    .map(|call| {
                        block(
                            index,
                            call,
                            vec![],
                            helper_call(index + 1, call + 1, argument.clone(), place(5, UNIT)),
                        )
                    })
                    .collect()
            };
            blocks.push(block(
                index,
                blocks.len() as u32,
                vec![],
                SemanticTerminatorKindV1::Return,
            ));
            function(index, SemanticFunctionRoleV1::KernelRoot, blocks).with_role(if index == 0 {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            })
        })
        .collect();
    admit_variant(&original, original.types().to_vec(), functions)
}

fn change_helper_function(
    source: &AdmittedInertSemanticMirV1,
    index: usize,
    change: impl FnOnce(&mut Vec<SemanticLocalDeclV1>, &mut Vec<SemanticBasicBlockV1>),
) -> AdmittedInertSemanticMirV1 {
    let mut functions = source.functions().to_vec();
    let mut locals = functions[index].locals().to_vec();
    let mut blocks = functions[index].blocks().to_vec();
    change(&mut locals, &mut blocks);
    functions[index] = function_with_output(
        index as u32,
        functions[index].role(),
        locals,
        blocks,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    );
    admit_variant(source, source.types().to_vec(), functions)
}

fn prepend_helper_statements(
    source: &AdmittedInertSemanticMirV1,
    function: usize,
    target: usize,
    mut statements: Vec<SemanticStatementV1>,
) -> AdmittedInertSemanticMirV1 {
    change_helper_function(source, function, |_, blocks| {
        statements.extend_from_slice(blocks[target].statements());
        blocks[target] = block(
            function as u32,
            target as u32,
            statements,
            blocks[target].terminator().kind().clone(),
        );
    })
}

#[test]
fn expanded_helper_bounds_follow_nested_instances_and_reject_evidence_substitution() {
    let f = Fixture::from_source(helper_loop_source(2, true, copy(1, U32)));
    let view = f.expansion.root(ROOT).unwrap();
    assert_eq!(view.instances().len(), 7);
    assert_eq!(f.report.certificates().len(), 4);
    let instances: std::collections::BTreeSet<_> = f
        .report
        .certificates()
        .iter()
        .map(|certificate| {
            let bound = view.local_origins()[certificate.bound().local().index() as usize];
            assert_eq!(bound.function(), SemanticFunctionIdV1::from_index(2));
            assert_eq!(bound.local(), SemanticLocalIdV1::from_index(1));
            assert_eq!(
                view.block_origins()[certificate.guard().block().block().index() as usize]
                    .instance(),
                bound.instance()
            );
            bound.instance().index()
        })
        .collect();
    assert_eq!(instances.len(), 4);
    let evidence = f.evidence();
    assert_eq!(f.replay(&evidence).unwrap(), f.report);
    assert_eq!(
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&f.report),
        Err(SemanticU32InductionEvidenceErrorV1::ExecutionViewUnsupported)
    );
    let omitted = encode(
        evidence.binding,
        evidence.checked_additions_examined,
        evidence.work_units,
        &evidence.certificates[..3],
        &mut CodecBudget::default(),
    )
    .unwrap();
    assert_eq!(
        f.replay(&InertCanonicalSemanticU32InductionEvidenceV2::decode(&omitted).unwrap()),
        Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
    );
    let other = f.report.certificates()[1].bound();
    let mut bytes = evidence.canonical_bytes().to_vec();
    let offset = HEADER_BYTES + 2 * 72;
    bytes[offset..offset + 4].copy_from_slice(&other.local().index().to_le_bytes());
    bytes[offset + 4..offset + 36].copy_from_slice(
        view.body().locals()[other.local().index() as usize]
            .identity()
            .as_bytes(),
    );
    assert_eq!(
        f.replay(&InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap()),
        Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
    );
}

#[test]
fn expanded_helper_bounds_reject_rewrites_and_ordinary_aliases() {
    let source = helper_loop_source(2, false, copy(1, U32));
    assert_eq!(
        Fixture::from_source(helper_loop_source(2, false, copy(1, U32)))
            .report
            .certificates()
            .len(),
        1
    );
    for (function, target) in [(0, 0), (1, 0), (2, 0), (2, 1), (2, 3)] {
        let rewritten = prepend_helper_statements(
            &source,
            function,
            target,
            vec![assign(1, U32, SemanticRvalueKindV1::Use(constant(9)))],
        );
        assert!(
            Fixture::from_source(rewritten)
                .report
                .certificates()
                .is_empty(),
            "rewritten {function}:{target}"
        );
    }
    for function in 0..=2 {
        let aliased = change_helper_function(&source, function, |locals, blocks| {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(function as u32, 26)),
                U32,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
            let mut statements = vec![assign(6, U32, SemanticRvalueKindV1::Use(copy(1, U32)))];
            statements.extend_from_slice(blocks[0].statements());
            blocks[0] = block(
                function as u32,
                0,
                statements,
                blocks[0].terminator().kind().clone(),
            );
        });
        assert!(
            Fixture::from_source(aliased)
                .report
                .certificates()
                .is_empty(),
            "aliased {function}"
        );
    }
    let temporary = change_helper_function(&source, 0, |_, blocks| {
        blocks[0] = block(
            0,
            0,
            vec![assign(2, U32, SemanticRvalueKindV1::Use(copy(1, U32)))],
            helper_call(1, 1, copy(2, U32), place(5, UNIT)),
        );
    });
    assert!(
        Fixture::from_source(temporary)
            .report
            .certificates()
            .is_empty()
    );
}

#[test]
fn expanded_helper_bounds_preserve_move_and_storage_availability() {
    let moved = SemanticOperandV1::Move(place(1, U32));
    let once = Fixture::from_source(helper_loop_source(2, false, moved.clone()));
    assert_eq!(once.report.certificates().len(), 1);
    let repeated = Fixture::from_source(helper_loop_source(2, true, moved));
    assert_eq!(repeated.report.checked_additions_examined(), 4);
    assert_eq!(repeated.report.certificates().len(), 1);
    assert_eq!(
        repeated.replay(&repeated.evidence()).unwrap(),
        repeated.report
    );
    let source = helper_loop_source(1, false, copy(1, U32));
    for function in 0..=1 {
        for kind in [
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
            SemanticStatementKindV1::Deinitialize(place(1, U32)),
        ] {
            let dead = prepend_helper_statements(
                &source,
                function,
                0,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    kind,
                )],
            );
            assert!(
                Fixture::from_source(dead).report.certificates().is_empty(),
                "dead argument {function}"
            );
        }
    }
    let moved_in_guard = change_helper_function(&source, 1, |_, blocks| {
        blocks[1] = block(
            1,
            1,
            vec![assign(
                3,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: copy(2, U32),
                    right: SemanticOperandV1::Move(place(1, U32)),
                },
            )],
            blocks[1].terminator().kind().clone(),
        );
    });
    assert!(
        Fixture::from_source(moved_in_guard)
            .report
            .certificates()
            .is_empty()
    );
}

#[test]
fn expanded_helper_bounds_reinitialize_static_frames_on_caller_backedges() {
    for (argument, expected) in [
        (copy(1, U32), 2),
        (SemanticOperandV1::Move(place(1, U32)), 0),
    ] {
        let source = helper_loop_source(1, false, argument.clone());
        let source = change_helper_function(&source, 0, |_, blocks| {
            *blocks = loop_blocks(0, 0, 2, 1);
            let checked = blocks[2].clone();
            blocks[2] = block(0, 2, vec![], helper_call(1, 5, argument, place(5, UNIT)));
            blocks.push(block(0, 4, vec![], SemanticTerminatorKindV1::Return));
            blocks.push(block(
                0,
                5,
                checked.statements().to_vec(),
                checked.terminator().kind().clone(),
            ));
        });
        let f = Fixture::from_source(source);
        assert_eq!(f.expansion.root(ROOT).unwrap().instances().len(), 2);
        assert_eq!(f.report.checked_additions_examined(), 2);
        assert_eq!(f.report.certificates().len(), expected);
        assert_eq!(f.replay(&f.evidence()).unwrap(), f.report);
    }
}

#[test]
fn expanded_helper_bounds_allow_early_return_not_guard_bypass_or_conditional_death() {
    let source = helper_loop_source(1, false, copy(1, U32));
    for variant in 0..3 {
        let source = change_helper_function(&source, 1, |locals, blocks| {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(1, 26)),
                BOOL,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
            *blocks = vec![block(
                1,
                0,
                vec![assign(
                    6,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: constant(0),
                        right: copy(1, U32),
                    },
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: copy(6, BOOL),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(
                                SemanticEdgeRoleV1::SwitchValue,
                                if variant == 1 { 3 } else { 6 },
                            ),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                },
            )];
            blocks.extend(loop_blocks(1, 1, 2, 1));
            blocks.push(block(1, 5, vec![], SemanticTerminatorKindV1::Return));
            if variant != 1 {
                blocks.push(block(
                    1,
                    6,
                    if variant == 2 {
                        vec![SemanticStatementV1::new(
                            SemanticSourceProvenanceV1::unavailable(),
                            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
                        )]
                    } else {
                        vec![]
                    },
                    if variant == 2 {
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
                    } else {
                        SemanticTerminatorKindV1::Return
                    },
                ));
            }
        });
        let f = Fixture::from_source(source);
        assert_eq!(
            f.report.certificates().len(),
            usize::from(variant == 0),
            "variant {variant}"
        );
    }
}

#[test]
fn expanded_helper_bounds_reject_projected_arguments_and_signed_types() {
    let source = helper_loop_source(1, false, copy(1, U32));
    let projected = change_helper_function(&source, 0, |_, blocks| {
        blocks[0] = block(
            0,
            0,
            vec![assign(
                4,
                PAIR,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        constant(8),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            BOOL,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 1).unwrap(),
                            ),
                        )),
                    ],
                )
                .unwrap(),
            )],
            helper_call(1, 1, field(4, 0, U32), place(5, UNIT)),
        );
    });
    assert!(
        Fixture::from_source(projected)
            .report
            .certificates()
            .is_empty()
    );
    let mut types = source.types().to_vec();
    types[0] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(true, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
    );
    let signed = admit_variant(&source, types, source.functions().to_vec());
    assert!(
        Fixture::from_source(signed)
            .report
            .certificates()
            .is_empty()
    );
}

#[test]
fn expanded_helper_bounds_share_cumulative_work_and_certificate_limits() {
    let f = Fixture::from_source(helper_loop_source(2, true, copy(1, U32)));
    let evidence = f.evidence();
    let work = f.report.work_units();
    assert_eq!(f.report.certificates().len(), 4);
    assert_eq!(
        evidence
            .verify_replay(
                &f.source,
                &f.expansion,
                &f.origins,
                SemanticU32InductionAnalysisLimitsV1::new(work, 4)
            )
            .unwrap(),
        f.report
    );
    for limit in [0, work / 2, work - 1] {
        assert!(matches!(
            evidence.verify_replay(
                &f.source,
                &f.expansion,
                &f.origins,
                SemanticU32InductionAnalysisLimitsV1::new(limit, 4)
            ),
            Err(SemanticU32InductionEvidenceErrorV2::Analysis(
                SemanticU32InductionAnalysisErrorV1::WorkLimit { .. }
            ))
        ));
    }
    assert!(matches!(
        evidence.verify_replay(
            &f.source,
            &f.expansion,
            &f.origins,
            SemanticU32InductionAnalysisLimitsV1::new(work, 3)
        ),
        Err(SemanticU32InductionEvidenceErrorV2::Analysis(
            SemanticU32InductionAnalysisErrorV1::CertificateLimit { .. }
        ))
    ));
}

#[test]
fn expanded_loop_evidence_round_trips_complete_report_and_repeated_instances() {
    let f = Fixture::new(0, true, 1, 1);
    let view = f.expansion.root(ROOT).unwrap();
    assert_eq!(view.instances().len(), 3);
    assert_ne!(
        view.instances()[1].local_start(),
        view.instances()[2].local_start()
    );
    assert_ne!(
        view.instances()[1].block_start(),
        view.instances()[2].block_start()
    );
    assert_eq!(f.report.checked_additions_examined(), 1);
    assert_eq!(f.report.certificates().len(), 1);
    let evidence = f.evidence();
    evidence.revalidate().unwrap();
    assert_eq!(f.replay(&evidence).unwrap(), f.report);
    let decoded =
        InertCanonicalSemanticU32InductionEvidenceV2::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    assert_eq!(f.replay(&decoded).unwrap(), f.report);
    assert_eq!(
        evidence.semantic_mir_sha256(),
        f.source.semantic_sha256().as_bytes()
    );
    assert_eq!(evidence.expansion_evidence_identity(), f.origins.identity());
    assert_eq!(evidence.expansion_identity(), f.expansion.identity());
    assert_eq!(evidence.execution_view_identity(), view.identity());
    assert_ne!(
        evidence.expansion_identity(),
        evidence.execution_view_identity()
    );
    assert_ne!(
        evidence.expansion_identity(),
        evidence.expansion_evidence_identity()
    );
    assert_eq!(evidence.function(), view.source_body().index());
    assert_eq!(
        evidence.function_identity(),
        view.body().identity().as_bytes()
    );
    assert_ne!(
        evidence.function_identity(),
        f.source.functions()[0].identity().as_bytes()
    );
    assert_eq!(
        evidence.certificates()[0],
        certificate_from_live(f.report.certificates()[0])
    );
    assert!(!evidence.grants_authority());
    assert!(!evidence.authorizes_compiler_transform());
    assert!(!f.replay(&decoded).unwrap().grants_authority());
    assert_eq!(
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&f.report),
        Err(SemanticU32InductionEvidenceErrorV1::ExecutionViewUnsupported)
    );
}

#[test]
fn induction_v2_rejects_subset_and_mutated_report_custody() {
    let f = Fixture::new(0, true, 1, 1);
    let evidence = f.evidence();
    let omitted = encode(
        evidence.binding,
        evidence.checked_additions_examined,
        evidence.work_units,
        &[],
        &mut CodecBudget::default(),
    )
    .unwrap();
    let omitted = InertCanonicalSemanticU32InductionEvidenceV2::decode(&omitted).unwrap();
    assert_eq!(
        f.replay(&omitted),
        Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
    );

    for offset in [
        188,
        192,
        HEADER_BYTES + 2 * 72,
        HEADER_BYTES + 2 * 72 + 4,
        HEADER_BYTES + 5 * 72 + 4 * 36 + 40 + 44 + 40,
    ] {
        let mut bytes = evidence.canonical_bytes().to_vec();
        bytes[offset] ^= 2;
        let changed = InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap();
        assert_eq!(
            f.replay(&changed),
            Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch),
            "offset {offset}"
        );
    }
    let mut duplicated = evidence.canonical_bytes().to_vec();
    duplicated.extend_from_slice(&evidence.canonical_bytes()[HEADER_BYTES..]);
    let length = duplicated.len() as u32;
    duplicated[16..20].copy_from_slice(&length.to_le_bytes());
    duplicated[188..192].copy_from_slice(&2_u32.to_le_bytes());
    duplicated[200..204].copy_from_slice(&2_u32.to_le_bytes());
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::decode(&duplicated),
        Err(SemanticU32InductionEvidenceErrorV2::Codec(
            SemanticU32InductionEvidenceErrorV1::NonCanonical
        ))
    ));
}

#[test]
fn induction_v2_rejects_cross_root_source_and_identity_substitution() {
    let f = Fixture::new(0, true, 2, 1);
    let evidence = f.evidence();
    let other_report = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
        &f.source,
        &f.expansion,
        SemanticFunctionIdV1::from_index(1),
        SemanticU32InductionAnalysisLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &f.origins,
            ROOT,
            &other_report,
            SemanticU32InductionAnalysisLimitsV1::default()
        ),
        Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch)
    ));
    for offset in [20, 52, 84, 116, 120, 152, 156] {
        let mut bytes = evidence.canonical_bytes().to_vec();
        bytes[offset] ^= 1;
        let changed = InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap();
        assert_eq!(
            f.replay(&changed),
            Err(SemanticU32InductionEvidenceErrorV2::BindingMismatch),
            "offset {offset}"
        );
    }
    let other = Fixture::new(10, true, 2, 1);
    assert!(
        evidence
            .verify_replay(
                &other.source,
                &f.expansion,
                &f.origins,
                SemanticU32InductionAnalysisLimitsV1::default()
            )
            .is_err()
    );
    assert!(
        evidence
            .verify_replay(
                &f.source,
                &other.expansion,
                &f.origins,
                SemanticU32InductionAnalysisLimitsV1::default()
            )
            .is_err()
    );
    assert!(
        evidence
            .verify_replay(
                &f.source,
                &f.expansion,
                &other.origins,
                SemanticU32InductionAnalysisLimitsV1::default()
            )
            .is_err()
    );
    assert!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &f.origins,
            SemanticFunctionIdV1::from_index(2),
            &f.report,
            SemanticU32InductionAnalysisLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn induction_v2_replays_expansion_origin_bytes_not_claimed_hashes() {
    let f = Fixture::new(0, true, 1, 1);
    let evidence = f.evidence();
    let local_identity = f.expansion.root(ROOT).unwrap().body().locals()[2].identity();
    let mut bytes = f.origins.canonical_bytes().to_vec();
    let offsets: Vec<_> = bytes
        .windows(32)
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == local_identity.as_bytes()).then_some(offset))
        .collect();
    let [offset] = offsets.as_slice() else {
        panic!("expected one exact execution-local identity");
    };
    bytes[*offset] ^= 1;
    let changed = InertCanonicalSemanticCallExpansionEvidenceV1::decode(&bytes).unwrap();
    assert_ne!(changed.identity(), f.origins.identity());
    assert_eq!(changed.expansion_identity(), f.origins.expansion_identity());
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &changed,
            ROOT,
            &f.report,
            SemanticU32InductionAnalysisLimitsV1::default()
        ),
        Err(SemanticU32InductionEvidenceErrorV2::Expansion(_))
    ));
    assert!(matches!(
        evidence.verify_replay(
            &f.source,
            &f.expansion,
            &changed,
            SemanticU32InductionAnalysisLimitsV1::default()
        ),
        Err(SemanticU32InductionEvidenceErrorV2::Expansion(_))
    ));
}

#[test]
fn induction_v2_call_free_reports_remain_exact_v1() {
    let f = Fixture::new(0, false, 1, 1);
    let original = analyze_semantic_u32_induction_no_overflow_v1(&f.source, ROOT).unwrap();
    assert_eq!(f.report, original);
    assert_eq!(f.report.execution_view_identity(), None);
    let v1 = InertCanonicalSemanticU32InductionEvidenceV1::from_report(&original).unwrap();
    assert_eq!(
        v1,
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&f.report).unwrap()
    );
    v1.revalidate().unwrap();
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &f.origins,
            ROOT,
            &f.report,
            SemanticU32InductionAnalysisLimitsV1::default()
        ),
        Err(SemanticU32InductionEvidenceErrorV2::CallFreeRequiresV1)
    ));
    assert!(InertCanonicalSemanticU32InductionEvidenceV2::decode(v1.canonical_bytes()).is_err());
}

#[test]
fn induction_v2_zero_certificates_replay_only_when_analysis_agrees() {
    let f = Fixture::new(0, true, 1, 2);
    assert_eq!(f.report.checked_additions_examined(), 1);
    assert!(f.report.certificates().is_empty());
    let evidence = f.evidence();
    assert_eq!(f.replay(&evidence).unwrap(), f.report);
}

#[test]
fn induction_v2_analysis_limits_apply_during_complete_replay() {
    let f = Fixture::new(0, true, 1, 1);
    let evidence = f.evidence();
    let exact = SemanticU32InductionAnalysisLimitsV1::new(f.report.work_units(), 1);
    assert_eq!(
        evidence
            .verify_replay(&f.source, &f.expansion, &f.origins, exact)
            .unwrap(),
        f.report
    );
    assert_eq!(
        analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
            &f.source,
            &f.expansion,
            ROOT,
            exact
        )
        .unwrap(),
        f.report
    );
    let short = SemanticU32InductionAnalysisLimitsV1::new(f.report.work_units() - 1, 1);
    assert!(matches!(
        evidence.verify_replay(&f.source, &f.expansion, &f.origins, short),
        Err(SemanticU32InductionEvidenceErrorV2::Analysis(
            SemanticU32InductionAnalysisErrorV1::WorkLimit { .. }
        ))
    ));
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::from_expanded_report(
            &f.source,
            &f.expansion,
            &f.origins,
            ROOT,
            &f.report,
            short
        ),
        Err(SemanticU32InductionEvidenceErrorV2::Analysis(
            SemanticU32InductionAnalysisErrorV1::WorkLimit { .. }
        ))
    ));
    assert!(matches!(
        evidence.verify_replay(
            &f.source,
            &f.expansion,
            &f.origins,
            SemanticU32InductionAnalysisLimitsV1::new(f.report.work_units(), 0)
        ),
        Err(SemanticU32InductionEvidenceErrorV2::Analysis(
            SemanticU32InductionAnalysisErrorV1::CertificateLimit { .. }
        ))
    ));
    assert!(matches!(
        analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
            &f.source,
            &f.expansion,
            ROOT,
            SemanticU32InductionAnalysisLimitsV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1 + 1, 1)
        ),
        Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits { .. })
    ));
}

#[test]
fn induction_v2_codec_limits_and_malformed_custody_fail_closed() {
    let f = Fixture::new(0, true, 1, 1);
    let evidence = f.evidence();
    let bytes = evidence.canonical_bytes();
    let mut budget = CodecBudget {
        used: 0,
        limit: bytes.len(),
    };
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::decode_with_budget(bytes, &mut budget),
        Err(SemanticU32InductionEvidenceErrorV2::CodecWorkLimit { .. })
    ));
    assert_eq!(budget.used, bytes.len());
    let mut budget = CodecBudget::default();
    InertCanonicalSemanticU32InductionEvidenceV2::decode_with_budget(bytes, &mut budget).unwrap();
    budget.limit = budget.used;
    assert!(matches!(
        InertCanonicalSemanticU32InductionEvidenceV2::decode_with_budget(bytes, &mut budget),
        Err(SemanticU32InductionEvidenceErrorV2::CodecWorkLimit { .. })
    ));
    for end in 0..bytes.len() {
        assert!(
            InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes[..end]).is_err(),
            "truncation {end}"
        );
    }
    for offset in [0, 8, 10, 12, 16, 200] {
        let mut invalid = bytes.to_vec();
        invalid[offset] ^= 0xff;
        assert!(
            InertCanonicalSemanticU32InductionEvidenceV2::decode(&invalid).is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(InertCanonicalSemanticU32InductionEvidenceV2::decode(&trailing).is_err());
    assert!(exact_size(usize::MAX).is_err());
    let mut corrupted_cache = evidence.clone();
    corrupted_cache.identity[0] ^= 1;
    assert!(corrupted_cache.revalidate().is_err());
    assert!(f.replay(&corrupted_cache).is_err());
}
