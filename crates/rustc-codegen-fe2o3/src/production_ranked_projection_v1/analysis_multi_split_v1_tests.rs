#[test]
fn multiway_analysis_split_forwards_live_inductions_through_synthetic_blocks() {
    let mut blocks = vec![ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    )];
    let base_blocks = [Some(1), Some(3), Some(4), Some(5)];
    let live_inductions = [vec![0, 1], vec![0], vec![1], vec![0, 1]];
    append_analysis_multi_split_blocks_with_arguments(
        &mut blocks,
        1,
        vec![],
        &[1, 2, 3],
        &base_blocks,
        &[0, 1],
        &live_inductions,
    )
    .unwrap();

    assert_eq!(blocks.len(), 3);
    assert!(matches!(
        blocks[1].terminator(),
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_arguments,
            second_arguments,
            first_block,
            second_block,
            ..
        } if first_arguments == &[ProductionRankedValueV1::BlockArgument {
            block: 1,
            argument: 0,
        }]
            && second_arguments == &[
                ProductionRankedValueV1::BlockArgument {
                    block: 1,
                    argument: 0,
                },
                ProductionRankedValueV1::BlockArgument {
                    block: 1,
                    argument: 1,
                },
            ]
            && *first_block == 3
            && *second_block == 2
    ));
    assert!(matches!(
        blocks[2].terminator(),
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_arguments,
            second_arguments,
            first_block,
            second_block,
            ..
        } if first_arguments == &[ProductionRankedValueV1::BlockArgument {
            block: 2,
            argument: 1,
        }]
            && second_arguments == &[
                ProductionRankedValueV1::BlockArgument {
                    block: 2,
                    argument: 0,
                },
                ProductionRankedValueV1::BlockArgument {
                    block: 2,
                    argument: 1,
                },
            ]
            && *first_block == 4
            && *second_block == 5
    ));
}

#[test]
fn multiway_analysis_split_expansion_enforces_the_ranked_block_limit() {
    let exact_successors = MAX_RANKED_BOUNDS_BLOCKS / 2;
    let exact = explicit_multi_switch(exact_successors);
    let (blocks, _, _) = build_ranked_cfg(
        &projection_types(),
        &exact,
        &[],
        &[const { None }; 2],
        &vec![None; exact.blocks().len()],
        &[],
        vec![],
        (0..exact.blocks().len())
            .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
            .collect(),
    )
    .unwrap();
    assert_eq!(blocks.len(), MAX_RANKED_BOUNDS_BLOCKS);

    let oversized = explicit_multi_switch(exact_successors + 1);
    assert!(matches!(
        build_ranked_cfg(
            &projection_types(),
            &oversized,
            &[],
            &[const { None }; 2],
            &vec![None; oversized.blocks().len()],
            &[],
            vec![],
            (0..oversized.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection exceeds the ranked block limit"
        ))
    ));
}

#[test]
fn unresolved_switch_fanout_is_bounded_before_analysis_allocation() {
    let function = explicit_multi_switch(MAX_RANKED_BOUNDS_BLOCKS + 1);
    assert!(matches!(
        projected_cfg_terminator(&function, 0, &[], false, &[], &[const { None }; 2], &[],),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "analysis switch successor count exceeds the ranked block limit"
        ))
    ));
}
