use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{
    SemanticU32InductionBoundSnapshotCertificateV1 as Snapshot,
    analyze_semantic_u32_induction_bound_snapshots_v1,
};

// Constructed source fixture only: the ordinary D route never rewrites its MIR.
// Reuse the original typed declaration/ABI fixture, add an explicit RHS epoch,
// then create the one source/SSA owner consumed by the actual materializer.
pub(in super::super) fn snapshot_source(
    seed: u8,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
    Snapshot,
) {
    snapshot_source_with_lhs_move(seed, false)
}

fn snapshot_source_with_lhs_move(
    seed: u8,
    lhs_move: bool,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
    Snapshot,
) {
    let (base, _, _) = fixture::source(seed);
    let original = base.source_semantic();
    let declaration = &original.functions()[0];
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let snapshot = SemanticLocalIdV1::from_index(5);
    let place = |local: u32| {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], u32_ty).unwrap()
    };
    let statement =
        |kind| SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind);
    let mut locals = declaration.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([55; 32]),
        u32_ty,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    if lhs_move {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([56; 32]),
            u32_ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let mut blocks = Vec::new();
    for (index, block) in declaration.blocks().iter().enumerate() {
        let statements = match index {
            1 => {
                let mut statements = vec![
                    statement(SemanticStatementKindV1::StorageLive(snapshot)),
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(5),
                        SemanticRvalueV1::new(
                            u32_ty,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1))),
                        ),
                    ))),
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], bool_ty)
                            .unwrap(),
                        SemanticRvalueV1::new(
                            bool_ty,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: if lhs_move {
                                    SemanticOperandV1::Move(place(6))
                                } else {
                                    SemanticOperandV1::Copy(place(2))
                                },
                                right: SemanticOperandV1::Move(place(5)),
                            },
                        ),
                    ))),
                ];
                if lhs_move {
                    statements.insert(
                        2,
                        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(6),
                            SemanticRvalueV1::new(
                                u32_ty,
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2))),
                            ),
                        ))),
                    );
                }
                statements
            }
            2 | 4 => {
                let mut statements =
                    vec![statement(SemanticStatementKindV1::StorageDead(snapshot))];
                statements.extend_from_slice(block.statements());
                statements
            }
            _ => block.statements().to_vec(),
        };
        blocks.push(
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap(),
        );
    }
    let function = SemanticFunctionDeclV1::new(
        declaration.identity(),
        declaration.role(),
        declaration.item_definition_identity(),
        declaration.monomorphization_identity(),
        declaration.generic_type_arguments_identity(),
        declaration.const_generic_arguments_identity(),
        declaration.source(),
        declaration.abi().clone(),
        locals,
        declaration.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(declaration.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        original.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    drop(base);
    assert!(
        fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(&admitted, ROOT)
            .unwrap()
            .certificates()
            .is_empty()
    );
    let report = analyze_semantic_u32_induction_bound_snapshots_v1(&admitted, ROOT).unwrap();
    let [certificate] = report.certificates() else {
        panic!("new-family snapshot source prerequisite")
    };
    let certificate = *certificate;
    let semantic = ProductionSemanticMirOwnerV1::try_new(admitted, Default::default()).unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(semantic, Default::default()).unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "component",
            [seed; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch, certificate)
}
fn materialize_snapshot(
    seed: u8,
    budget: &mut Budget<'_>,
) -> (ProductionScalarSsaEmissionOwnerV1, Snapshot) {
    let (ssa, launch, certificate) = snapshot_source(seed);
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        budget,
    )
    .unwrap();
    (owner, certificate)
}
fn run<'s>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    certificate: &Snapshot,
    budget: &mut Budget<'_>,
) -> Result<ProductionU32BoundSnapshotRecurrenceV1<'s>> {
    owner.with_u32_recurrences_v1(Default::default(), budget, |query, budget| {
        query.check_u32_bound_snapshot_certificate_v1(ROOT, certificate, budget)
    })
}
fn exact(
    value: ProductionU32BoundSnapshotRecurrenceV1<'_>,
) -> ProductionU32BoundSnapshotRecurrenceFactV1<'_> {
    match value {
        ProductionU32BoundSnapshotRecurrenceV1::Joined(fact) => fact,
        other => panic!("exact snapshot join: {other:?}"),
    }
}
fn resource(error: &Error) -> Option<Resource> {
    match error {
        Error::Resource(error)
        | Error::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error))
        | Error::Loops(CanonicalKirLoopErrorV1::Resource(error)) => Some(*error),
        _ => None,
    }
}

#[test]
fn actual_copy_statement_entry_input_and_n_recurrence_join_without_relabeling() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, certificate) = materialize_snapshot(30, &mut budget);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    let fact = exact(run(&owner, &certificate, &mut budget).unwrap());
    assert!(std::ptr::eq(fact.source(), owner.original()));
    assert_eq!(fact.certificate(), certificate);
    assert_ne!(certificate.guard_bound(), certificate.bound());
    assert_eq!(certificate.bound_snapshot().unwrap().statement(), 1);
    assert_eq!(fact.recurrence().scalar(), ScalarType::U32);
    assert!(!fact.authorizes_compiler_transform());
    let rows = &owner.emission.capture.expected;
    assert!(
        rows.iter()
            .any(|row| row.variable == SsaVariableIdV1::new(5)
                && row.site
                    == SourceSite::Event(Site::Statement {
                        block: SsaBlockIdV1::new(1),
                        statement: 1
                    }))
    );
    assert!(
        rows.iter()
            .any(|row| row.variable == SsaVariableIdV1::new(1) && row.site == SourceSite::Entry)
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn direct_legacy_and_new_family_use_the_same_actual_recurrence_without_changing_legacy_fields() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, legacy) = materialize(&mut budget);
    let source = owner.original().semantic_ssa().source_semantic();
    let report = analyze_semantic_u32_induction_bound_snapshots_v1(source, ROOT).unwrap();
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1() + report.retained_storage())
        .unwrap();
    let old = joined(query(&owner, &legacy, &mut budget).unwrap());
    let new = exact(run(&owner, &report.certificates()[0], &mut budget).unwrap());
    assert_eq!(old.recurrence(), new.recurrence());
    assert_eq!(old.certificate(), legacy);
    assert_eq!(new.certificate().bound_snapshot(), None);
}

#[test]
fn ordered_rhs_resolution_preserves_the_legacy_operand_zero_contract() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize_snapshot(30, &mut budget);
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    let lhs = owner
        .emission
        .first_operand(
            owner.original(),
            certificate.function(),
            certificate.guard(),
            SsaVariableIdV1::new(certificate.guard_induction().local().index()),
            &mut budget,
        )
        .unwrap();
    assert!(matches!(lhs, SsaValueV1::BlockArgument { .. }));
    assert!(matches!(
        owner.emission.first_operand(
            owner.original(),
            certificate.function(),
            certificate.guard(),
            SsaVariableIdV1::new(certificate.guard_bound().local().index()),
            &mut budget
        ),
        Err(Error::Mismatch("source operand SSA resolution"))
    ));
    exact(run(&owner, &certificate, &mut budget).unwrap());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn ordered_rhs_resolution_checks_the_real_lhs_snapshot_move_kill() {
    let (ssa, launch, certificate) = snapshot_source_with_lhs_move(30, true);
    assert!(certificate.guard_induction_snapshot().is_some());
    assert!(certificate.bound_snapshot().is_some());
    assert_ne!(certificate.guard_induction(), certificate.induction());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    let fact = exact(run(&owner, &certificate, &mut budget).unwrap());
    assert_eq!(fact.certificate(), certificate);
    assert!(std::ptr::eq(fact.source(), owner.original()));
    let rows = owner
        .original()
        .semantic_ssa()
        .occurrences_v1()
        .unwrap()
        .function(ROOT)
        .unwrap();
    let site = Site::Statement {
        block: SsaBlockIdV1::new(certificate.guard().block().block().index()),
        statement: certificate.guard().statement(),
    };
    let guard_rows: Vec<_> = rows
        .events()
        .iter()
        .filter(|row| row.site() == site)
        .collect();
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    assert_eq!(
        (guard_rows[0].operand(), guard_rows[0].role()),
        (Operand::RvalueOperand(0), Role::BaseUse)
    );
    assert_eq!(
        (guard_rows[1].operand(), guard_rows[1].role()),
        (Operand::RvalueOperand(0), Role::MoveKill)
    );
    assert_eq!(
        (guard_rows[2].operand(), guard_rows[2].role()),
        (Operand::RvalueOperand(1), Role::BaseUse)
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn foreign_new_family_fact_and_root_fail_with_clean_resource_floor() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize_snapshot(30, &mut budget);
    let (_, _, foreign) = snapshot_source(31);
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        run(&owner, &foreign, &mut budget),
        Err(Error::Mismatch("certificate actual source identity"))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(
        owner
            .with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| query
                .check_u32_bound_snapshot_certificate_v1(
                    SemanticFunctionIdV1::from_index(9),
                    &certificate,
                    budget
                ))
            .is_err()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn relabeled_snapshot_entry_or_changed_native_copy_cannot_pass_replay() {
    for change in 0..3 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (mut owner, certificate) = materialize_snapshot(30, &mut budget);
        let expected = owner
            .emission
            .capture
            .expected
            .iter()
            .position(|row| {
                row.variable == SsaVariableIdV1::new(5)
                    && matches!(
                        row.site,
                        SourceSite::Event(Site::Statement { statement: 1, .. })
                    )
            })
            .unwrap();
        match change {
            0 => owner.emission.capture.expected[expected].site = SourceSite::Entry,
            1 => owner.emission.capture.expected[expected].variable = SsaVariableIdV1::new(1),
            _ => {
                let definition = owner
                    .emission
                    .capture
                    .definitions
                    .iter_mut()
                    .find(|row| row.expected == expected)
                    .unwrap();
                definition.definitions[0] = None;
            }
        }
        budget
            .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
            .unwrap();
        let floor = budget.storage();
        assert!(run(&owner, &certificate, &mut budget).is_err(), "{change}");
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn new_query_error_is_sticky_and_foreign_budget_is_not_charged() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, certificate) = materialize_snapshot(30, &mut budget);
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    assert!(
        owner
            .with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| {
                assert!(
                    query
                        .check_u32_bound_snapshot_certificate_v1(
                            SemanticFunctionIdV1::from_index(9),
                            &certificate,
                            budget
                        )
                        .is_err()
                );
                let before = budget.work();
                assert!(
                    query
                        .check_u32_bound_snapshot_certificate_v1(ROOT, &certificate, budget)
                        .is_err()
                );
                assert_eq!(budget.work(), before);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(budget.storage(), floor);
    let mut foreign_work = Work::new(WORK);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    foreign.reserve_storage(floor + 4096).unwrap();
    let foreign_floor = foreign.storage();
    let mut scoped = 0;
    assert!(
        owner
            .with_u32_recurrences_v1(Default::default(), &mut budget, |query, current| {
                scoped = current.storage() - floor;
                assert!(matches!(
                    query.check_u32_bound_snapshot_certificate_v1(ROOT, &certificate, &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                Ok(())
            })
            .is_err()
    );
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), foreign_floor);
    assert_eq!(budget.storage(), floor + scoped);
    budget.release_storage(scoped).unwrap();
}

#[test]
fn new_query_exact_and_one_short_work_and_storage_preserve_floor() {
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let (owner, certificate) = materialize_snapshot(30, &mut setup);
    let execute = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = FLOOR + owner.retained_analysis_storage_v1();
        budget.reserve_storage(floor).unwrap();
        let result = run(&owner, &certificate, &mut budget).map(|value| exact(value).recurrence());
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, used, peak) = execute(WORK, STORAGE);
    let recurrence = result.unwrap();
    assert_eq!(execute(used, peak).0.unwrap(), recurrence);
    assert!(matches!(
        resource(&execute(used - 1, peak).0.unwrap_err()),
        Some(Resource::Work(_))
    ));
    assert!(matches!(
        resource(&execute(used, peak - 1).0.unwrap_err()),
        Some(Resource::Storage(_))
    ));
}
