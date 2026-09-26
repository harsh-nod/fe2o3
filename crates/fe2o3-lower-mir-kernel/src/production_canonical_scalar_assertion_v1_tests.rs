use super::*;
use ProductionCanonicalScalarAssertionDispositionV1 as Disposition;
use ProductionCanonicalScalarSourceErrorV1 as HistoryError;
use fe2o3_mir_model::semantic_mir_v1::*;

type HistoryResult<T> = Result<T, HistoryError>;

fn prepare(source: ProductionPreRankedKirOwnerV1) -> ProductionCanonicalScalarFixedPointOwnerV1 {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (owner, additional) =
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_with_assertions_v1(
            source,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(additional, owner.additional_storage());
    assert_eq!(
        owner.retained_storage_floor_v1(),
        owner.input_storage_floor_v1() + additional.retained_storage()
    );
    owner
}

fn history_run<T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalScalarAssertionPoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> HistoryResult<T>,
) -> HistoryResult<T> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_assertion_policy_checks_v1(&mut budget, callback);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}

// This is genuine admitted source-model materialization, not a rustc fixture.
// The argument remains used by the real comparison and is never deleted to
// manufacture admission. A range proof can succeed while sparse folding cannot.
fn dynamic_source(
    expected: bool,
    unknown: bool,
    second_literal: bool,
) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let boolean = SemanticTypeIdV1::from_index(1);
    let word = SemanticTypeIdV1::from_index(2);
    let p = root.source();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([232; 32]),
        SemanticLayoutIdentityV1::from_sha256([233; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            word,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let operation = if unknown {
        SemanticBinaryOpV1::LessThan
    } else if expected {
        SemanticBinaryOpV1::LessOrEqual
    } else {
        SemanticBinaryOpV1::GreaterThan
    };
    let bound = if unknown { 17 } else { u64::MAX as u128 };
    let mut blocks = vec![body(
        210,
        vec![assign(
            2,
            boolean,
            SemanticRvalueKindV1::Binary {
                operation,
                left: local(1, word),
                right: operand(word, bound, 8),
            },
        )],
        assertion(
            local(2, boolean),
            expected,
            SemanticAssertMessageV1::NullPointerDereference,
            1,
        ),
    )];
    if second_literal {
        blocks.push(body(
            211,
            vec![],
            assertion(
                operand(boolean, 1, 1),
                true,
                SemanticAssertMessageV1::NullPointerDereference,
                2,
            ),
        ));
    }
    blocks.push(body(212, vec![], SemanticTerminatorKindV1::Return));
    let function = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        p,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([240; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                p,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([241; 32]),
                word,
                SemanticLocalRoleV1::Argument(0),
                p,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([242; 32]),
                boolean,
                SemanticLocalRoleV1::Temporary,
                p,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut types = source.types().to_vec();
    types.push(word_type());
    rebuild(&seed, types, vec![function])
}

fn dead_assertion_source() -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let blocks = vec![
        body(
            214,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand(boolean, 0, 1),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        body(
            215,
            vec![],
            assertion(
                operand(boolean, 1, 1),
                true,
                SemanticAssertMessageV1::NullPointerDereference,
                2,
            ),
        ),
        body(216, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(root, root.locals().to_vec(), blocks)],
    )
}

fn payload_source() -> ProductionPreRankedKirOwnerV1 {
    let seed = dynamic_source(true, false, false);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    let word = SemanticTypeIdV1::from_index(2);
    let mut locals = root.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([243; 32]),
        boolean,
        SemanticLocalRoleV1::Temporary,
        root.source(),
    ));
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([244; 32]),
        word,
        SemanticLocalRoleV1::Temporary,
        root.source(),
    ));
    for (identity, ty) in [(245, boolean), (246, word), (247, word)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([identity; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            root.source(),
        ));
    }
    let mut entry = root.blocks()[0].statements().to_vec();
    for (index, value) in [(4, 0), (6, 3), (7, 1)] {
        entry.push(assign(
            index,
            word,
            SemanticRvalueKindV1::Use(operand(word, value, 8)),
        ));
    }
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let blocks = vec![
        body(217, entry, root.blocks()[0].terminator().kind().clone()),
        body(
            218,
            vec![assign(
                3,
                boolean,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::GreaterOrEqual,
                    left: local(1, word),
                    right: operand(word, 0, 8),
                },
            )],
            assertion(
                local(3, boolean),
                true,
                SemanticAssertMessageV1::NullPointerDereference,
                2,
            ),
        ),
        // Distinct entry/backedge values require a genuine success-edge payload.
        body(
            219,
            vec![assign(
                5,
                boolean,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: local(4, word),
                    right: local(6, word),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: local(5, boolean),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 3),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                )
                .unwrap(),
            },
        ),
        body(
            220,
            vec![assign(
                4,
                word,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: local(4, word),
                    right: local(7, word),
                },
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        body(221, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(root, locals, blocks)],
    )
}

fn multiroot_assertion_source() -> ProductionPreRankedKirOwnerV1 {
    let seed = cr_noop(&["zeta", "alpha"]);
    let boolean_seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let boolean = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    let bool_type = &boolean_seed.semantic_ssa().source_semantic().types()[1];
    let bool_layout = SemanticLayoutIdentityV1::from_sha256([204; 32]);
    assert!(types.iter().all(|ty| ty.layout_identity() != bool_layout));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            bool_layout,
            bool_type.layout().clone(),
            bool_type.shape().clone(),
        )
        .with_rustc_abi_properties(bool_type.abi_properties())
        .with_rust_type_kind(bool_type.rust_type_kind()),
    );
    let functions = source
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, function)| {
            let expected = ordinal == 0;
            copy_function(
                function,
                function.locals().to_vec(),
                vec![
                    body(
                        225 + ordinal as u8 * 2,
                        vec![],
                        assertion(
                            operand(boolean, u128::from(expected), 1),
                            expected,
                            SemanticAssertMessageV1::NullPointerDereference,
                            1,
                        ),
                    ),
                    body(
                        226 + ordinal as u8 * 2,
                        vec![],
                        SemanticTerminatorKindV1::Return,
                    ),
                ],
            )
        })
        .collect();
    rebuild(&seed, types, functions)
}

fn final_reports(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    view: &ProductionCanonicalScalarAssertionPoliciesV1<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> HistoryResult<()> {
    let original = view.original_metadata(budget)?.inventory(budget)?;
    assert!(std::ptr::eq(
        original.owner(),
        owner.original_source().executable()
    ));
    let output = view.final_inventory(budget)?;
    assert!(std::ptr::eq(output.owner(), owner.output()));
    assert!(!std::ptr::eq(original.owner(), output.owner()));
    let policies = view.policies(budget)?;
    assert!(std::ptr::eq(policies.owner(budget)?, owner.output()));
    assert_eq!(
        policies.module_function_count(budget)?,
        output.functions().len()
    );
    let definitions = policies.definition_count(budget)?;
    assert!(
        definitions > 0,
        "empty reports cannot qualify a real source owner"
    );
    for ordinal in 0..definitions {
        let coordinate = policies.definition_coordinate(ordinal, budget)?;
        assert_eq!(
            policies.history(ordinal, budget)?.function(),
            coordinate.0 as usize
        );
        let report = policies.report(ordinal, budget)?;
        assert_eq!(report.paired_stage_count(), 9);
        assert_eq!(report.reports().pass_order().len(), 9);
        assert!(report.reports().is_clean());
    }
    assert_eq!(policies.pending_obligations().iter().count(), 19);
    assert!(!view.ranked_verification_is_complete());
    assert!(!view.grants_artifact_or_launch_authority());
    Ok(())
}

fn snapshots(owner: &ProductionCanonicalScalarFixedPointOwnerV1) -> Vec<Vec<u8>> {
    let mut all = vec![
        owner
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes()
            .to_vec(),
    ];
    for round in owner.history().rounds() {
        all.push(
            round
                .integer()
                .owner()
                .canonical()
                .canonical_bytes()
                .to_vec(),
        );
        all.push(
            round
                .scalar()
                .owner()
                .canonical()
                .canonical_bytes()
                .to_vec(),
        );
    }
    all.push(owner.output().canonical().canonical_bytes().to_vec());
    all
}

#[test]
fn literal_both_polarities_have_actual_selected_success_events_and_final_reports() {
    for expected in [true, false] {
        let owner = prepare(literal(expected, false));
        assert_ne!(
            owner
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        assert!(owner.history().rounds().len() >= 2);
        history_run(&owner, |view, budget| {
            final_reports(&owner, view, budget)?;
            assert_eq!(view.assertion_count(budget)?, 1);
            let row = view.assertion(0, budget)?;
            assert_eq!(row.original_binding().expected(), expected);
            assert_eq!(row.proof_kind(), Proof::ExactRange);
            assert_eq!(row.disposition(), Disposition::HistoryElided);
            let step = row
                .selection()
                .expect("elision needs an actual checked selection");
            assert!(usize::from(step.round()) < owner.history().rounds().len());
            assert!(row.condition().is_none());
            assert!(row.failure_edge().is_none());
            assert!(row.removal().is_none());
            assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn dynamic_source_ranges_retain_real_final_conditions_for_both_polarities() {
    for expected in [true, false] {
        let owner = prepare(dynamic_source(expected, false, false));
        history_run(&owner, |view, budget| {
            final_reports(&owner, view, budget)?;
            assert_eq!(view.assertion_count(budget)?, 1);
            let row = view.assertion(0, budget)?;
            assert_eq!(row.disposition(), Disposition::Retained);
            assert_eq!(row.proof_kind(), Proof::ExactRange);
            assert!(row.selection().is_none());
            let (used, definition) = row
                .condition()
                .expect("positive must retain its real dynamic selector");
            let failure = row.failure_edge().unwrap();
            let policies = view.policies(budget)?;
            assert_eq!(policies.pair_count(budget)?, 1);
            let pair = policies.pair(0, budget)?;
            assert_eq!(pair.incoming_edges().len(), 1);
            let incoming = policies.incoming_edge(pair.incoming_edges().start, budget)?;
            assert_eq!(incoming.condition().coordinate, used);
            assert_eq!(incoming.definition().coordinate, definition);
            assert_eq!(incoming.failure().coordinate, failure);
            assert_eq!(incoming.success_when(), expected);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn complete_shared_sink_aliases_survive_mixed_retained_and_elided_history() {
    let owner = prepare(dynamic_source(true, false, true));
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 2);
        let first = view.assertion(0, budget)?;
        let second = view.assertion(1, budget)?;
        assert_ne!(first.span(), second.span());
        assert_eq!(first.disposition(), Disposition::Retained);
        assert_eq!(second.disposition(), Disposition::HistoryElided);
        assert!(second.selection().is_some());
        let policies = view.policies(budget)?;
        assert_eq!(policies.pair_count(budget)?, 1);
        assert_eq!(policies.pair(0, budget)?.incoming_edges().len(), 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn retained_shared_sink_preserves_nonempty_success_payload_occurrences() {
    let owner = prepare(payload_source());
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 2);
        for ordinal in 0..2 {
            assert_eq!(
                view.assertion(ordinal, budget)?.disposition(),
                Disposition::Retained
            );
        }
        let policies = view.policies(budget)?;
        assert_eq!(policies.pair_count(budget)?, 1);
        let incoming = policies.pair(0, budget)?.incoming_edges();
        assert_eq!(incoming.len(), 2);
        let mut nonempty = 0;
        for ordinal in incoming {
            let row = policies.incoming_edge(ordinal, budget)?;
            assert!(row.failure().arguments.is_empty());
            nonempty += usize::from(!row.success().arguments.is_empty());
        }
        assert!(
            nonempty > 0,
            "fixture must retain genuine cross-block success payload"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn every_literal_shared_sink_alias_keeps_a_distinct_original_binding() {
    let owner = prepare(two_assertions());
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 2);
        let first = view.assertion(0, budget)?;
        let second = view.assertion(1, budget)?;
        assert_ne!(
            first.original_binding().block(),
            second.original_binding().block()
        );
        for row in [first, second] {
            assert_eq!(row.disposition(), Disposition::HistoryElided);
            assert!(row.selection().is_some());
        }
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn genuine_materializer_elision_still_gets_fresh_proof_and_real_final_nine() {
    let owner = prepare(elided_owner());
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 1);
        let row = view.assertion(0, budget)?;
        assert!(matches!(
            row.original_binding().outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        ));
        assert_eq!(row.disposition(), Disposition::SourceElided);
        assert!(row.selection().is_none());
        assert!(row.removal().is_none());
        assert_eq!(row.proof_kind(), Proof::ExactRange);
        Ok(())
    })
    .unwrap();
}

#[test]
fn checked_dead_control_removal_keeps_the_source_obligation_and_event() {
    let owner = prepare(dead_assertion_source());
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 1);
        let row = view.assertion(0, budget)?;
        assert_eq!(row.proof_kind(), Proof::ExactRange);
        assert_eq!(row.disposition(), Disposition::UnreachableRemoved);
        assert!(row.removal().is_some());
        assert!(row.condition().is_none());
        assert_eq!(
            row.success_placement(),
            fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted
        );
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn repeated_consumption_preserves_every_actual_history_snapshot_and_root_symbol_order() {
    let owner = prepare(dynamic_source(true, false, true));
    let before = snapshots(&owner);
    let original_roots = owner
        .original_source()
        .executable()
        .module()
        .kernels
        .clone();
    let final_roots = owner.output().module().kernels.clone();
    let original_symbols = owner
        .original_source()
        .executable()
        .module()
        .functions
        .iter()
        .map(|f| f.id.clone())
        .collect::<Vec<_>>();
    let final_symbols = owner
        .output()
        .module()
        .functions
        .iter()
        .map(|f| f.id.clone())
        .collect::<Vec<_>>();
    for _ in 0..2 {
        history_run(&owner, |view, budget| {
            final_reports(&owner, view, budget)?;
            assert_eq!(view.assertion_count(budget)?, 2);
            Ok(())
        })
        .unwrap();
        assert_eq!(snapshots(&owner), before);
        assert_eq!(
            owner.original_source().executable().module().kernels,
            original_roots
        );
        assert_eq!(owner.output().module().kernels, final_roots);
        assert_eq!(
            owner
                .original_source()
                .executable()
                .module()
                .functions
                .iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>(),
            original_symbols
        );
        assert_eq!(
            owner
                .output()
                .module()
                .functions
                .iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>(),
            final_symbols
        );
    }
}

#[test]
fn assertion_free_multiroot_owner_still_runs_each_real_final_definition() {
    let owner = prepare(cr_noop(&["zeta", "alpha"]));
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 0);
        assert_eq!(view.policies(budget)?.definition_count(budget)?, 2);
        assert_eq!(view.policies(budget)?.pair_count(budget)?, 0);
        assert_eq!(
            view.final_inventory(budget)?.owner().module().kernels.len(),
            2
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn independent_asserting_roots_keep_complete_aliases_and_ordered_launch_roster() {
    let owner = prepare(multiroot_assertion_source());
    let roots = owner
        .original_source()
        .executable()
        .module()
        .kernels
        .clone();
    assert_eq!(roots.len(), 2);
    history_run(&owner, |view, budget| {
        final_reports(&owner, view, budget)?;
        assert_eq!(view.assertion_count(budget)?, 2);
        let first = view.assertion(0, budget)?;
        let second = view.assertion(1, budget)?;
        assert_ne!(first.span(), second.span());
        assert_ne!(
            first.original_binding().block().function,
            second.original_binding().block().function
        );
        for row in [first, second] {
            assert_eq!(row.disposition(), Disposition::HistoryElided);
            assert!(row.selection().is_some());
        }
        assert_eq!(view.policies(budget)?.definition_count(budget)?, 2);
        assert_eq!(
            view.final_inventory(budget)?.owner().module().kernels,
            roots
        );
        Ok(())
    })
    .unwrap();
}

#[cfg(test)]
mod qualification {
    include!("production_canonical_scalar_assertion_qualification_v1_tests.rs");
}

mod private_call_history {
    include!("production_canonical_private_call_v1_tests.rs");
    mod hostile {
        include!("production_canonical_private_call_hostile_v1_tests.rs");
    }
    mod differential {
        include!("production_canonical_private_call_differential_v1_tests.rs");
    }
}
