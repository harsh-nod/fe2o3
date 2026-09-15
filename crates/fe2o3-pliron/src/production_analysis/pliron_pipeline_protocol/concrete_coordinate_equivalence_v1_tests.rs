use super::*;

fn literal_coordinate_source() -> String {
    let read = COMPUTED
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Read]"))
        .unwrap();
    let write = COMPUTED
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Write]"))
        .unwrap();
    let constant = "    read_coordinate_v20 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 0]: <() -> (kernel.index )>;";
    COMPUTED
        .replace(
            write,
            &write.replace("slot_v7, lane_v4", "slot_v7, zero_v2"),
        )
        .replace(
            read,
            &format!(
                "{constant}\n{}",
                read.replace("slot_v7, lane_v4", "slot_v7, read_coordinate_v20")
            ),
        )
}

fn one_block(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with("kernel.br ")
                && !["^stage():", "^consume():", "^release():"].contains(&line.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rejects_uninitialized(source: &str) {
    let (context, function) = parse(source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(
        matches!(report.findings(), [PlironPipelineProtocolFindingV1::InvalidSchedule { detail, .. }]
        if detail == "pipeline read coordinate was not initialized in the same epoch"),
        "{report:?}"
    );
    assert!(report.certificates().is_empty());
}

#[test]
fn distinct_equal_constants_initialize_reads_in_same_and_cross_block_lifecycles() {
    let source = literal_coordinate_source();
    for source in [source.clone(), one_block(&source)] {
        let (context, function) = parse(&source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert_clean(&report);
        assert_eq!(report.certificates()[0].staged_writes(), 1);
        assert_eq!(report.certificates()[0].consuming_reads(), 1);
    }
}

#[test]
fn unequal_and_unknown_coordinates_do_not_acquire_initialization() {
    let source = literal_coordinate_source();
    let unequal = source.replace("read_coordinate_v20 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 0]", "read_coordinate_v20 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 1]");
    let unknown = source
        .lines()
        .filter(|line| !line.contains("read_coordinate_v20 ="))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(
            "builtin.function <() -> ()>",
            "builtin.function <(kernel.index ) -> ()>",
        )
        .replace("^entry():", "^entry(read_coordinate_v20: kernel.index ):");
    for source in [
        unequal.clone(),
        one_block(&unequal),
        unknown.clone(),
        one_block(&unknown),
    ] {
        rejects_uninitialized(&source);
    }
}

#[test]
fn the_existing_structural_equivalence_proves_commuted_coordinates_only() {
    let write = COMPUTED
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Write]"))
        .unwrap();
    let read = COMPUTED
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Read]"))
        .unwrap();
    let expression = |name: &str, lhs: &str, rhs: &str| {
        format!(
            "    {name} = kernel.index_binary ({lhs}, {rhs}) [] [kernel_index_binary_kind: kernel.index_binary_kind Add]: <(kernel.index , kernel.index ) -> (kernel.index )>;"
        )
    };
    let source = COMPUTED
        .replace(
            write,
            &format!(
                "{}\n{}",
                expression("write_coordinate_v21", "lane_v4", "zero_v2"),
                write.replace("slot_v7, lane_v4", "slot_v7, write_coordinate_v21")
            ),
        )
        .replace(
            read,
            &format!(
                "{}\n{}",
                expression("read_coordinate_v20", "zero_v2", "lane_v4"),
                read.replace("slot_v7, lane_v4", "slot_v7, read_coordinate_v20")
            ),
        );
    let (context, function) = parse(&source);
    assert_clean(&run_pliron_pipeline_protocol_check_v1(&context, &function));
    rejects_uninitialized(&source.replace("(zero_v2, lane_v4)", "(eight_v5, lane_v4)"));
}

#[test]
fn coordinates_written_in_another_epoch_do_not_initialize_a_new_epoch() {
    let source = literal_coordinate_source();
    let event = source
        .lines()
        .find(|line| line.contains("kernel.pipeline_event") && line.contains("kind Stage]"))
        .unwrap()
        .replace(
            "(p1, epoch_v6, slot_v7)",
            "(p1, next_epoch_v40, next_slot_v41)",
        );
    let events = ["Stage", "Commit", "Wait", "Consume"]
        .map(|kind| event.replace("kind Stage]", &format!("kind {kind}]")))
        .join("\n");
    let read = source
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Read]"))
        .unwrap()
        .replace(
            "slot_v7, read_coordinate_v20",
            "next_slot_v41, read_coordinate_v20",
        );
    let tail = format!(
        "    next_epoch_v40 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 11]: <() -> (kernel.index )>;\n    next_slot_v41 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 2]: <() -> (kernel.index )>;\n{events}\n{read}\n{}",
        event.replace("kind Stage]", "kind Release]")
    );
    let source = source.replace(
        "    kernel.return ()",
        &format!("{tail}\n    kernel.return ()"),
    );
    rejects_uninitialized(&source);
}

fn replay_coordinate_queries(source: &str, queries: usize) -> (bool, bool, usize, usize) {
    // This directly qualifies the already-ordered concrete state machine and
    // its shared equivalence meter, not graph discovery or the full resource envelope.
    let (context, function) = parse(source);
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let pipeline = inventory
        .operations()
        .iter()
        .copied()
        .find(|site| Operation::get_op::<PipelineCreateOp>(site.pointer(), &context).is_some())
        .unwrap();
    let events = inventory
        .operations()
        .iter()
        .filter_map(|site| {
            let event = Operation::get_op::<PipelineEventOp>(site.pointer(), &context)?;
            Some(EventSiteV1 {
                site: *site,
                kind: event.kind(&context),
                epoch: event.epoch(&context),
                slot: event.slot(&context),
            })
        })
        .collect::<Vec<_>>();
    let accesses = inventory
        .operations()
        .iter()
        .filter_map(|site| {
            let access = Operation::get_op::<RankedAccessOp>(site.pointer(), &context)?;
            let indices = access.indices(&context);
            Some(AccessSiteV1 {
                site: *site,
                kind: access.kind(&context).unwrap(),
                slot: indices[0],
                indices,
            })
        })
        .collect::<Vec<_>>();
    let mut actions = events
        .iter()
        .map(ConcreteActionV1::Event)
        .chain(accesses.iter().map(ConcreteActionV1::Access))
        .collect::<Vec<_>>();
    actions.sort_by_key(|action| match action {
        ConcreteActionV1::Event(event) => (event.site.block(), event.site.operation()),
        ConcreteActionV1::Access(access) => (access.site.block(), access.site.operation()),
    });
    let mut analyses = PlironAnalysisManagerV1::new(&function);
    analyses.prepare_sparse_indices(&context, &function);
    let facts = analyses.sparse_indices().unwrap();
    let mut meter = EquivalenceResourceMeterV1::new(queries, 2).unwrap();
    let result =
        verify_ordered_concrete_schedule(&context, pipeline, 3, &actions, Some(facts), &mut meter);
    (
        result.is_ok(),
        meter.exhausted(),
        meter.queries,
        meter.expanded_pairs,
    )
}

#[test]
fn source_order_counts_repeated_failed_candidates_before_the_successful_match() {
    let source = literal_coordinate_source();
    let write = source
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Write]"))
        .unwrap();
    let first = write.replace("slot_v7, zero_v2", "slot_v7, first_coordinate_v21");
    let source = source.replace(write, &format!("    first_coordinate_v21 = kernel.index_constant () [] [kernel_index_value: kernel.index_value 1]: <() -> (kernel.index )>;\n{first}\n{first}\n{write}"));
    // Actual write coordinates in source order are [1,1,0], read coordinate0
    // has a distinct SSA identity. There are three queries and two unique
    // pairs. A query limit2 must reject the third attempted query; the repeated
    // failed candidate uses the memo, but still consumes its normal query.
    for _ in 0..8 {
        assert_eq!(replay_coordinate_queries(&source, 3), (true, false, 3, 2));
        assert_eq!(replay_coordinate_queries(&source, 2), (false, true, 3, 1));
    }
    let (context, function) = parse(&source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_clean(&report);
    assert_eq!(report.certificates()[0].staged_writes(), 3);
}
