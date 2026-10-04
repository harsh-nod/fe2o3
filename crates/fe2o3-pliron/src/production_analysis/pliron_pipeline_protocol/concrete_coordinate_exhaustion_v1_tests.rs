use super::*;

fn constants() -> (Context, [Value; 3]) {
    let (mut context, _) = parse(&literal_coordinate_source());
    let values = [1, 0, 0].map(|value| IndexConstantOp::new(&mut context, value).result(&context));
    (context, values)
}

fn counters(meter: &EquivalenceResourceMeterV1) -> (usize, usize, usize, usize) {
    (
        meter.queries,
        meter.expanded_pairs,
        meter.cursor_steps,
        meter.memo.len(),
    )
}

#[test]
fn concrete_query_denial_stops_before_later_identity_write() {
    let (context, [one, zero, read]) = constants();
    let rows = [[one], [zero], [read]];
    let writes = rows.iter().map(|row| row.as_slice()).collect::<Vec<_>>();
    let mut meter = EquivalenceResourceMeterV1::new(1, 8).unwrap();
    assert!(!concrete_coordinate_initialized_v1(
        &context,
        &writes,
        &[read],
        &mut meter
    ));
    assert!(meter.exhausted());
    // Unequal first pair: one query/cursor/expansion/memo entry. The second
    // query is denied before any cursor or expansion; do not visit row three.
    assert_eq!(counters(&meter), (2, 1, 1, 1));
    assert!(!concrete_coordinate_initialized_v1(
        &context,
        &writes,
        &[read],
        &mut meter
    ));
    assert_eq!(counters(&meter), (2, 1, 1, 1));
}

#[test]
fn concrete_pair_denial_stops_before_later_identity_write() {
    let (context, [one, zero, read]) = constants();
    let rows = [[one], [zero], [read]];
    let writes = rows.iter().map(|row| row.as_slice()).collect::<Vec<_>>();
    let mut meter = EquivalenceResourceMeterV1::new(16, 1).unwrap();
    assert!(!concrete_coordinate_initialized_v1(
        &context,
        &writes,
        &[read],
        &mut meter
    ));
    assert!(meter.exhausted());
    // The second query/cursor is admitted, then unique expansion two is denied
    // before memo insertion. A later identical Value cannot repair exhaustion.
    assert_eq!(counters(&meter), (2, 2, 2, 1));
    assert!(!concrete_coordinate_initialized_v1(
        &context,
        &writes,
        &[read],
        &mut meter
    ));
    assert_eq!(counters(&meter), (2, 2, 2, 1));
}

#[test]
fn concrete_exhausted_empty_and_identity_cases_do_no_further_work() {
    let (context, [one, zero, _]) = constants();
    let empty: &[Value] = &[];
    let identity = [zero];
    for (writes, coordinate) in [
        (vec![], empty),
        (vec![empty], empty),
        (vec![identity.as_slice()], identity.as_slice()),
    ] {
        let mut meter = EquivalenceResourceMeterV1::new(0, 1).unwrap();
        assert!(!index_values_equivalent(&context, one, zero, &mut meter));
        assert_eq!(counters(&meter), (1, 0, 0, 0));
        assert!(!concrete_coordinate_initialized_v1(
            &context, &writes, coordinate, &mut meter
        ));
        assert_eq!(counters(&meter), (1, 0, 0, 0));
    }
    let mut healthy = EquivalenceResourceMeterV1::new(0, 0).unwrap();
    assert!(concrete_coordinate_initialized_v1(
        &context,
        &[empty],
        empty,
        &mut healthy
    ));
    assert!(!concrete_coordinate_initialized_v1(
        &context,
        &[],
        empty,
        &mut healthy
    ));
    assert!(!healthy.exhausted());
    assert_eq!(counters(&healthy), (0, 0, 0, 0));
}

fn two_pipeline_source(depth: usize) -> String {
    let source = literal_coordinate_source();
    let write = source
        .lines()
        .find(|line| line.contains("kernel.access") && line.contains("kind Write]"))
        .unwrap();
    let mut casts = String::new();
    let mut left = "zero_v2".to_owned();
    let mut right = "zero_v2".to_owned();
    for level in 0..depth {
        let next_left = format!("left_coordinate_{level}");
        let next_right = format!("right_coordinate_{level}");
        for (name, input) in [(&next_left, &left), (&next_right, &right)] {
            casts.push_str(&format!(
                "    {name} = kernel.index_unsigned_cast ({input}) [] [kernel_unsigned_bit_width: kernel.index_value 32]: <(kernel.index ) -> (kernel.index )>;\n"
            ));
        }
        left = next_left;
        right = next_right;
    }
    let first_write = write.replace("slot_v7, zero_v2", &format!("slot_v7, {left}"));
    let later_identity_write = write.replace("slot_v7, zero_v2", &format!("slot_v7, {right}"));
    let source = source
        .replace(
            write,
            &format!("{casts}{first_write}\n{later_identity_write}"),
        )
        .replace("slot_v7, read_coordinate_v20", &format!("slot_v7, {right}"));

    // The earlier, disjoint owner has a literal access-free lifecycle. Its
    // certificate must be discarded when the second owner's query is denied.
    let view = source
        .lines()
        .find(|line| line.contains("v0 = kernel.ranked_view"))
        .unwrap();
    let first_view = view
        .replace("v0 =", "first_view_v10 =")
        .replace(
            "kernel.allocation_origin 301",
            "kernel.allocation_origin 302",
        )
        .replace("kernel.noalias_class 41", "kernel.noalias_class 42");
    let create = source
        .lines()
        .find(|line| line.contains("p1 = kernel.pipeline_create"))
        .unwrap();
    let first_create = create
        .replace("p1 =", "first_pipeline_v11 =")
        .replace("(v0)", "(first_view_v10)");
    let event = source
        .lines()
        .find(|line| line.contains("kernel.pipeline_event") && line.contains(" Stage]"))
        .unwrap()
        .replace(
            "(p1, epoch_v6, slot_v7)",
            "(first_pipeline_v11, zero_v2, zero_v2)",
        );
    let first_schedule = ["Stage", "Commit", "Wait", "Consume", "Release"]
        .map(|kind| event.replace("kind Stage]", &format!("kind {kind}]")))
        .join("\n");
    source
        .replace(view, &format!("{first_view}\n{first_create}\n{view}"))
        .replacen(
            "    kernel.br () [^stage]",
            &format!("{first_schedule}\n    kernel.br () [^stage]"),
            1,
        )
}

#[test]
fn actual_protocol_discards_prior_certificate_on_deep_coordinate_exhaustion() {
    // Each paired cast consumes one unique visit. The common root is the
    // identical zero Value and needs no expansion. Depth256 exactly fits the
    // fixed per-query limit; depth257 fails before expanding pair257. The
    // second write is identical to the read but cannot repair that failure.
    assert_eq!(MAX_EQUIVALENCE_WORK_V1, 256);
    let (context, function) = parse(&two_pipeline_source(256));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.certificates().len(), 2);
    assert_eq!(report.certificates()[0].pipeline_block(), 0);
    assert_eq!(report.certificates()[0].pipeline_operation(), 2);
    assert_eq!(report.certificates()[0].concrete_epochs(), 1);
    assert_eq!(report.certificates()[0].staged_writes(), 0);
    assert_eq!(report.certificates()[1].staged_writes(), 2);
    assert_eq!(report.certificates()[1].consuming_reads(), 1);

    let (context, function) = parse(&two_pipeline_source(257));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(
        matches!(
            report.findings(),
            [PlironPipelineProtocolFindingV1::AnalysisIncomplete { detail }]
            if detail == "pipeline expression equivalence exceeded its authenticated resource limit"
        ),
        "{report:?}"
    );
    assert!(report.certificates().is_empty());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}
