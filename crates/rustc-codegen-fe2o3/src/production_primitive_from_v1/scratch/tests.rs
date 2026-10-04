use super::*;

fn limits(bytes: usize) -> SemanticMirLimitsV1 {
    SemanticMirLimitsV1::default()
        .with_limit(
            SemanticMirResourceV1::CanonicalBytes,
            u64::try_from(bytes).unwrap(),
        )
        .unwrap()
}
fn stages() -> [Stage; 4] {
    [
        Stage::Collector,
        Stage::ClosureReplay,
        Stage::Preflight,
        Stage::BodyReplay,
    ]
}

#[test]
fn primitive_from_scratch_requested_exact_short_and_zero_are_typed_all_stages() {
    for stage in stages() {
        let locals = 10;
        let blocks = 5;
        let expected = size_of::<Scratch>()
            + locals * size_of::<Fact>()
            + locals * size_of::<bool>()
            + blocks * size_of::<bool>();
        let mut work = 17;
        let mut charge = |n| {
            work += n;
            Ok::<_, ()>(())
        };
        let mut cx = Context {
            stage,
            limits: limits(expected),
            charge: &mut charge,
            counts: [0; 8],
        };
        let scratch = Scratch::new(locals, blocks, &mut cx).unwrap();
        let actual = size_of::<Scratch>()
            + scratch.facts.capacity() * size_of::<Fact>()
            + scratch.aliases.capacity() * size_of::<bool>()
            + scratch.visited.capacity() * size_of::<bool>();
        assert_eq!(actual, expected, "pinned allocator capacity oracle");
        assert_eq!(scratch.facts, vec![Fact::Unknown; locals]);
        assert_eq!(scratch.aliases, vec![false; locals]);
        assert_eq!(scratch.visited, vec![false; blocks]);
        drop(scratch);
        assert_eq!(work, 17 + 4 + 2 * locals + blocks);
        for maximum in [0, expected - 1] {
            let expected_actual = if maximum == 0 {
                size_of::<Scratch>() as u64
            } else {
                expected as u64
            };
            let expected_slab = if maximum == 0 {
                Slab::Header
            } else {
                Slab::Blocks
            };
            let mut attempts = 17;
            let result = Scratch::new(
                locals,
                blocks,
                &mut Context {
                    stage,
                    limits: limits(maximum),
                    charge: &mut |n| {
                        attempts += n;
                        Ok::<_, ()>(())
                    },
                    counts: [0; 8],
                },
            );
            assert!(
                matches!(result, Err(PrimitiveFromErrorV1::Resource { stage: s,
                source: Resource::ScratchLimit { phase: Phase { slab, backing: Backing::Requested }, actual, maximum: m } })
                if s == stage && m == maximum as u64 && actual == expected_actual && slab == expected_slab)
            );
            assert_eq!(
                attempts,
                if maximum == 0 {
                    18
                } else {
                    17 + 4 + 2 * locals
                }
            );
        }
    }
}

#[test]
fn primitive_from_scratch_work_keeps_attempts_typed_and_sequential_on_error_and_unwind() {
    for stage in stages() {
        let one = 4 + 2 * 3 + 2;
        let needed = 17 + 2 * one;
        for limit in [needed, needed - 1] {
            let mut work = 17;
            let mut charge = |n| {
                work += n;
                if work > limit {
                    Err((work, limit))
                } else {
                    Ok(())
                }
            };
            let mut cx = Context {
                stage,
                limits: limits(4096),
                charge: &mut charge,
                counts: [0; 8],
            };
            drop(Scratch::new(3, 2, &mut cx).unwrap());
            let result = Scratch::new(3, 2, &mut cx);
            if limit == needed {
                assert!(result.is_ok());
            } else {
                assert!(
                    matches!(result, Err(PrimitiveFromErrorV1::Work { stage: s, source: (actual, maximum) }) if s == stage && actual == needed && maximum == limit)
                );
            }
            assert_eq!(work, needed);
        }
        let mut work = 17;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Scratch::new(
                3,
                2,
                &mut Context {
                    stage,
                    limits: limits(4096),
                    counts: [0; 8],
                    charge: &mut |n| {
                        work += n;
                        if work >= 17 + one {
                            std::panic::resume_unwind(Box::new(()));
                        }
                        Ok::<_, ()>(())
                    },
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(work, 17 + one);
        let mut cx = Context {
            stage,
            limits: limits(4096),
            counts: [0; 8],
            charge: &mut |n| {
                work += n;
                Ok::<_, ()>(())
            },
        };
        drop(Scratch::new(3, 2, &mut cx).unwrap());
        assert_eq!(work, 17 + 2 * one);
    }
}

#[test]
fn primitive_from_scratch_actual_capacity_and_overflow_are_distinct_checks() {
    let phase = Phase {
        slab: Slab::Locals,
        backing: Backing::Actual,
    };
    let mut charge = |_| Ok::<_, ()>(());
    let cx = Context {
        stage: Stage::Preflight,
        limits: limits(100),
        charge: &mut charge,
        counts: [0; 8],
    };
    assert!(
        matches!(bytes(101, 1, 0, phase, &cx), Err(PrimitiveFromErrorV1::Resource {
        source: Resource::ScratchLimit { phase: p, actual: 101, maximum: 100 }, .. }) if p == phase)
    );
    assert!(
        matches!(bytes(usize::MAX, 2, 0, phase, &cx), Err(PrimitiveFromErrorV1::Resource {
        source: Resource::SizeOverflow { phase: p }, .. }) if p == phase)
    );
    assert!(
        matches!(bytes(1, 1, usize::MAX, phase, &cx), Err(PrimitiveFromErrorV1::Resource {
        source: Resource::SizeOverflow { phase: p }, .. }) if p == phase)
    );
    // Arithmetic/check coverage only: allocator overcapacity/failure is not injected.
}
