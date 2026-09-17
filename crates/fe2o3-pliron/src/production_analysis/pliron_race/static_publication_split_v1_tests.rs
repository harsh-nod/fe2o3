fn publication_post_split(control_dependencies: Vec<V>) -> Vec<ProductionRankedBlockV1> {
    let mut result = blocks();
    for block in &mut result[1..3] {
        *block = ProductionRankedBlockV1::new(block.operations().to_vec(), T::Branch { target: 3 });
    }
    result.extend([
        ProductionRankedBlockV1::new(
            vec![],
            T::AnalysisSplit {
                control_dependencies,
                first_block: 4,
                second_block: 5,
            },
        ),
        ProductionRankedBlockV1::new(vec![], T::Return),
        ProductionRankedBlockV1::new(vec![], T::Trap),
    ]);
    result
}

fn publication_pre_split() -> Vec<ProductionRankedBlockV1> {
    let mut result = blocks();
    let role = result[0].terminator().clone();
    result[0] = ProductionRankedBlockV1::new(
        result[0].operations().to_vec(),
        T::AnalysisSplit {
            control_dependencies: vec![local(0)],
            first_block: 3,
            second_block: 4,
        },
    );
    result.extend([
        ProductionRankedBlockV1::new(vec![], T::Branch { target: 5 }),
        ProductionRankedBlockV1::new(vec![], T::Branch { target: 5 }),
        ProductionRankedBlockV1::new(vec![], role),
    ]);
    result
}

fn assert_publication_split_rejected(blocks: Vec<ProductionRankedBlockV1>) {
    let error = match compile(blocks) {
        Ok(_) => panic!("unsafe split must not acquire publication authority"),
        Err(error) => error,
    };
    let error = match *error {
        crate::ProductionRankedCompileErrorV1::Session(
            crate::ProductionSessionErrorV1::RankedRace(error),
        ) => error,
        other => panic!("mutant did not reach the publication race proof: {other:?}"),
    };
    assert!(
        error
            .report()
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedRaceFindingV1::HappensBeforeIncomplete { .. }))
    );
    assert!(error.report().static_publication().is_none());
}

#[test]
fn static_publication_unknown_post_split_retains_exact_protocol_sites() {
    for controls in [vec![], vec![local(0)], vec![local(1), local(2)]] {
        let owner = compile(publication_post_split(controls)).unwrap();
        let proof = owner.race_report().static_publication().unwrap();
        assert_eq!(proof.maximum_invocations(), 256);
        assert_eq!(proof.discharged_cell_pairs(), 128);
        assert_eq!(proof.unresolved_cell_pairs(), 0);
        assert_eq!(
            proof.sites().map(|site| (site.block(), site.operation())),
            [(1, 0), (1, 1), (2, 0), (2, 1), (2, 2), (2, 3)]
        );
    }
}

#[test]
fn static_publication_unknown_pre_split_preserves_both_role_paths() {
    let owner = compile(publication_pre_split()).unwrap();
    assert!(owner.race_report().is_clean());
    assert_eq!(
        owner
            .race_report()
            .static_publication()
            .unwrap()
            .discharged_cell_pairs(),
        128
    );
    for role in [1, 2] {
        let mut source = publication_pre_split();
        source[3] = ProductionRankedBlockV1::new(vec![], T::Branch { target: role });
        assert_publication_split_rejected(source);
    }
}

#[test]
fn static_publication_unknown_split_cannot_repeat_or_add_protocol_effects() {
    for target in [1, 2, 3] {
        let mut source = publication_post_split(vec![]);
        source.pop();
        source[3] = ProductionRankedBlockV1::new(
            vec![],
            T::AnalysisSplit {
                control_dependencies: vec![],
                first_block: 4,
                second_block: target,
            },
        );
        assert_publication_split_rejected(source);
    }
    let mut source = publication_post_split(vec![]);
    source[5] = ProductionRankedBlockV1::new(
        vec![O::Access {
            kind: AccessKindAttr::Read,
            view: local(3),
            indices: vec![local(2)],
        }],
        T::Return,
    );
    assert_publication_split_rejected(source);
}
