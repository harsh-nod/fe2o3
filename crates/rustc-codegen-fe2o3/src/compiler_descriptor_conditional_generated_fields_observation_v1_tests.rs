use super::*;

/// Assertion helper over inert observations. It does not validate a receipt or
/// admit anything; compare the receipt with the separate retention observation.
pub(crate) fn check(observed: &Observation) {
    let Some((Event::ReplayCompleted, callbacks)) = observed.events.split_last() else {
        panic!("generated-field replay did not complete its postchecks");
    };
    assert!(!callbacks.is_empty());
    for event in callbacks {
        let Event::ProjectionCallback(projection) = event else {
            panic!("unexpected completion before the last callback");
        };
        assert!(
            projection
                .arguments
                .windows(2)
                .all(|pair| { pair[0].canonical_parameter < pair[1].canonical_parameter })
        );
        assert!(projection.arguments[usize::from(projection.output_argument)].output);
        assert_eq!(projection.arguments.iter().filter(|a| a.output).count(), 1);
        for read in &projection.reads {
            assert!(read_matches(projection, read));
        }
        for argument in &projection.arguments {
            assert!(
                argument.output
                    || projection
                        .reads
                        .iter()
                        .any(|read| { read.canonical_parameter == argument.canonical_parameter })
            );
        }
    }
}

fn read_matches(projection: &Projection, read: &Read) -> bool {
    projection
        .arguments
        .get(usize::from(read.argument))
        .is_some_and(|argument| {
            !argument.output
                && argument.canonical_parameter == read.canonical_parameter
                && argument.source_argument == read.source_argument
                && argument.adjusted_argument == read.adjusted_argument
                && argument.semantic_local == read.semantic_local
                && argument.semantic_type == read.semantic_type
        })
}

fn argument(canonical: u32, source: u32, output: bool) -> Argument {
    Argument {
        canonical_parameter: canonical,
        canonical_value: canonical + 10,
        source_argument: source,
        adjusted_argument: source,
        semantic_local: source + 1,
        semantic_type: if output { 9 } else { 8 },
        generated_field: source as u16,
        output,
        source_type_identity: [1; 32],
        device_layout_identity: [2; 32],
        generated_offset: source * 16,
        generated_semantic_type_identity: [3; 32],
    }
}

fn read(argument: u16, canonical: u32, source: u32, op: u32) -> Read {
    Read {
        argument,
        canonical_parameter: canonical,
        source_argument: source,
        adjusted_argument: source,
        semantic_local: source + 1,
        semantic_type: 8,
        canonical_block: 1,
        canonical_operation: op as usize,
        ranked_block: 2,
        ranked_operation: op + 100,
    }
}

// Inert observer fixtures cannot construct a Request, checked view or receipt.
fn projection() -> Projection {
    Projection {
        root: 2,
        body: 3,
        kernel_binding: [4; 32],
        arguments: vec![
            argument(2, 1, false),
            argument(5, 2, false),
            argument(8, 0, true),
        ],
        output_argument: 2,
        reads: vec![read(1, 5, 2, 3), read(0, 2, 1, 1), read(1, 5, 2, 9)],
        canonical_store_block: 1,
        canonical_store_operation: 12,
        statement: [5; 32],
        receipt: [6; 32],
        work: 100,
        storage: 200,
    }
}

#[test]
fn observer_preserves_distinct_ordinals_and_repeated_read_order() {
    let (_, observed) = observe(|| {
        record(|| Event::ProjectionCallback(projection()));
        replay_completed();
    });
    check(&observed);
    let Event::ProjectionCallback(p) = &observed.events[0] else {
        panic!()
    };
    assert_eq!(
        p.arguments
            .iter()
            .map(|a| a.generated_field)
            .collect::<Vec<_>>(),
        [1, 2, 0]
    );
    assert_eq!(
        p.reads.iter().map(|r| r.argument).collect::<Vec<_>>(),
        [1, 0, 1]
    );
    assert_eq!(
        p.reads
            .iter()
            .map(|r| r.canonical_operation)
            .collect::<Vec<_>>(),
        [3, 1, 9]
    );
    assert_eq!(
        p.reads
            .iter()
            .map(|r| r.ranked_operation)
            .collect::<Vec<_>>(),
        [103, 101, 109]
    );
    assert_eq!(
        p.arguments[usize::from(p.output_argument)].generated_offset,
        0
    );
}

#[test]
fn observed_read_index_and_every_source_coordinate_must_agree() {
    let p = projection();
    let original = p.reads[0].clone();
    for case in 0..8 {
        let mut changed = original.clone();
        match case {
            0 => changed.argument = 0,
            1 => changed.argument = p.output_argument,
            2 => changed.argument = u16::MAX,
            3 => changed.canonical_parameter += 1,
            4 => changed.source_argument += 1,
            5 => changed.adjusted_argument += 1,
            6 => changed.semantic_local += 1,
            7 => changed.semantic_type += 1,
            _ => unreachable!(),
        }
        assert!(
            !read_matches(&p, &changed),
            "observation substitution {case}"
        );
    }
    assert!(read_matches(&p, &original));
}

#[test]
fn callback_observation_is_not_postcheck_completion() {
    let (_, observed) = observe(|| record(|| Event::ProjectionCallback(projection())));
    assert!(std::panic::catch_unwind(|| check(&observed)).is_err());
    assert_eq!(observed.events.len(), 1);
}

#[test]
fn no_observer_does_not_construct_diagnostic_payload() {
    record(|| panic!("unobserved diagnostic allocation"));
}

#[test]
fn observation_restores_on_unwind_and_rejects_nesting() {
    let panic = std::panic::catch_unwind(|| {
        observe(|| {
            record(|| Event::ProjectionCallback(projection()));
            panic!("descriptor consumer unwound before postchecks");
        })
    });
    assert!(panic.is_err());
    let (answer, observed) = observe(|| 7);
    assert_eq!(answer, 7);
    assert!(observed.events.is_empty());
    assert!(std::panic::catch_unwind(|| observe(|| observe(|| ()))).is_err());
    assert!(observe(|| ()).1.events.is_empty());
}

#[test]
fn observer_has_a_fixed_complete_root_roster_event_bound() {
    let (_, observed) = observe(|| {
        for _ in 0..MAX_CONDITIONAL_ROOTS_V1 {
            record(|| Event::ProjectionCallback(projection()));
        }
        replay_completed();
        assert!(std::panic::catch_unwind(replay_completed).is_err());
    });
    assert_eq!(observed.events.len(), MAX_CONDITIONAL_ROOTS_V1 + 1);
}
