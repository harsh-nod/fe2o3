use super::*;
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1, SemanticU32InductionBoundSnapshotErrorV1,
    SemanticU32InductionBoundSnapshotMeterV1, SemanticU32InductionBoundSnapshotReportV1,
    analyze_semantic_u32_induction_bound_snapshots_v1 as derive,
    analyze_semantic_u32_induction_bound_snapshots_with_meter_v1 as metered,
    analyze_semantic_u32_induction_no_overflow_reachable_with_limits_v2,
};

const LEFT: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(7);
const OTHER: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(10);

fn reachable(
    source: &AdmittedInertSemanticMirV1,
) -> fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1 {
    analyze_semantic_u32_induction_no_overflow_reachable_with_limits_v2(
        source,
        FUNCTION,
        SemanticU32InductionAnalysisLimitsV1::default(),
    )
    .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    MoveEntry,
    SecondEntryUse,
    EntryWrite,
    EntryKill,
    ExtraSnapshotUse,
    ProjectedSnapshotUse,
    DuplicateDefinition,
    StaleDefinition,
    LateDefinition,
    DifferentType,
    TransitiveCopy,
    LiveAfterDefinition,
    KillBeforeGuard,
    RestartBeforeGuard,
    MissingBodyKill,
    MissingExitKill,
    ExtraKill,
    WrongBlockKill,
    LeftKillBeforeGuard,
    SwappedGuard,
    WrongBackedge,
}

fn life(local: SemanticLocalIdV1, live: bool) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        if live {
            SemanticStatementKindV1::StorageLive(local)
        } else {
            SemanticStatementKindV1::StorageDead(local)
        },
    )
}
fn use_copy(to: SemanticLocalIdV1, from: SemanticLocalIdV1) -> SemanticStatementV1 {
    assign(
        place(to, U32),
        U32,
        SemanticRvalueKindV1::Use(copy(from, U32)),
    )
}
fn source(change: Change) -> AdmittedInertSemanticMirV1 {
    let seed = 37;
    let (types, old) = function(seed, Mutation::AliasGuardBound);
    let mut locals = old.locals().to_vec();
    let mut statements = old
        .blocks()
        .iter()
        .map(|b| b.statements().to_vec())
        .collect::<Vec<_>>();
    let mut terminators = old
        .blocks()
        .iter()
        .map(|b| b.terminator().kind().clone())
        .collect::<Vec<_>>();
    // Match the actual loop's separate left/right snapshots and header epochs.
    statements[1] = vec![
        life(LEFT, true),
        use_copy(LEFT, INDUCTION),
        life(ALIAS, true),
        use_copy(ALIAS, BOUND),
        assign(
            place(PREDICATE, BOOL),
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: SemanticOperandV1::Move(place(LEFT, U32)),
                right: SemanticOperandV1::Move(place(ALIAS, U32)),
            },
        ),
    ];
    statements[2].insert(0, life(ALIAS, false));
    statements[2].insert(0, life(LEFT, false));
    statements[4] = vec![life(LEFT, false), life(ALIAS, false)];
    match change {
        Change::None => {}
        Change::MoveEntry => {
            statements[1][3] = assign(
                place(ALIAS, U32),
                U32,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(BOUND, U32))),
            )
        }
        Change::SecondEntryUse => {
            statements[4].push(use_copy(SemanticLocalIdV1::from_index(0), BOUND))
        }
        Change::EntryWrite => statements[0].push(assign(
            place(BOUND, U32),
            U32,
            SemanticRvalueKindV1::Use(scalar_constant(U32, 9, 4)),
        )),
        Change::EntryKill => statements[4].push(life(BOUND, false)),
        Change::ExtraSnapshotUse => {
            statements[4].push(use_copy(SemanticLocalIdV1::from_index(0), ALIAS))
        }
        Change::ProjectedSnapshotUse => statements[4].push(assign(
            place(SemanticLocalIdV1::from_index(0), U32),
            U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected_place(ALIAS, U32))),
        )),
        Change::DuplicateDefinition => statements[1].insert(4, use_copy(ALIAS, BOUND)),
        Change::StaleDefinition => {
            let definition = statements[1].remove(3);
            statements[0].push(definition);
        }
        Change::LateDefinition => statements[1].swap(3, 4),
        Change::DifferentType => {
            locals[ALIAS.index() as usize] = local(seed, 25, U64, SemanticLocalRoleV1::Temporary);
            statements[1][3] = assign(
                place(ALIAS, U64),
                U64,
                SemanticRvalueKindV1::Use(copy(OTHER, U64)),
            );
            statements[1][4] = assign(
                place(PREDICATE, BOOL),
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: copy(OTHER, U64),
                    right: copy(ALIAS, U64),
                },
            );
        }
        Change::TransitiveCopy => {
            let temp = SemanticLocalIdV1::from_index(0);
            statements[1].insert(3, use_copy(temp, BOUND));
            statements[1][4] = use_copy(ALIAS, temp);
        }
        Change::LiveAfterDefinition => statements[1].swap(2, 3),
        Change::KillBeforeGuard => statements[1].insert(4, life(ALIAS, false)),
        Change::RestartBeforeGuard => {
            statements[1].insert(4, life(ALIAS, false));
            statements[1].insert(5, life(ALIAS, true));
        }
        Change::MissingBodyKill => {
            statements[2].remove(1);
        }
        Change::MissingExitKill => {
            statements[4].remove(1);
        }
        Change::ExtraKill => statements[4].push(life(ALIAS, false)),
        Change::WrongBlockKill => {
            statements[2].remove(1);
            statements[3].push(life(ALIAS, false));
        }
        Change::LeftKillBeforeGuard => statements[1].insert(4, life(LEFT, false)),
        Change::SwappedGuard => {
            statements[1][4] = assign(
                place(PREDICATE, BOOL),
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: copy(ALIAS, U32),
                    right: copy(LEFT, U32),
                },
            )
        }
        Change::WrongBackedge => {
            terminators[3] = SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4))
        }
    }
    let blocks = statements
        .into_iter()
        .zip(terminators)
        .enumerate()
        .map(|(index, (statements, term))| block(seed, 40 + index as u8, statements, term))
        .collect();
    let function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(seed, 80))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![FUNCTION],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn exact_snapshot_binds_entry_and_statement_without_legacy_conversion() {
    let source = source(Change::None);
    let report = derive(&source, FUNCTION).unwrap();
    let [fact] = report.certificates() else {
        panic!("one exact header-bound snapshot")
    };
    assert_eq!(fact.semantic_mir_sha256(), source.semantic_sha256());
    assert_eq!(fact.bound().local(), BOUND);
    assert_eq!(fact.guard_bound().local(), ALIAS);
    assert_eq!(fact.guard_induction().local(), LEFT);
    assert_eq!(fact.bound_snapshot().unwrap().statement(), 3);
    assert_eq!(fact.guard_induction_snapshot().unwrap().statement(), 1);
    assert_eq!(fact.guard().statement(), 4);
    assert_eq!(fact.bound_snapshot().unwrap().block(), fact.header());
    assert_eq!(report.checked_additions_examined(), 1);
    assert!(
        report.retained_storage() >= std::mem::size_of_val(&report) + std::mem::size_of_val(fact)
    );
    assert!(!report.grants_authority());
    assert!(!fact.authorizes_compiler_transform());
    assert!(
        analyze_semantic_u32_induction_no_overflow_v1(&source, FUNCTION)
            .unwrap()
            .certificates()
            .is_empty()
    );
    assert!(reachable(&source).certificates().is_empty());
}

#[test]
fn old_alias_guard_bound_remains_negative_but_new_family_is_explicitly_positive() {
    let source = admitted(1, Mutation::AliasGuardBound);
    let first = analyze_semantic_u32_induction_no_overflow_v1(&source, FUNCTION).unwrap();
    let reachable_report = reachable(&source);
    assert!(first.certificates().is_empty());
    assert!(reachable_report.certificates().is_empty());
    let old_bytes = InertCanonicalSemanticU32InductionEvidenceV1::from_report(&first).unwrap();
    let old_reachable_bytes =
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&reachable_report).unwrap();
    let new = derive(&source, FUNCTION).unwrap();
    assert_eq!(new.certificates().len(), 1);
    assert_eq!(new.certificates()[0].bound().local(), BOUND);
    assert_eq!(new.certificates()[0].guard_bound().local(), ALIAS);
    assert_eq!(
        first,
        analyze_semantic_u32_induction_no_overflow_v1(&source, FUNCTION).unwrap()
    );
    assert_eq!(reachable_report, reachable(&source));
    assert_eq!(
        old_bytes.canonical_bytes(),
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&first)
            .unwrap()
            .canonical_bytes()
    );
    assert_eq!(
        old_reachable_bytes.canonical_bytes(),
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&reachable_report)
            .unwrap()
            .canonical_bytes()
    );
}

#[test]
fn snapshot_recipe_uniqueness_order_entry_immutability_and_aliases_are_closed() {
    for change in [
        Change::MoveEntry,
        Change::SecondEntryUse,
        Change::EntryWrite,
        Change::EntryKill,
        Change::ExtraSnapshotUse,
        Change::ProjectedSnapshotUse,
        Change::DuplicateDefinition,
        Change::StaleDefinition,
        Change::LateDefinition,
        Change::DifferentType,
        Change::TransitiveCopy,
        Change::SwappedGuard,
        Change::WrongBackedge,
    ] {
        assert!(
            derive(&source(change), FUNCTION)
                .unwrap()
                .certificates()
                .is_empty(),
            "{change:?}"
        );
    }
}

#[test]
fn lifetime_epoch_kills_restarts_and_both_successor_closures_are_exact() {
    for change in [
        Change::LiveAfterDefinition,
        Change::KillBeforeGuard,
        Change::RestartBeforeGuard,
        Change::MissingBodyKill,
        Change::MissingExitKill,
        Change::ExtraKill,
        Change::WrongBlockKill,
        Change::LeftKillBeforeGuard,
    ] {
        assert!(
            derive(&source(change), FUNCTION)
                .unwrap()
                .certificates()
                .is_empty(),
            "{change:?}"
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denial {
    Work { attempted: usize },
    Storage { attempted: usize },
}
struct Meter {
    work: usize,
    storage: usize,
    work_limit: usize,
    storage_limit: usize,
}
impl Meter {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work: 0,
            storage: 0,
            work_limit,
            storage_limit,
        }
    }
}
impl SemanticU32InductionBoundSnapshotMeterV1 for Meter {
    type Error = Denial;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denial> {
        let attempted = self.work.checked_add(amount).unwrap();
        if attempted > self.work_limit {
            return Err(Denial::Work { attempted });
        }
        self.work = attempted;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denial> {
        let attempted = self.storage.checked_add(amount).unwrap();
        if attempted > self.storage_limit {
            return Err(Denial::Storage { attempted });
        }
        self.storage = attempted;
        Ok(())
    }
}
fn run(
    source: &AdmittedInertSemanticMirV1,
    meter: &mut Meter,
) -> Result<
    SemanticU32InductionBoundSnapshotReportV1,
    SemanticU32InductionBoundSnapshotErrorV1<Denial>,
> {
    metered(
        source,
        FUNCTION,
        SemanticU32InductionAnalysisLimitsV1::default(),
        meter,
    )
}

#[test]
fn exact_and_one_short_external_work_storage_and_local_limits_are_denials() {
    let source = source(Change::None);
    let mut complete = Meter::new(usize::MAX, usize::MAX);
    let report = run(&source, &mut complete).unwrap();
    assert_eq!(complete.work, report.work_units());
    assert!(complete.storage > report.retained_storage());
    let mut exact = Meter::new(complete.work, complete.storage);
    assert_eq!(report, run(&source, &mut exact).unwrap());
    let mut work_short = Meter::new(complete.work - 1, complete.storage);
    assert!(
        matches!(run(&source, &mut work_short), Err(SemanticU32InductionBoundSnapshotErrorV1::Meter(Denial::Work { attempted })) if attempted > work_short.work_limit)
    );
    let mut storage_short = Meter::new(complete.work, complete.storage - 1);
    assert!(
        matches!(run(&source, &mut storage_short), Err(SemanticU32InductionBoundSnapshotErrorV1::Meter(Denial::Storage { attempted })) if attempted > storage_short.storage_limit)
    );
    let mut meter = Meter::new(usize::MAX, usize::MAX);
    assert!(matches!(
        metered(
            &source,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(report.work_units() - 1, 1),
            &mut meter
        ),
        Err(SemanticU32InductionBoundSnapshotErrorV1::Analysis(
            SemanticU32InductionAnalysisErrorV1::WorkLimit { .. }
        ))
    ));
    assert!(matches!(
        metered(
            &source,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1, 0),
            &mut meter
        ),
        Err(SemanticU32InductionBoundSnapshotErrorV1::Analysis(
            SemanticU32InductionAnalysisErrorV1::CertificateLimit {
                actual: 1,
                limit: 0
            }
        ))
    ));
}

#[test]
fn source_identity_and_function_selection_cannot_be_substituted() {
    let first = admitted(1, Mutation::AliasGuardBound);
    let second = admitted(81, Mutation::AliasGuardBound);
    let left = derive(&first, FUNCTION).unwrap();
    let right = derive(&second, FUNCTION).unwrap();
    assert_ne!(left.semantic_mir_sha256(), right.semantic_mir_sha256());
    assert_ne!(
        left.certificates()[0].bound(),
        right.certificates()[0].bound()
    );
    assert!(matches!(
        derive(&first, SemanticFunctionIdV1::from_index(1)),
        Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(_))
    ));
}

#[test]
fn unchanged_direct_and_left_snapshot_shapes_use_the_new_family_without_rhs_relabeling() {
    for mutation in [Mutation::None, Mutation::AliasGuardInduction] {
        let source = admitted(7, mutation);
        let report = derive(&source, FUNCTION).unwrap();
        let [fact] = report.certificates() else {
            panic!("direct immutable entry bound")
        };
        assert_eq!(fact.bound(), fact.guard_bound());
        assert_eq!(fact.bound_snapshot(), None);
        assert_eq!(
            fact.guard_induction_snapshot().is_some(),
            matches!(mutation, Mutation::AliasGuardInduction)
        );
    }
}

#[test]
fn common_guard_checked_add_and_cfg_exclusions_are_not_widened() {
    for mutation in [
        Mutation::U64Induction,
        Mutation::SignedI32Induction,
        Mutation::TemporaryBound,
        Mutation::StaleAliasGuardInduction,
        Mutation::ProjectedCheckedInduction,
        Mutation::LessOrEqualGuard,
        Mutation::ReversedGuard,
        Mutation::StepTwo,
        Mutation::InitialOne,
        Mutation::TrueValueSwitch,
        Mutation::ExpectedOverflow,
        Mutation::WrongOverflowMessage,
        Mutation::ReachableUnwind,
        Mutation::AliasedUpdate,
        Mutation::ResetDefinition,
        Mutation::DuplicatePredicateDefinition,
        Mutation::DuplicateCheckedDefinition,
        Mutation::AlternateBodyEntry,
    ] {
        assert!(
            derive(&admitted(1, mutation), FUNCTION)
                .unwrap()
                .certificates()
                .is_empty(),
            "{mutation:?}"
        );
    }
}

#[test]
fn legacy_zero_limit_charge_and_exact_work_replay_stay_on_the_old_path() {
    for mutation in [
        Mutation::None,
        Mutation::AliasGuardInduction,
        Mutation::AliasGuardBound,
    ] {
        let source = admitted(1, mutation);
        let report = analyze_semantic_u32_induction_no_overflow_v1(&source, FUNCTION).unwrap();
        let evidence = InertCanonicalSemanticU32InductionEvidenceV1::from_report(&report).unwrap();
        assert!(matches!(
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &source,
                FUNCTION,
                SemanticU32InductionAnalysisLimitsV1::new(0, 1)
            ),
            Err(SemanticU32InductionAnalysisErrorV1::WorkLimit {
                actual: 5,
                limit: 0
            })
        ));
        let exact = analyze_semantic_u32_induction_no_overflow_with_limits_v1(
            &source,
            FUNCTION,
            SemanticU32InductionAnalysisLimitsV1::new(report.work_units(), 1),
        )
        .unwrap();
        assert_eq!(exact, report);
        assert_eq!(
            evidence.canonical_bytes(),
            InertCanonicalSemanticU32InductionEvidenceV1::from_report(&exact)
                .unwrap()
                .canonical_bytes()
        );
        assert!(matches!(
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &source,
                FUNCTION,
                SemanticU32InductionAnalysisLimitsV1::new(report.work_units() - 1, 1)
            ),
            Err(SemanticU32InductionAnalysisErrorV1::WorkLimit { .. })
        ));
    }
}
