use super::*;

fn chain_owner(
    scalars: usize,
    length: usize,
    killed: bool,
    nop: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let base = owner(scalars, if killed { 2 } else { 0 }, killed);
    let original = &base.source_semantic().functions()[0];
    let mut blocks = original.blocks().to_vec();
    // Put a second neutral chain on the actual late-write backedge as well.
    let sites = if killed {
        vec![0, blocks.len() - 1]
    } else {
        vec![0]
    };
    for site in sites {
        let next = blocks.len();
        let old = blocks[site].clone();
        blocks[site] = SemanticBasicBlockV1::new(
            old.identity(),
            old.source(),
            old.statements().to_vec(),
            SemanticTerminatorV1::new(old.source(), goto(next)),
        )
        .unwrap();
        for i in 0..length {
            let statements = if nop {
                vec![SemanticStatementV1::new(
                    original.source(),
                    SemanticStatementKindV1::Nop,
                )]
            } else {
                vec![]
            };
            blocks.push(block(
                next + i,
                statements,
                if i + 1 == length {
                    old.terminator().kind().clone()
                } else {
                    goto(next + i + 1)
                },
            ));
        }
    }
    build_owner_from_function(
        SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            original.locals().to_vec(),
            original.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone()),
    )
}

#[test]
fn neutral_owner_full_rosters_source_coordinates_and_late_cycle_are_preserved() {
    for (scalars, length, killed, nop) in [
        (0, 1, false, false),
        (16, 8, false, false),
        (128, 48, false, false),
        (16, 8, true, false),
        (16, 8, false, true),
    ] {
        let owner = chain_owner(scalars, length, killed, nop);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let body = view.body().clone();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        assert!(
            reads.observation.completed_without_exhaustion(),
            "{:?}",
            reads.observation
        );
        let expected = if killed { vec![] } else { vec![(4, 2)] };
        assert_eq!(roster(&reads, view.body()), expected);
        assert_eq!(view.body(), &body);
        assert!(!reads.contains(&body, &body.blocks()[4].statements()[0]));
        println!(
            "neutral73_owner scalars={scalars} length={length} killed={killed} nop={nop} work={} roster={expected:?}",
            MAX_WORK - reads.observation.remaining_work
        );
    }
}

fn run_with_storage(owner: &ProductionSemanticSsaOwnerV1, inherited: usize) -> (bool, usize) {
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let held = analysis.budget.reserve(inherited).unwrap();
    let result = analysis.run();
    if result.is_err() {
        assert_eq!(analysis.observation.admitted_reads, 0);
        assert!(!analysis.observation.completed);
        assert!(
            !analysis.observation.work_exhausted,
            "only a storage boundary is measured"
        );
        assert!(matches!(
            analysis.budget.failure(),
            Some(flow_failure::ResourceFailure::Storage { .. })
        ));
    } else {
        assert!(analysis.observation.completed_without_exhaustion());
        assert_eq!(result.as_ref().unwrap().len(), 1);
    }
    let work = MAX_WORK - analysis.budget.remaining;
    drop(held);
    assert!(
        analysis.budget.reserve(MAX_STORAGE).is_ok(),
        "no retained entry or scratch leaks after run"
    );
    assert_eq!(
        MAX_WORK - analysis.budget.remaining,
        work,
        "storage release must not refund work"
    );
    (result.is_ok(), work)
}

#[test]
fn neutral_owner_storage_boundary_uses_one_aggregate_ledger_and_releases_on_every_exit() {
    let owner = chain_owner(128, 48, false, false);
    assert!(run_with_storage(&owner, 0).0);
    assert!(!run_with_storage(&owner, MAX_STORAGE).0);
    let (mut low, mut high) = (0, MAX_STORAGE);
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if run_with_storage(&owner, middle).0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    assert!(run_with_storage(&owner, low).0);
    assert!(!run_with_storage(&owner, low + 1).0);
    println!("neutral73_storage max_inherited={low} first_rejected={high} limit={MAX_STORAGE}");
}

#[test]
fn neutral_owner_work_prefixes_never_publish_before_convergence() {
    let owner = chain_owner(16, 8, true, false);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let types = owner.source_semantic().types();
    let mut complete = Analysis::new(types, view);
    let expected = complete.run().unwrap();
    assert!(expected.is_empty());
    let used = MAX_WORK - complete.budget.remaining;
    for available in [0, 1, used / 2, used - 1, used] {
        let mut bounded = Analysis::new(types, view);
        bounded.budget = Budget::new(available);
        let result = bounded.run();
        if available == used {
            assert_eq!(result.unwrap(), expected);
            assert!(bounded.observation.completed_without_exhaustion());
        } else {
            assert!(result.is_err());
            assert_eq!(bounded.observation.admitted_reads, 0);
            assert!(!bounded.observation.completed);
            assert!(bounded.observation.work_exhausted);
        }
        assert!(bounded.budget.reserve(MAX_STORAGE).is_ok());
    }
}
