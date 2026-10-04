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

#[test]
fn original_mir_request_prepaid_frames_have_an_independent_field_oracle() {
    type Request = PreparedOriginalSemanticMirRefinementV30<'static, 'static>;
    type Fields = (
        &'static Source<'static>,
        CanonicalGeneratedVerusProofInputV3,
        OriginalSemanticMirRefinementSubjectV30,
        usize,
        Ledger,
        usize,
        usize,
        Option<[u8; 32]>,
    );
    assert_eq!(size_of::<Request>(), size_of::<Fields>());
    type RuntimeFields = (
        &'static [ExplicitLaunchExtent],
        FormalIndexWidth,
        EndiannessV2,
    );
    type Captured = (
        &'static Source<'static>,
        usize,
        Ledger,
        usize,
        Option<RuntimeFields>,
    );
    assert_eq!(
        size_of::<PreparationCapture<'_, '_, '_>>(),
        size_of::<Captured>()
    );
    type InventoryPair = (
        Inventory<'static>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
    );
    let queries = 3 * size_of::<&()>()
        + size_of::<OriginalSemanticMirRefinementSubjectV30>()
        + size_of::<Result<OriginalSemanticMirRefinementSubjectV30>>()
        + size_of::<Result<&[u8]>>()
        + 3 * size_of::<Result<()>>()
        + size_of::<std::result::Result<(), SourceError>>()
        + size_of::<RuntimeFields>()
        + size_of::<Sha256>()
        + size_of::<Result<[u8; 32]>>()
        + size_of::<std::slice::Iter<'_, ExplicitLaunchExtent>>()
        + size_of::<std::slice::Iter<'_, u64>>()
        + 8 * size_of::<usize>();
    assert_eq!(query_headers_v30().unwrap(), queries);
    let expected = 2 * SOURCE_LIMIT
        + queries
        + size_of::<Fields>()
        + 2 * size_of::<Result<Request>>()
        + size_of::<std::thread::Result<Result<Request>>>()
        + size_of::<Captured>()
        + align_of::<Captured>()
        + size_of::<std::panic::AssertUnwindSafe<Captured>>()
        + 3 * size_of::<Option<RuntimeFields>>()
        + size_of::<Option<[u8; 32]>>()
        + size_of::<(&Source<'_>, &mut Budget<'_>, Ledger, usize, usize)>()
        + size_of::<(
            &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        )>()
        + size_of::<(
            &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
            &mut Budget<'_>,
        )>()
        + size_of::<InventoryPair>()
        + size_of::<std::result::Result<InventoryPair, CanonicalKirInventoryErrorV1>>()
        + size_of::<Writer<'_, '_>>()
        + size_of::<Result<Writer<'_, '_>>>()
        + size_of::<(String, [usize; 6])>()
        + size_of::<Result<(String, [usize; 6])>>()
        + size_of::<Result<String>>()
        + size_of::<
            std::result::Result<
                CanonicalGeneratedVerusProofInputV3,
                GeneratedVerusProofInputErrorV3,
            >,
        >()
        + size_of::<OriginalSemanticMirRefinementSubjectV30>()
        + size_of::<Sha256>()
        + 2 * size_of::<Result<()>>();
    assert_eq!(original_mir_headers_v30().unwrap(), expected);
}

fn prepared(first: SemanticBinaryOpV1, budget: &mut Budget<'_>) -> ProductionPreparedSourceV18 {
    prepared_roster([first; 2], budget)
}

fn prepared_roster(
    operations: [SemanticBinaryOpV1; 2],
    budget: &mut Budget<'_>,
) -> ProductionPreparedSourceV18 {
    let (mut types, _) = semantics::original_scalar_v30::tests::fixture(operations[0], false);
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
        let (_, template) =
            semantics::original_scalar_v30::tests::fixture(operations[usize::from(root)], false);
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
                .map(|argument| argument.value().clone())
                .collect(),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = template.locals().to_vec();
        locals[0] = SemanticLocalDeclV1::new(
            template.locals()[0].identity(),
            unit,
            SemanticLocalRoleV1::Return,
            provenance,
        );
        let mut statements = template.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(last) = statements[1].kind() else {
            unreachable!()
        };
        statements[1] = SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], word).unwrap(),
                last.value().clone(),
            )),
        );
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
                SemanticBlockIdV1::from_index(0),
                vec![
                    SemanticBasicBlockV1::new(
                        SemanticBlockIdentityV1::from_sha256([tag + 8; 32]),
                        provenance,
                        statements,
                        SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(if root == 0 {
                    b"z_original".to_vec()
                } else {
                    b"a_original".to_vec()
                })
                .unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag + 9; 32]),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(
                            Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                            None,
                            None,
                        )
                        .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
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
                if root == 0 {
                    "z_original"
                } else {
                    "a_original"
                },
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
    first: SemanticBinaryOpV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<OriginalSemanticMirRefinementSubjectV30>,
    usize,
    usize,
    usize,
) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let prepared = prepared(first, &mut budget);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let request = prepare_original_semantic_mir_refinement_v30(source, budget)?;
        assert_eq!(budget.storage(), floor);
        let receipt = request.retained_storage();
        budget.reserve_storage(receipt)?;
        let subject = request.subject(budget)?;
        request.check_original_source(source, budget)?;
        let text = std::str::from_utf8(request.generated_source(budget)?).unwrap();
        assert_eq!(&subject.census()[..4], &[2, 2, 4, 4]);
        assert!(text.contains("original_mir_refines_canonical_0_v30"));
        assert!(text.contains("original_mir_refines_canonical_1_v30"));
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
fn original_mir_request_joins_all_real_source_roots_and_every_assignment() {
    let first = run(SemanticBinaryOpV1::BitXor, LIMIT, LIMIT);
    let second = run(SemanticBinaryOpV1::BitXor, LIMIT, LIMIT);
    let first_subject = first.0.unwrap();
    assert_eq!(first_subject, second.0.unwrap());
    assert_eq!((first.1, first.2, first.3), (second.1, 37, second.3));
    let changed = run(SemanticBinaryOpV1::BitAnd, LIMIT, LIMIT).0.unwrap();
    assert_ne!(
        changed.statement_identity(),
        first_subject.statement_identity()
    );
}

#[test]
fn original_mir_request_refuses_the_entire_unmodeled_source() {
    let result = run(SemanticBinaryOpV1::Add, LIMIT, LIMIT);
    assert!(matches!(
        result.0,
        Err(Error::Statement(
            "original MIR arithmetic/effect contract is not modeled"
        ))
    ));
    assert_eq!(result.2, 37);
}

#[test]
fn original_mir_source_request_has_exact_and_one_short_complete_resources() {
    let (result, work, floor, peak) = run(SemanticBinaryOpV1::BitXor, LIMIT, LIMIT);
    result.unwrap();
    assert_eq!(floor, 37);
    let exact = run(SemanticBinaryOpV1::BitXor, work, peak);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, 37, peak));
    for short_work in [true, false] {
        let result = run(
            SemanticBinaryOpV1::BitXor,
            work - usize::from(short_work),
            peak - usize::from(!short_work),
        );
        match result.0 {
            Err(Error::Source(SourceError::Resource(Resource::Work(error)))) if short_work => {
                assert_eq!((error.actual(), error.limit()), (work, work - 1))
            }
            Err(Error::Source(SourceError::Resource(Resource::Storage(error)))) if !short_work => {
                assert_eq!((error.actual(), error.limit()), (peak, peak - 1))
            }
            other => panic!("original source request exact resource boundary: {other:?}"),
        }
        assert_eq!(result.2, 37);
    }
}

#[test]
fn original_mir_request_rejects_funded_foreign_ledger_before_any_charge() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let prepared = prepared(SemanticBinaryOpV1::BitXor, &mut budget);
    let mut refused = 0;
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let request = prepare_original_semantic_mir_refinement_v30(source, budget)?;
        budget.reserve_storage(request.retained_storage())?;
        let mut foreign_work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, LIMIT);
        foreign.reserve_storage(budget.storage())?;
        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
        let error = request.generated_source(&foreign).unwrap_err();
        assert!(matches!(
            error,
            Error::Source(SourceError::Resource(Resource::Accounting))
        ));
        assert_eq!(
            (foreign.work(), foreign.storage(), foreign.peak_storage()),
            before
        );
        drop(request);
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
fn original_mir_request_refuses_one_unmodeled_root_without_omitting_it() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(37).unwrap();
    let source = prepared_roster(
        [SemanticBinaryOpV1::BitXor, SemanticBinaryOpV1::Add],
        &mut budget,
    );
    let result = source.with_source_consumer_v18(&mut budget, |source, budget| {
        let request = prepare_original_semantic_mir_refinement_v30(source, budget)?;
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

#[test]
fn original_mir_request_rejects_equal_bytes_from_a_different_source_owner() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let first = prepared(SemanticBinaryOpV1::BitXor, &mut budget);
    first
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let request = prepare_original_semantic_mir_refinement_v30(source, budget)?;
            let retained = request.retained_storage();
            budget.reserve_storage(retained)?;
            let identity = request.subject(budget)?.canonical_identity();
            let second = prepared(SemanticBinaryOpV1::BitXor, budget);
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
