use super::super::{Budget, FlowWorkProfile, FlowWorkStage};
use super::*;

fn enabled(events: usize) -> Observation {
    Observation::with_setting(
        Some((27, [0x81; 32])),
        [100, 7, 3, 6, 19],
        11,
        Some(OsStr::new("1")),
        Some(OsStr::new("uses")),
        events,
    )
}

fn owner() -> MatchOwner {
    MatchOwner {
        index: 21,
        facts: 31,
        fact_count: 6,
        keys: [2, 2],
    }
}

fn read(observation: &mut Observation, reader: usize, count: usize) {
    if count == 0 {
        return;
    }
    let (site, source) = observation
        .current
        .map_or(((3, 9), 101), |current| (current.site, current.source));
    observation.enter(
        if reader == 0 {
            Part::GlobalStatement
        } else {
            Part::GlobalCapture
        },
        site.0,
        site.1,
    );
    observation.charged(1);
    observation.query(source, owner());
    observation.charged(3);
    observation.dispatch(3);
    observation.bucket(41, 6);
    for ordinal in 0..count {
        observation.charged(16);
        observation.matcher(ordinal, true);
        observation.callback(ordinal, ordinal + 1 == count);
    }
}

fn text(observation: &Observation, total: usize) -> String {
    let output = observation
        .take_summary("compiler-budget-prefix", total, Some((0, 16)))
        .unwrap();
    assert!(output.len <= OUTPUT_BYTES);
    let text = std::str::from_utf8(&output.bytes[..output.len])
        .unwrap()
        .to_owned();
    assert_eq!(text.lines().count(), 1);
    text
}

#[test]
fn exact_opt_in_identity_and_disabled_state_are_preserved() {
    for identity in [None, Some((27, [0x81; 32]))] {
        for trace in [None, Some(OsStr::new("true")), Some(OsStr::new("1"))] {
            for role in [
                None,
                Some(OsStr::new("workgroup")),
                Some(OsStr::new("uses")),
            ] {
                let mut observation =
                    Observation::with_setting(identity, [0; 5], 11, trace, role, 10);
                observation.enter(Part::CarrierSource, 1, 2);
                observation.charged(7);
                let expected = identity.is_some()
                    && trace == Some(OsStr::new("1"))
                    && role == Some(OsStr::new("uses"));
                assert_eq!(
                    observation.take_summary("test", 7, None).is_some(),
                    expected
                );
                assert_eq!(
                    observation.units[Part::CarrierSource as usize],
                    if expected { 7 } else { 0 }
                );
            }
        }
    }
}

#[test]
fn exact_uses83_regions_partition_successful_charges_once() {
    let mut observation = enabled(MAX_DIAGNOSTIC_EVENTS);
    let parts = [
        Part::Setup,
        Part::GlobalSetup,
        Part::Statement,
        Part::GlobalStatement,
        Part::CarrierSource,
        Part::CarrierSiblings,
        Part::CarrierDisjoint,
        Part::ClosedLane,
        Part::GridCapture,
        Part::DefinedCapture,
        Part::GlobalCapture,
        Part::GlobalTerminator,
        Part::TerminalCache,
        Part::TerminalArguments,
        Part::EpochEvidence,
        Part::ArgumentAcceptance,
        Part::UseRecording,
        Part::Unclassified,
    ];
    let mut total = 17;
    observation.charged(17);
    for (index, part) in parts.into_iter().enumerate() {
        observation.enter(part, index as u32, 9);
        observation.charged(index + 1);
        total += index + 1;
    }
    assert_eq!(observation.units.iter().sum::<usize>(), total);
    assert_eq!(observation.visits, [1; PARTS]);
    let out = text(&observation, total);
    assert!(out.contains("collector=complete reason=none"));
    assert!(out.contains("part=unclassified site=Some((17, 9))"));
    assert!(out.contains("unclassified=18/1"));
    assert!(observation.take_summary("again", total, None).is_none());
}

#[test]
fn independent_nested_loop_oracle_counts_duplicate_ordinals_and_one_reader_sites() {
    let mut observation = enabled(MAX_DIAGNOSTIC_EVENTS);
    let mut expected = 0;
    let mut lengths = [0; 2];
    let mut closed = [0; 4];
    for a in 0..=6 {
        for u in 0..=6 {
            let site = (a * 7 + u) as u32;
            observation.statement(11, 100 + site as usize, 0, site);
            read(&mut observation, 0, a);
            read(&mut observation, 1, u);
            // Ordinals stay distinct even when every fact has identical match data.
            for left in 0..a {
                for right in 0..u {
                    expected += usize::from(left == right);
                }
            }
            lengths[0] += a;
            lengths[1] += u;
            closed[usize::from(a != 0) + 2 * usize::from(u != 0)] += 1;
        }
    }
    observation.setup(Part::Unclassified);
    assert_eq!(observation.readers.map(|r| r.matchers), lengths);
    assert_eq!(observation.readers.map(|r| r.callbacks), lengths);
    assert_eq!(observation.overlap, expected);
    assert_eq!(observation.closed, closed);
    assert_eq!(observation.max_audit_prefix, 6);
    assert_eq!(observation.max_union_prefix, 6);
    assert!(observation.incomplete.is_none());
    let out = text(&observation, observation.units.iter().sum());
    assert!(out.contains(&format!("overlap={expected}")));
}

#[test]
fn negative_matches_and_rejected_callbacks_are_not_accepted_uses() {
    let mut observation = enabled(100);
    observation.statement(11, 101, 3, 9);
    observation.enter(Part::GlobalStatement, 3, 9);
    observation.charged(1);
    observation.query(101, owner());
    observation.bucket(41, 6);
    observation.charged(16);
    observation.matcher(0, false);
    observation.charged(16);
    observation.matcher(2, true);
    observation.callback(2, false);
    observation.charged(16);
    observation.matcher(5, true);
    observation.callback(5, true);
    let counts = observation.readers[0];
    assert_eq!(
        (
            counts.matchers,
            counts.matches,
            counts.callbacks,
            counts.accepted
        ),
        (3, 2, 2, 1)
    );
    assert_eq!(counts.last_accepted, Some((3, 9, 5)));
    assert_eq!(observation.current.unwrap().accepted, [Some(5), None]);
}

#[test]
fn setup_failure_has_no_fabricated_statement_identity() {
    let mut observation = enabled(10);
    observation.setup(Part::GlobalSetup);
    observation.charged(5);
    let out = text(&observation, 5);
    assert!(out.contains("part=global-setup site=None"));
    assert!(out.contains("current-prefix=[0, 0]"));
    assert!(out.contains("keys=None"));
}

#[test]
fn body_statement_index_facts_and_bucket_substitutions_mark_unknown() {
    for mutation in 0..8 {
        let mut observation = enabled(100);
        observation.statement(11, 101, 3, 9);
        read(&mut observation, 0, 1);
        observation.enter(Part::GlobalCapture, 3, 9);
        let mut changed = owner();
        match mutation {
            0 => observation.statement(12, 101, 3, 9),
            1 => observation.query(102, changed),
            2..=5 => {
                match mutation {
                    2 => changed.index += 1,
                    3 => changed.facts += 1,
                    4 => changed.fact_count += 1,
                    _ => changed.keys[0] += 1,
                }
                observation.query(101, changed);
            }
            _ => {
                observation.query(101, changed);
                observation.bucket(
                    if mutation == 6 { 42 } else { 41 },
                    if mutation == 7 { 5 } else { 6 },
                );
            }
        }
        let out = text(&observation, observation.units.iter().sum());
        assert!(out.contains("collector=incomplete"), "{mutation}: {out}");
        assert!(out.contains("overlap=unknown"), "{mutation}: {out}");
    }
}

#[test]
fn duplicate_query_or_reordered_fact_ordinals_do_not_claim_overlap() {
    for mutation in 0..3 {
        let mut observation = enabled(100);
        observation.statement(11, 101, 3, 9);
        observation.enter(Part::GlobalStatement, 3, 9);
        observation.query(101, owner());
        observation.bucket(41, 6);
        observation.matcher(2, false);
        match mutation {
            0 => observation.query(101, owner()),
            1 => observation.matcher(1, false),
            _ => observation.matcher(2, false),
        }
        assert!(text(&observation, 0).contains("overlap=unknown"));
    }
}

#[test]
fn diagnostic_exhaustion_is_explicit_before_or_during_collection() {
    for cap in [0, 1, 5, 12] {
        let mut observation = enabled(cap);
        observation.statement(11, 101, 3, 9);
        read(&mut observation, 0, 3);
        // Diagnostic exhaustion freezes even the current site; no work grant results.
        if observation.incomplete.is_none() {
            read(&mut observation, 1, 3);
        }
        observation.charged(100);
        let out = text(&observation, 100);
        assert!(out.contains("collector=incomplete reason=diagnostic-budget"));
        assert!(out.contains("overlap=unknown"));
    }
}

#[test]
fn output_and_state_are_fixed_size_and_overflow_is_incomplete() {
    assert!(std::mem::size_of::<Observation>() <= 2048);
    let mut observation = enabled(10);
    observation.identity = Some((u32::MAX, [0xff; 32]));
    observation.units = [usize::MAX; PARTS];
    observation.visits = [usize::MAX; PARTS];
    observation.rosters = [usize::MAX; 5];
    observation.closed = [usize::MAX; 4];
    observation.readers = [Reader {
        queries: usize::MAX,
        matchers: usize::MAX,
        matches: usize::MAX,
        callbacks: usize::MAX,
        accepted: usize::MAX,
        dispatch: usize::MAX,
        last_accepted: Some((u32::MAX, u32::MAX, usize::MAX)),
    }; 2];
    observation.charged(1);
    let out = text(&observation, usize::MAX);
    assert!(out.contains("collector=incomplete"));
    assert!(out.contains("overlap=unknown"));
    let mut bounded = Text::default();
    assert!(bounded.write_str(&"x".repeat(OUTPUT_BYTES + 1)).is_err());
    assert_eq!(bounded.len, 0);
}

#[test]
fn original_compiler_limit_and_error_oracle_ignore_diagnostic_allowance() {
    assert_eq!(super::super::MAX_FLOW_WORK, 262_144);
    for limit in 0..64 {
        for diagnostic_cap in [0, 1, 1_000] {
            let mut cold = Budget {
                remaining: limit,
                limit,
                profile: FlowWorkProfile::default(),
            };
            let mut traced = Budget {
                remaining: limit,
                limit,
                profile: FlowWorkProfile::default(),
            };
            traced.profile.uses = Observation::for_test(11, 6, diagnostic_cap);
            let mut remaining = limit;
            for (stage, work) in [
                (FlowWorkStage::Facts, 3),
                (FlowWorkStage::Candidates, 2),
                (FlowWorkStage::Uses, 4),
                (FlowWorkStage::Uses, 0),
                (FlowWorkStage::Uses, 16),
                (FlowWorkStage::Uses, 9),
                (FlowWorkStage::Paths, 5),
            ] {
                cold.profile.stage = stage;
                traced.profile.stage = stage;
                traced.profile.uses.enter(Part::GlobalStatement, 2, 7);
                let expected_remaining = remaining.checked_sub(work);
                let expected = cold.charge(work);
                let observed = traced.charge(work);
                assert_eq!(expected, observed);
                assert_eq!(cold.remaining, traced.remaining);
                assert_eq!(observed.is_ok(), expected_remaining.is_some());
                match expected_remaining {
                    Some(next) => remaining = next,
                    None => {
                        assert_eq!(traced.remaining, remaining);
                        break;
                    }
                }
            }
        }
    }
}
