use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "role_tests.rs"]
mod role_tests;
#[path = "lane_tests.rs"]
mod lane_tests;
#[path = "policy_tests.rs"]
mod policy_tests;
#[path = "site_tests.rs"]
mod site_tests;

#[path = "../../matrix_borrows_v1/canonical_fixture.rs"]
mod fixture;

fn source() -> (AdmittedInertSemanticMirV1, SemanticCallExpansionV1) {
    let source = fixture::source(fixture::Mutation::None);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    expansion.verify_replay(&source).unwrap();
    (source, expansion)
}

fn enabled<'a>(
    view: Option<&'a SemanticExpandedRootV1>,
    owned: impl Iterator<Item = u32>,
    candidates: &[SemanticBorrowCandidateV1],
) -> Observation<'a> {
    Observation::with_setting(view, owned, candidates, Some(OsStr::new("1")))
}

// Observation-state tests, not a substitute for the production candidate pass.
// Coordinates and nodes come from a real admitted/replayed source fixture.
fn candidates(view: &SemanticExpandedRootV1) -> Vec<SemanticBorrowCandidateV1> {
    let mut result = Vec::new();
    for (block, body) in view.body().blocks().iter().enumerate() {
        for (statement, node) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = node.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
                continue;
            };
            result.push(SemanticBorrowCandidateV1 {
                site: Site {
                    block: block as u32,
                    statement: statement as u32,
                },
                source_local: place.local().index(),
                source_type: place.ty(),
                source_reference: None,
                value_alias: false, source_kind: SemanticBorrowCandidateSourceV1::Direct,
                valid: true,
                consumers: 0,
                intrinsic_consumer: false,
            });
        }
    }
    assert!(result.len() >= 2);
    result
}

#[test]
fn actual_pass_observer_retains_the_first_invalidation_and_exact_origin() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut candidates = candidates(view);
    let mut observation = enabled(
        Some(view),
        candidates.iter().map(|c| c.source_type.index()),
        &candidates,
    );
    observation.after(candidates[0].site, "no-change", &candidates);
    assert!(
        observation.tracked[..observation.len]
            .iter()
            .flatten()
            .all(|c| c.failure.is_none())
    );
    candidates[0].valid = false;
    let point = candidates[1].site;
    observation.after(point, "ordinary-statement", &candidates);
    observation.after(candidates[0].site, "later-call", &candidates);
    observation.component(0, Some(0), false, &candidates);
    observation.component(0, None, false, &candidates);
    let first = observation.tracked[0].unwrap();
    assert_eq!(first.failure.unwrap().site, point);
    assert_eq!(first.failure.unwrap().mode, "ordinary-statement");
    assert_eq!(first.component, Some((false, Some(0))));
    let before = (
        candidates[0].valid,
        candidates[0].consumers,
        source.canonical_encoding().to_vec(),
    );
    let mut output = Vec::new();
    observation
        .write(&mut output, view, &candidates, &BTreeSet::new(), |_| None)
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("pass_complete=true"));
    assert!(output.contains("initial_valid=true final_valid=false"));
    assert!(output.contains("mode=ordinary-statement"));
    assert!(output.contains("source_block=Some("));
    assert!(output.contains("source_statement=Some("));
    assert!(output.contains("owner_origin=Some("));
    assert_eq!(
        before,
        (
            candidates[0].valid,
            candidates[0].consumers,
            source.canonical_encoding().to_vec()
        )
    );
}

#[test]
fn initial_rejection_and_accepted_component_are_not_relabelled_as_use_loss() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut candidates = candidates(view);
    candidates[0].valid = false;
    let mut observation = enabled(
        Some(view),
        candidates.iter().map(|c| c.source_type.index()),
        &candidates,
    );
    observation.after(candidates[0].site, "unchanged", &candidates);
    observation.component(1, None, true, &candidates);
    let mut accepted = BTreeSet::new();
    accepted.insert(candidates[1].site);
    let mut out = Vec::new();
    observation
        .write(&mut out, view, &candidates, &accepted, |_| None)
        .unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("initial_valid=false final_valid=false"));
    assert!(!out.contains("first_observed_invalidation"));
    assert!(out.contains("accepted_borrow=true component=Some((true, None))"));
}

#[test]
fn observer_does_not_track_unrelated_owned_types_or_disabled_direct_flows() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let candidates = candidates(view);
    let unrelated = enabled(Some(view), [u32::MAX].into_iter(), &candidates);
    assert_eq!(unrelated.len, 0);
    let mut direct = enabled(
        None,
        [candidates[0].source_type.index()].into_iter(),
        &candidates,
    );
    direct.after(candidates[0].site, "disabled", &candidates);
    direct.component(0, Some(0), false, &candidates);
    assert_eq!(direct.len, 0);
    assert_eq!(direct.remaining, MAX_STEPS);
    let empty = enabled(Some(view), std::iter::empty(), &candidates);
    assert!(empty.view.is_none());
}

#[test]
fn observer_work_and_selection_caps_report_truncation_without_changing_candidates() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let first = candidates(view)[0];
    let candidates = vec![first; MAX_TRACKED + 1];
    let mut observation = enabled(
        Some(view),
        [first.source_type.index()].into_iter(),
        &candidates,
    );
    assert_eq!(observation.len, MAX_TRACKED);
    assert!(observation.truncated);
    observation.remaining = 1;
    observation.after(first.site, "no-work", &candidates);
    assert_eq!(observation.remaining, 0);
    assert!(
        observation.tracked[..observation.len]
            .iter()
            .flatten()
            .all(|c| c.failure.is_none())
    );
    let mut output = Vec::new();
    observation
        .write(&mut output, view, &candidates, &BTreeSet::new(), |_| None)
        .unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("observation_truncated=true")
    );
    assert!(candidates.iter().all(|c| c.valid && c.consumers == 0));
    let oversized = enabled(Some(view), (0..MAX_OWNED as u32 + 1).into_iter(), &[]);
    assert_eq!(oversized.owned_len, MAX_OWNED);
    assert!(oversized.truncated);
}

#[test]
fn observation_output_limit_and_writer_failure_are_independent_of_proof_state() {
    let mut output = Output {
        bytes: [0; MAX_OUTPUT],
        len: 0,
        truncated: false,
    };
    output.write_all(&[b'x'; MAX_OUTPUT]).unwrap();
    assert_eq!(output.len, MAX_OUTPUT);
    assert!(output.write_all(b"y").is_err());
    assert!(output.truncated);
    assert_eq!(output.len, MAX_OUTPUT);
    struct Fails;
    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let candidates = candidates(view);
    let observation = enabled(
        Some(view),
        candidates.iter().map(|c| c.source_type.index()),
        &candidates,
    );
    let remaining = observation.remaining;
    assert!(
        observation
            .write(&mut Fails, view, &candidates, &BTreeSet::new(), |_| None)
            .is_err()
    );
    assert_eq!(observation.remaining, remaining);
    assert!(candidates.iter().all(|c| c.valid));
}

#[test]
fn missing_or_nonexact_trace_setting_skips_scans_and_emission() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let candidates = candidates(view);
    for setting in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some(" 1"),
        Some("1 "),
    ] {
        let mut observation = Observation::with_setting(
            Some(view),
            std::iter::from_fn(|| panic!("disabled trace scanned owned types")),
            &candidates,
            setting.map(OsStr::new),
        );
        observation.after(candidates[0].site, "disabled", &candidates);
        observation.component(0, Some(0), false, &candidates);
        observation.emit(view.body(), &candidates, &BTreeSet::new(), |_| {
            panic!("disabled trace queried a route")
        });
        assert!(observation.view.is_none());
        assert_eq!(observation.owned_len, 0);
        assert_eq!(observation.len, 0);
        assert_eq!(observation.remaining, MAX_STEPS);
        assert!(!observation.truncated);
    }
}

#[test]
fn exact_trace_setting_observes_without_changing_candidate_or_source_state() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut candidates = candidates(view);
    candidates[0].valid = false;
    candidates[1].consumers = 1;
    candidates[1].intrinsic_consumer = true;
    let accepted = BTreeSet::from([candidates[1].site]);
    let snapshot = || {
        (
            candidates
                .iter()
                .map(|c| {
                    (
                        c.site,
                        c.source_local,
                        c.source_type,
                        c.source_reference,
                        c.value_alias,
                        c.valid,
                        c.consumers,
                        c.intrinsic_consumer,
                    )
                })
                .collect::<Vec<_>>(),
            accepted.clone(),
            source.canonical_encoding().to_vec(),
            *view.identity(),
        )
    };
    let before = snapshot();
    for setting in [None, Some(OsStr::new("1"))] {
        let mut observation = Observation::with_setting(
            Some(view),
            candidates.iter().map(|c| c.source_type.index()),
            &candidates,
            setting,
        );
        observation.after(candidates[0].site, "same-input", &candidates);
        observation.component(0, Some(0), false, &candidates);
        observation.component(1, None, true, &candidates);
        if setting.is_some() {
            assert!(observation.view.is_some());
            assert!(observation.len >= 2);
            let mut output = Vec::new();
            observation
                .write(&mut output, view, &candidates, &accepted, |_| None)
                .unwrap();
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("initial_valid=false final_valid=false"));
            assert!(output.contains("accepted_borrow=true component=Some((true, None))"));
        } else {
            assert!(observation.view.is_none());
        }
        assert_eq!(before, snapshot());
    }
}
