// Shared inert MIR34 fixture: real structural admission, not live source custody.
// This donor cannot mint compiler, artifact, source/native proof, or launch authority.
use fe2o3_mir_model::semantic_mir_v1::*;

mod base {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/defined_helper_semantic_fixture_v1.rs"
    ));
}
pub const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub const SINGLETON: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
pub const KINDS: [SemanticGfx942InlineInstructionV30; 6] = [
    SemanticGfx942InlineInstructionV30::VMovB32,
    SemanticGfx942InlineInstructionV30::VAddU32,
    SemanticGfx942InlineInstructionV30::VSubU32,
    SemanticGfx942InlineInstructionV30::VAndB32,
    SemanticGfx942InlineInstructionV30::VOrB32,
    SemanticGfx942InlineInstructionV30::VXorB32,
];
pub fn word(local: u32) -> SemanticPlaceV1 {
    base::place(local)
}
pub fn tuple(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SINGLETON).unwrap()
}
pub fn component(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), WORD).unwrap()],
        WORD,
    )
    .unwrap()
}
pub fn statement(place: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place.clone(),
            SemanticRvalueV1::new(place.ty(), value),
        )),
    )
}
pub fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    base::block(tag, statements, terminator)
}
pub fn replace_body(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let result = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = original.kernel_entry() {
        result.with_kernel_entry(entry.clone())
    } else {
        result
    }
}
fn value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let attribute = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Direct(attribute))
}
pub fn singleton_type(repr: SemanticBackendReprV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            repr,
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
    )
}
pub struct Fixture {
    pub types: Vec<SemanticTypeDeclV1>,
    pub functions: Vec<SemanticFunctionDeclV1>,
    pub callables: Vec<SemanticCallableDeclV1>,
}
impl Fixture {
    pub fn new(kind: SemanticGfx942InlineInstructionV30) -> Self {
        let donor = base::source(vec![base::subtract()]);
        let mut types = donor.types().to_vec();
        types.push(singleton_type(
            *types[WORD.index() as usize].layout().backend_repr(),
        ));
        let source = SemanticSourceProvenanceV1::unavailable();
        let marker_abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([12; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![value(WORD); kind.input_count()],
            value(WORD),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ByValue;
            kind.input_count()
        ])
        .unwrap();
        let callables = vec![
            SemanticCallableDeclV1::Defined {
                function: SemanticFunctionIdV1::from_index(0),
            },
            SemanticCallableDeclV1::Defined {
                function: SemanticFunctionIdV1::from_index(1),
            },
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([90; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([91; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([92; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([93; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([94; 32]),
                    source,
                    marker_abi,
                ),
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
                    SemanticGfx942InlineU32V30::new(
                        kind,
                        SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS,
                    )
                    .unwrap(),
                ),
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([95; 32]),
            },
        ];
        let mut root_locals = donor.functions()[0].locals().to_vec();
        root_locals[3] = SemanticLocalDeclV1::new(
            root_locals[3].identity(),
            SINGLETON,
            SemanticLocalRoleV1::Temporary,
            source,
        );
        let root_call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![base::copy(1), base::copy(2)],
            Some(SemanticCallDestinationV1::new(
                tuple(3),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let root = replace_body(
            &donor.functions()[0],
            root_locals,
            vec![
                block(31, vec![], SemanticTerminatorKindV1::Call(root_call)),
                block(
                    32,
                    vec![statement(
                        word(4),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(component(3))),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
        let helper_identity = SemanticFunctionIdentityV1::from_sha256([60; 32]);
        let helper_abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([11; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![value(WORD); 2],
            value(SINGLETON),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
        .unwrap();
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                source,
            )
        };
        let marker_call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            (0..kind.input_count())
                .map(|i| base::copy(i as u32 + 1))
                .collect(),
            Some(SemanticCallDestinationV1::new(
                word(3),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
        .with_inline_assembly_source_v30(
            SemanticInlineAssemblySourceV30::new([70; 32], helper_identity, [71; 32], [72; 32])
                .unwrap(),
        );
        let helper = SemanticFunctionDeclV1::new(
            helper_identity,
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([60; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([60; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([60; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([60; 32]),
            source,
            helper_abi,
            vec![
                local(61, SINGLETON, SemanticLocalRoleV1::Return),
                local(62, WORD, SemanticLocalRoleV1::Argument(0)),
                local(63, WORD, SemanticLocalRoleV1::Argument(1)),
                local(64, WORD, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(61, vec![], SemanticTerminatorKindV1::Call(marker_call)),
                block(
                    62,
                    vec![statement(
                        tuple(0),
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                vec![base::copy(3)],
                            )
                            .unwrap(),
                        ),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
        .unwrap();
        Self {
            types,
            functions: vec![root, helper],
            callables,
        }
    }
    pub fn request(self) -> InertSemanticMirRequestV1 {
        InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            self.types,
            vec![],
            vec![],
            vec![],
            self.functions,
            self.callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
    }
    pub fn admit(self) -> AdmittedInertSemanticMirV1 {
        self.request()
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap()
    }
}
