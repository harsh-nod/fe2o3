//! Inert source-shape and actual shared-adapter controls, not admitted source
//! custody. The separately compiled Rust fixture qualifies the real owner path.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const A: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const B: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const ACC: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const OTHER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);
#[derive(Clone, Copy)]
enum Case {
    Exact,
    Mutable,
    WrongPointee,
    WrongProducer,
    UnusedProducerDeclaration,
    MultipleCalls,
    WrongReceiver,
    WrongAbi,
    Wave32,
    Escape,
    Overwrite,
    ExtraHelperCall,
}
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn abi(inputs: &[SemanticTypeIdV1], output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs
            .iter()
            .map(|ty| SemanticAbiValueV1::new(*ty, SemanticAbiPassModeV1::Ignore))
            .collect(),
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn input(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}
fn call(
    callee: u32,
    args: Vec<SemanticOperandV1>,
    dest: SemanticPlaceV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            args,
            Some(SemanticCallDestinationV1::new(
                dest,
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
    i: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([i; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), kind),
    )
    .unwrap()
}
fn assignment(dest: SemanticPlaceV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = dest.ty();
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            dest,
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn function(
    i: u8,
    role: SemanticFunctionRoleV1,
    abi: SemanticFunctionAbiV1,
    local_types: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let args = abi.source_input_types().len();
    let locals = local_types
        .iter()
        .enumerate()
        .map(|(n, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([n as u8 + 100; 32]),
                *ty,
                if n == 0 {
                    SemanticLocalRoleV1::Return
                } else if n <= args {
                    SemanticLocalRoleV1::Argument(n as u32 - 1)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source(),
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([i; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([i + 1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([i + 2; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([i + 3; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([i + 4; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn intrinsic(
    i: u8,
    abi: SemanticFunctionAbiV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([i; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([i + 1; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([i + 2; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([i + 3; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([i + 4; 32]),
            source(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([i + 5; 32]),
    }
}
struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
    roots: [SemanticFunctionIdV1; 1],
}
impl Fixture {
    fn new(case: Case) -> Self {
        let shapes = [
            (SemanticTypeShapeV1::Unit, 0, 1),
            (
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
                0,
                1,
            ),
            (
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        if matches!(case, Case::WrongPointee) {
                            OTHER
                        } else {
                            CONTEXT
                        },
                        SemanticPointerKindV1::Reference,
                        if matches!(case, Case::Mutable) {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
                8,
                8,
            ),
            (SemanticTypeShapeV1::Opaque, 8, 4),
            (SemanticTypeShapeV1::Opaque, 8, 4),
            (SemanticTypeShapeV1::Opaque, 16, 4),
            (
                SemanticTypeShapeV1::Array {
                    element: F32,
                    length: 4,
                },
                16,
                4,
            ),
            (
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
                4,
                4,
            ),
            (SemanticTypeShapeV1::Unit, 0, 1),
        ];
        let types = shapes
            .into_iter()
            .enumerate()
            .map(|(i, (shape, size, align))| {
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([i as u8 + 40; 32]),
                    SemanticLayoutIdentityV1::from_sha256([i as u8 + 50; 32]),
                    SemanticTypeLayoutV1::new(Some(size), align).unwrap(),
                    shape,
                )
            })
            .collect();
        let helper_abi = abi(&[REFERENCE, A, B, ACC], ARRAY)
            .with_source_argument_ownership(vec![
                if matches!(case, Case::WrongAbi) {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                },
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ])
            .unwrap();
        let mut statements = vec![assignment(
            place(2, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(
                    if matches!(case, Case::WrongProducer) {
                        7
                    } else {
                        1
                    },
                    CONTEXT,
                ),
            },
        )];
        if matches!(case, Case::Overwrite) {
            statements.push(assignment(
                place(1, CONTEXT),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    CONTEXT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ));
        }
        let root_call = || {
            call(
                1,
                vec![input(2, REFERENCE), input(3, A), input(4, B), input(5, ACC)],
                place(6, ARRAY),
                2,
            )
        };
        let mut root_blocks = vec![
            block(
                10,
                vec![],
                if matches!(case, Case::UnusedProducerDeclaration) {
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(1),
                    ))
                } else {
                    call(0, vec![], place(1, CONTEXT), 1)
                },
            ),
            block(11, statements, root_call()),
            block(12, vec![], SemanticTerminatorKindV1::Return),
        ];
        if matches!(case, Case::MultipleCalls) {
            root_blocks.push(block(13, vec![], root_call()));
        }
        let root = function(
            20,
            SemanticFunctionRoleV1::KernelRoot,
            abi(&[], UNIT),
            &[
                UNIT, CONTEXT, REFERENCE, A, B, ACC, ARRAY, CONTEXT, REFERENCE,
            ],
            root_blocks,
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"nomination_fixture".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([25; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ));
        let extra = if matches!(case, Case::Escape) {
            vec![assignment(
                place(6, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, REFERENCE))),
            )]
        } else {
            vec![]
        };
        let values_call = || call(3, vec![input(5, ACC)], place(0, ARRAY), 2);
        let mut helper_blocks = vec![
            block(
                30,
                extra,
                call(
                    2,
                    vec![
                        input(
                            if matches!(case, Case::WrongReceiver) {
                                6
                            } else {
                                1
                            },
                            REFERENCE,
                        ),
                        input(2, A),
                        input(3, B),
                        input(4, ACC),
                    ],
                    place(5, ACC),
                    1,
                ),
            ),
            block(31, vec![], values_call()),
            block(32, vec![], SemanticTerminatorKindV1::Return),
        ];
        if matches!(case, Case::ExtraHelperCall) {
            helper_blocks.push(block(33, vec![], values_call()));
        }
        let helper = function(
            35,
            SemanticFunctionRoleV1::InternalHelper,
            helper_abi.clone(),
            &[ARRAY, REFERENCE, A, B, ACC, ACC, REFERENCE],
            helper_blocks,
        );
        let width = if matches!(case, Case::Wave32) { 32 } else { 64 };
        let contract = |role| SemanticMfmaOperandContractV1 {
            role,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: width,
        };
        let callables = vec![
            intrinsic(
                60,
                abi(&[], CONTEXT),
                SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: CONTEXT },
            ),
            SemanticCallableDeclV1::defined(HELPER),
            intrinsic(
                70,
                abi(&[REFERENCE, A, B, ACC], ACC),
                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                    context: CONTEXT,
                    lhs_fragment: A,
                    rhs_fragment: B,
                    accumulator_fragment: ACC,
                    lhs: contract(SemanticMfmaOperandRoleV1::A),
                    rhs: contract(SemanticMfmaOperandRoleV1::B),
                    accumulator: SemanticMfmaAccumulatorContractV1 {
                        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
                        wave_width: width,
                        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
                    },
                },
            ),
            intrinsic(
                80,
                abi(&[ACC], ARRAY),
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                    fragment: ACC,
                    values: ARRAY,
                },
            ),
        ];
        Self {
            types,
            functions: vec![root, helper],
            callables,
            roots: [SemanticFunctionIdV1::from_index(0)],
        }
    }
    fn view(&self) -> Source<'_> {
        Source {
            types: &self.types,
            functions: &self.functions,
            callables: &self.callables,
            roots: &self.roots,
        }
    }
    fn run(
        &self,
        limits: ProductionSemanticSsaLimitsV1,
        summary: &mut ProductionSemanticSsaSummaryV1,
    ) -> Result<bool, Error> {
        let mut meter = Meter { limits, summary };
        // Same production prefix; only Source construction is test-owned inert fixture data.
        meter.work(128)?;
        meter.storage(SCRATCH_WORDS)?;
        nominate(&self.view(), HELPER, 1, REFERENCE, &mut meter)
    }
}
fn accepts(f: &Fixture) -> bool {
    f.run(
        ProductionSemanticSsaLimitsV1::default(),
        &mut ProductionSemanticSsaSummaryV1::default(),
    )
    .unwrap()
}
#[test]
fn actual_source_occurrences_and_exact_reference_join_nominate() {
    assert!(accepts(&Fixture::new(Case::Exact)));
}
#[test]
fn actual_function_roles_do_not_assume_root_ordinal_zero() {
    let mut f = Fixture::new(Case::Exact);
    f.functions.swap(0, 1);
    f.roots[0] = SemanticFunctionIdV1::from_index(1);
    f.callables[1] = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0));
    let mut summary = ProductionSemanticSsaSummaryV1::default();
    let mut meter = Meter {
        limits: ProductionSemanticSsaLimitsV1::default(),
        summary: &mut summary,
    };
    meter.work(128).unwrap();
    meter.storage(SCRATCH_WORDS).unwrap();
    assert!(
        nominate(
            &f.view(),
            SemanticFunctionIdV1::from_index(0),
            1,
            REFERENCE,
            &mut meter
        )
        .unwrap()
    );
}
#[test]
fn mutable_or_different_pointee_never_get_blanket_ordinary_transparency() {
    assert!(accepts(&Fixture::new(Case::Exact)));
    for case in [Case::Mutable, Case::WrongPointee, Case::WrongAbi] {
        assert!(!accepts(&Fixture::new(case)));
    }
}
#[test]
fn declaration_presence_cannot_replace_actual_producer_or_borrow_source() {
    assert!(accepts(&Fixture::new(Case::Exact)));
    for case in [
        Case::UnusedProducerDeclaration,
        Case::WrongProducer,
        Case::Overwrite,
    ] {
        assert!(!accepts(&Fixture::new(case)));
    }
}
#[test]
fn extra_calls_wrong_receiver_and_wave32_are_not_nominated() {
    assert!(accepts(&Fixture::new(Case::Exact)));
    for case in [
        Case::MultipleCalls,
        Case::WrongReceiver,
        Case::Wave32,
        Case::ExtraHelperCall,
    ] {
        assert!(!accepts(&Fixture::new(case)));
    }
}
fn parameter() -> NominalReferenceParameterV29 {
    NominalReferenceParameterV29 {
        ordinal: 0,
        local: 1,
        reference_type: REFERENCE,
        pointee: CONTEXT,
        closed: false,
    }
}
#[test]
fn nomination_is_not_closure_actual_shared_adapter_still_rejects_escape() {
    for (case, closed_expected) in [(Case::Exact, true), (Case::Escape, false)] {
        let f = Fixture::new(case);
        assert!(accepts(&f));
        let (_, closed) = adapter::analyze_borrow_uses_v29(
            &f.functions[1],
            &f.callables,
            &[Some(parameter()), None, None, None],
            None,
        );
        assert_eq!(closed.contains(&0), closed_expected);
    }
}
#[test]
fn root_context_promotes_only_after_actual_helper_use_closes() {
    let f = Fixture::new(Case::Exact);
    assert!(accepts(&f));
    let old = adapter::transparent_borrow_sites_v1(&f.functions[0], &f.callables);
    assert!(old.is_empty());
    let (_, closed) = adapter::analyze_borrow_uses_v29(
        &f.functions[1],
        &f.callables,
        &[Some(parameter()), None, None, None],
        None,
    );
    assert!(closed.contains(&0));
    let mut p = parameter();
    p.closed = true;
    let effects = NominalReferenceEffectsV29 {
        parameters: vec![vec![], vec![Some(p), None, None, None]],
        parameter_count: 1,
        scratch_peak: 0,
    };
    let (sites, _) =
        adapter::analyze_borrow_uses_v29(&f.functions[0], &f.callables, &[], Some(&effects));
    assert_eq!(sites.len(), 1);
    let (before, _, _) = adapter::semantic_function_ssa_input_v1(
        &f.functions[0],
        Some(&f.types),
        &f.callables,
        &old,
    );
    let (after, _, _) = adapter::semantic_function_ssa_input_v1(
        &f.functions[0],
        Some(&f.types),
        &f.callables,
        &sites,
    );
    assert!(!before.promotable()[1]);
    assert!(after.promotable()[1]);
}
fn limits(storage: usize, work: usize) -> ProductionSemanticSsaLimitsV1 {
    let p = ProductionSemanticSsaModuleLimitsV1::production();
    ProductionSemanticSsaLimitsV1::with_module_limits(
        SsaPlannerLimitsV1::default(),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            p.max_variables(),
            p.max_blocks(),
            p.max_edges(),
            p.max_events(),
            p.max_edge_definitions(),
            p.max_output_items(),
            storage,
            work,
        )
        .unwrap(),
    )
}
#[test]
fn exact_and_one_short_resources_retain_original_prefix() {
    let f = Fixture::new(Case::Exact);
    let mut measured = ProductionSemanticSsaSummaryV1::default();
    assert!(
        f.run(ProductionSemanticSsaLimitsV1::default(), &mut measured)
            .unwrap()
    );
    let (storage, work) = (measured.storage_words, measured.work_units);
    assert_eq!(storage, SCRATCH_WORDS);
    for (s, w, pass) in [
        (storage, work, true),
        (storage - 1, work, false),
        (storage, work - 1, false),
    ] {
        let mut summary = ProductionSemanticSsaSummaryV1 {
            storage_words: 9,
            work_units: 7,
            ..Default::default()
        };
        let result = f.run(limits(s + 9, w + 7), &mut summary);
        if pass {
            assert!(result.unwrap());
            assert_eq!(summary.storage_words, storage + 9);
            assert_eq!(summary.work_units, work + 7);
        } else {
            assert!(result.is_err());
            assert!(summary.storage_words >= 9);
            assert!(summary.work_units >= 7);
        }
    }
}
#[test]
fn finite_block_cap_refuses_without_scanning_extra_source_body() {
    let mut f = Fixture::new(Case::Exact);
    let old = f.functions[0].clone();
    let mut blocks = old.blocks().to_vec();
    while blocks.len() <= BLOCKS {
        blocks.push(block(90, vec![], SemanticTerminatorKindV1::Return));
    }
    f.functions[0] = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    assert!(!accepts(&f));
}
