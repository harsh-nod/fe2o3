use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::{
    ProductionExecutionSourceInputV29, ProductionPendingScopedSourceOwnerV29,
    ProductionPreparedSourceV18, ProductionScopeCallableCandidateV29,
    ProductionSemanticKirLimitsV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const LIMIT: usize = 100_000_000;

fn prepared(cyclic: bool, budget: &mut Budget<'_>) -> ProductionPreparedSourceV18 {
    prepared_roster(cyclic, false, budget)
}

fn prepared_roster(
    cyclic: bool,
    unmodeled_second: bool,
    budget: &mut Budget<'_>,
) -> ProductionPreparedSourceV18 {
    let (mut types, template) = semantics::original_scalar_v30::control_fixture_v31(cyclic, false);
    let unit = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
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
    ));
    let word = SemanticTypeIdV1::from_index(0);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let mut functions = Vec::new();
    for root in 0..2u8 {
        let tag = 100 + 20 * root;
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            template
                .abi()
                .arguments()
                .iter()
                .map(|a| a.value().clone())
                .collect(),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = template.locals().to_vec();
        locals[0] = SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag + 2; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            provenance,
        );
        // Preserve each assignment and the full CFG, but keep the kernel return
        // ABI Unit. The former scalar return remains an observed temporary.
        let blocks = template
            .blocks()
            .iter()
            .enumerate()
            .map(|(ordinal, block)| {
                let statements = block
                    .statements()
                    .iter()
                    .enumerate()
                    .map(|(at, statement)| {
                        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                            return statement.clone();
                        };
                        if root == 1 && unmodeled_second && ordinal == 1 && at == 0 {
                            let SemanticRvalueKindV1::Binary { left, right, .. } =
                                assignment.value().kind()
                            else {
                                unreachable!()
                            };
                            return SemanticStatementV1::new(
                                provenance,
                                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                    assignment.destination().clone(),
                                    SemanticRvalueV1::new(
                                        word,
                                        SemanticRvalueKindV1::Binary {
                                            operation: SemanticBinaryOpV1::Add,
                                            left: left.clone(),
                                            right: right.clone(),
                                        },
                                    ),
                                )),
                            );
                        }
                        if assignment.destination().local().index() != 0 {
                            return statement.clone();
                        }
                        SemanticStatementV1::new(
                            provenance,
                            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(3),
                                    vec![],
                                    word,
                                )
                                .unwrap(),
                                assignment.value().clone(),
                            )),
                        )
                    })
                    .collect();
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([tag + 10 + ordinal as u8; 32]),
                    provenance,
                    statements,
                    block.terminator().clone(),
                )
                .unwrap()
            })
            .collect();
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag + 3; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([tag + 4; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag + 5; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 6; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 7; 32]),
                provenance,
                abi,
                locals,
                template.entry(),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(if root == 0 {
                    b"z_control".to_vec()
                } else {
                    b"a_control".to_vec()
                })
                .unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag + 9; 32]),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            )),
        );
    }
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        (0..2)
            .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i)))
            .collect(),
        (0..2).map(SemanticFunctionIdV1::from_index).collect(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let launches: Vec<_> = semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(root, function)| {
            let entry = semantic.functions()[function.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                if root == 0 { "z_control" } else { "a_control" },
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, &launches).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let semantic_sha256 = *owner.source_semantic_sha256();
    ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
        owner,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: &semantic_sha256,
            roots: &[],
            classes: &[ProductionScopeCallableCandidateV29::Ordinary; 2],
            events: &[],
        },
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

fn run(
    cyclic: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<OriginalSemanticMirRefinementSubjectV31>,
    usize,
    usize,
    usize,
) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let prepared = prepared(cyclic, &mut budget);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let request = prepare_original_semantic_mir_refinement_v31(source, budget)?;
        assert_eq!(budget.storage(), floor);
        let receipt = request.retained_storage();
        budget.reserve_storage(receipt)?;
        let subject = request.subject(budget)?;
        request.check_original_source(source, budget)?;
        let text = std::str::from_utf8(request.generated_source(budget)?).unwrap();
        let assignments = if cyclic { 8 } else { 6 };
        assert_eq!(&subject.census()[..4], &[2, 2, assignments, assignments]);
        for root in 0..2 {
            assert!(text.contains(&format!("original_mir_cfg_refines_canonical_{root}_v31")));
            assert!(text.contains(&format!("original_control_invocation_trace_{root}_v31")));
            assert!(text.contains(&format!("original_control_step_relation_{root}_v31")));
        }
        assert!(text.contains("struct OriginalControlStateV31"));
        assert!(text.contains("defined: Seq<bool>"));
        assert!(text.contains("cfg_finite_trace_refinement_v26("));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        assert!(!request.authenticates_executed_proof());
        assert!(!request.grants_artifact_or_launch_authority());
        drop(request);
        budget.release_storage(receipt)?;
        assert_eq!(budget.storage(), floor);
        Ok::<_, Error>(subject)
    });
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn original_mir_cfg_request_joins_all_diamond_roots_and_each_original_assignment() {
    let first = run(false, LIMIT, LIMIT);
    let repeated = run(false, LIMIT, LIMIT);
    assert_eq!(first.0.unwrap(), repeated.0.unwrap());
    assert_eq!((first.1, first.2, first.3), (repeated.1, 37, repeated.3));
}

#[test]
fn original_mir_cfg_request_preserves_entry_loop_invocation_and_recurrence() {
    let first = run(true, LIMIT, LIMIT);
    let repeated = run(true, LIMIT, LIMIT);
    assert_eq!(first.0.unwrap(), repeated.0.unwrap());
    assert_eq!((first.1, first.2, first.3), (repeated.1, 37, repeated.3));
}

#[test]
fn original_mir_cfg_request_has_exact_and_one_short_complete_resources() {
    for cyclic in [false, true] {
        let measured = run(cyclic, LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(cyclic, measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
        assert!(matches!(run(cyclic, measured.1 - 1, measured.3).0,
            Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(cyclic, measured.1, measured.3 - 1).0,
            Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                if error.actual() == measured.3 && error.limit() == measured.3 - 1));
    }
}

#[test]
fn original_mir_cfg_request_is_nominally_distinct_and_rejects_funded_foreign_query() {
    use std::any::TypeId;
    assert_ne!(
        TypeId::of::<PreparedOriginalSemanticMirRefinementV30<'static, 'static>>(),
        TypeId::of::<PreparedOriginalSemanticMirRefinementV31<'static, 'static>>()
    );
    assert_ne!(
        TypeId::of::<OriginalSemanticMirRefinementSubjectV30>(),
        TypeId::of::<OriginalSemanticMirRefinementSubjectV31>()
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let prepared = prepared(false, &mut budget);
    let mut refused = 0;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let request = prepare_original_semantic_mir_refinement_v31(source, budget)?;
        let retained = request.retained_storage();
        budget.reserve_storage(retained)?;
        let mut foreign_work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, LIMIT);
        foreign.reserve_storage(LIMIT / 2)?;
        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
        let error = request.subject(&foreign).unwrap_err();
        assert!(matches!(
            error,
            Error::Source(SourceError::Resource(Resource::Accounting))
        ));
        assert_eq!(
            (foreign.work(), foreign.storage(), foreign.peak_storage()),
            before
        );
        assert!(matches!(
            request.subject(budget),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        drop(request);
        // Accounting refusal denies custody credit settlement, as in V30.
        refused = budget.storage();
        Err::<(), Error>(error)
    });
    assert!(matches!(
        result,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
    assert_eq!(budget.storage(), refused);
}

#[test]
fn original_mir_cfg_request_refuses_one_unmodeled_root_without_omitting_its_control() {
    for cyclic in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(37).unwrap();
        let prepared = prepared_roster(cyclic, true, &mut budget);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let request = prepare_original_semantic_mir_refinement_v31(source, budget)?;
            drop(request);
            Ok::<(), Error>(())
        });
        assert!(matches!(
            result,
            Err(Error::Statement(
                "original MIR arithmetic/effect contract is not modeled"
            ))
        ));
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn original_mir_cfg_request_rejects_equal_bytes_from_a_foreign_source_owner() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let first = prepared(false, &mut budget);
    first
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let request = prepare_original_semantic_mir_refinement_v31(source, budget)?;
            let retained = request.retained_storage();
            budget.reserve_storage(retained)?;
            let identity = request.subject(budget)?.canonical_identity();
            let second = prepared(false, budget);
            second.with_source_consumer_v18(budget, |foreign, budget| {
                assert_eq!(*foreign.canonical(budget)?.identity(), identity);
                assert!(!std::ptr::eq(source, foreign));
                assert!(matches!(
                    request.check_original_source(foreign, budget),
                    Err(Error::Statement(
                        "original MIR request has a foreign source owner"
                    ))
                ));
                request.check_original_source(source, budget)
            })?;
            request.check_original_source(source, budget)?;
            drop(request);
            budget.release_storage(retained)?;
            Ok::<(), Error>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn original_mir_cfg_request_uses_exact_generic_field_and_result_frames() {
    type V30 = PreparedOriginalSemanticMirRefinementV30<'static, 'static>;
    type V31 = PreparedOriginalSemanticMirRefinementV31<'static, 'static>;
    type Fields = (
        &'static Source<'static>,
        CanonicalGeneratedVerusProofInputV3,
        OriginalSemanticMirRefinementSubjectV31,
        usize,
        Ledger,
        usize,
        usize,
    );
    assert_eq!(size_of::<V31>(), size_of::<Fields>());
    assert_eq!(size_of::<V30>(), size_of::<V31>());
    let queries = 3 * size_of::<&()>()
        + size_of::<OriginalSemanticMirRefinementSubjectV31>()
        + size_of::<Result<OriginalSemanticMirRefinementSubjectV31>>()
        + size_of::<Result<&[u8]>>()
        + 3 * size_of::<Result<()>>()
        + size_of::<std::result::Result<(), SourceError>>();
    assert_eq!(query_headers::<31>().unwrap(), queries);
    assert_eq!(
        original_mir_headers::<31>().unwrap(),
        original_mir_headers::<30>().unwrap()
    );
}
