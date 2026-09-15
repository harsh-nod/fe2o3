//! Diagnostic state only; source nodes come from the parent's admitted/replayed fixture.
use super::*;

fn selection(body: &SemanticFunctionDeclV1, local: u32, site: Site) -> StorageObservation {
    let identity = body
        .identity()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    StorageObservation::selected(
        body,
        &format!("{identity}/{local}/{}/{}", site.block, site.statement),
    )
    .unwrap()
}

fn same_candidates(actual: &[SemanticBorrowCandidateV1], expected: &[SemanticBorrowCandidateV1]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert_eq!(
            (
                a.site,
                a.source_local,
                a.source_type,
                a.source_reference,
                a.value_alias,
                a.source_kind,
                a.valid,
                a.consumers,
                a.intrinsic_consumer
            ),
            (
                b.site,
                b.source_local,
                b.source_type,
                b.source_reference,
                b.value_alias,
                b.source_kind,
                b.valid,
                b.consumers,
                b.intrinsic_consumer
            )
        );
    }
}

fn inactive(observation: &Observation<'_>, remaining: usize) {
    assert!(observation.view.is_none());
    assert!(observation.selected_site.is_none());
    assert_eq!(observation.scope, "site-owned-type");
    assert_eq!(
        (
            observation.owned_len,
            observation.len,
            observation.remaining
        ),
        (0, 0, remaining)
    );
    assert!(!observation.truncated);
}

// Add an observation-only alias row at a real expanded reference-forwarding
// assignment. No source, production candidate pass or SSA event is changed.
fn family(view: &SemanticExpandedRootV1) -> (Vec<SemanticBorrowCandidateV1>, usize, usize) {
    let mut rows = candidates(view);
    for root in 0..rows.len() {
        let candidate = rows[root];
        let SemanticStatementKindV1::Assign(borrow) = view.body().blocks()
            [candidate.site.block as usize]
            .statements()[candidate.site.statement as usize]
            .kind()
        else {
            unreachable!()
        };
        let reference = borrow.destination();
        for (block, body) in view.body().blocks().iter().enumerate() {
            for (statement, node) in body.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = node.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                ) = assignment.value().kind()
                else {
                    continue;
                };
                if place.local() != reference.local()
                    || place.ty() != reference.ty()
                    || !place.projections().is_empty()
                    || !assignment.destination().projections().is_empty()
                    || assignment.destination().ty() != reference.ty()
                {
                    continue;
                }
                let alias = rows.len();
                rows.push(SemanticBorrowCandidateV1 {
                    site: Site {
                        block: block as u32,
                        statement: statement as u32,
                    },
                    source_local: place.local().index(),
                    source_reference: Some(place.local().index()),
                    value_alias: true,
                    ..candidate
                });
                assert_ne!(rows[root].site, rows[alias].site);
                return (rows, root, alias);
            }
        }
    }
    panic!("replayed fixture must retain a Borrow reference's argument forwarding");
}

fn output(
    observation: &Observation<'_>,
    view: &SemanticExpandedRootV1,
    rows: &[SemanticBorrowCandidateV1],
    accepted: &BTreeSet<Site>,
) -> String {
    let mut bytes = Vec::new();
    observation
        .write(&mut bytes, view, rows, accepted, |_| None)
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn storage_site96_disabled_or_missing_inputs_do_not_spend_or_scan() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let rows = candidates(view);
    let selected = selection(view.body(), rows[0].source_local, rows[0].site);
    let before = rows.clone();
    let encoded = source.canonical_encoding().to_vec();
    for flag in [None, Some(""), Some("0"), Some("true"), Some("1 ")] {
        let mut observation = Observation::with_site_selection(
            Some(view),
            &rows,
            flag.map(OsStr::new),
            Some(&selected),
        );
        observation.after(rows[0].site, "disabled", &rows);
        observation.component(0, Some(0), false, &rows);
        observation.emit(view.body(), &rows, &BTreeSet::new(), |_| {
            panic!("disabled route lookup")
        });
        inactive(&observation, MAX_STEPS);
    }
    inactive(
        &Observation::with_site_selection(None, &rows, Some(OsStr::new("1")), Some(&selected)),
        MAX_STEPS,
    );
    inactive(
        &Observation::with_site_selection(Some(view), &rows, Some(OsStr::new("1")), None),
        MAX_STEPS,
    );
    same_candidates(&rows, &before);
    assert_eq!(source.canonical_encoding(), encoded.as_slice());
}

#[test]
fn storage_site96_wrong_body_local_query_and_nonborrow_stop_before_family_scan() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (rows, root, alias) = family(view);
    let borrowed = rows[root];
    let foreign = source
        .functions()
        .iter()
        .find(|body| body.identity() != view.body().identity())
        .unwrap();
    let wrong_local = if borrowed.source_local == 0 { 1 } else { 0 };
    for selected in [
        selection(foreign, 0, borrowed.site),
        selection(view.body(), wrong_local, borrowed.site),
        selection(
            view.body(),
            borrowed.source_local,
            Site {
                block: u32::MAX,
                statement: 0,
            },
        ),
        selection(
            view.body(),
            borrowed.source_local,
            Site {
                block: borrowed.site.block,
                statement: u32::MAX,
            },
        ),
        selection(view.body(), rows[alias].source_local, rows[alias].site),
    ] {
        let observation = Observation::with_site_selection(
            Some(view),
            &rows,
            Some(OsStr::new("1")),
            Some(&selected),
        );
        inactive(&observation, MAX_STEPS - 16);
    }
    let identity = view
        .body()
        .identity()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(
        StorageObservation::selected(
            view.body(),
            &format!("{identity}/{}/0/0", view.body().locals().len())
        )
        .is_none()
    );
}

#[test]
fn storage_site96_exact_borrow_selects_whole_owned_family_without_mutation() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (mut rows, root, alias) = family(view);
    rows[alias].valid = false;
    rows[alias].consumers = 2;
    rows[alias].intrinsic_consumer = true;
    let selected = selection(view.body(), rows[root].source_local, rows[root].site);
    assert_eq!(
        selected.selected_site(view.body()),
        Some((rows[root].source_local, rows[root].site))
    );
    let before = rows.clone();
    let encoded = source.canonical_encoding().to_vec();
    let body = view.body().clone();
    let observation =
        Observation::with_site_selection(Some(view), &rows, Some(OsStr::new("1")), Some(&selected));
    assert!(std::ptr::eq(observation.view.unwrap(), view));
    assert_eq!(observation.scope, "site-owned-type");
    assert_eq!(observation.selected_site, Some(rows[root].site));
    assert_eq!(
        &observation.owned[..observation.owned_len],
        &[rows[root].source_type.index()]
    );
    let expected = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.source_type == rows[root].source_type).then_some(index))
        .collect::<Vec<_>>();
    let actual = observation.tracked[..observation.len]
        .iter()
        .flatten()
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(actual.contains(&root) && actual.contains(&alias));
    let tracked_alias = observation.tracked[..observation.len]
        .iter()
        .flatten()
        .find(|row| row.index == alias)
        .unwrap();
    assert!(!tracked_alias.initial && !tracked_alias.previous);
    assert!(tracked_alias.failure.is_none());
    assert!(
        actual.len() < rows.len(),
        "unrelated owned types must remain unselected"
    );
    assert_eq!(observation.remaining, MAX_STEPS - 16 - 17 - 17 * rows.len());
    assert!(!observation.truncated);
    let text = output(&observation, view, &rows, &BTreeSet::new());
    let site = rows[root].site;
    let origin = &view.block_origins()[site.block as usize];
    assert!(text.contains(&format!(
        "selected_borrow_site=({}, {})",
        site.block, site.statement
    )));
    assert!(text.contains(&format!(
        "source_block={:?} source_statement={:?}",
        Some((
            origin.instance().index(),
            origin.function().index(),
            origin.block().index()
        )),
        origin.statements().get(site.statement as usize)
    )));
    assert!(text.contains(&format!(
        "borrow-place local={} ty={}",
        rows[root].source_local,
        rows[root].source_type.index()
    )));
    assert!(text.contains("scope=site-owned-type"));
    let empty =
        Observation::with_site_selection(Some(view), &[], Some(OsStr::new("1")), Some(&selected));
    assert_eq!(
        (empty.owned_len, empty.len, empty.remaining),
        (1, 0, MAX_STEPS - 33)
    );
    assert_eq!(empty.selected_site, Some(site));
    same_candidates(&rows, &before);
    assert_eq!(source.canonical_encoding(), encoded.as_slice());
    assert_eq!(view.body(), &body);
}

#[test]
fn storage_site96_alias_first_failure_component_and_accepted_roster_are_retained() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let (mut rows, root, alias) = family(view);
    let selected = selection(view.body(), rows[root].source_local, rows[root].site);
    let original = rows.clone();
    let encoded = source.canonical_encoding().to_vec();
    let mut observation =
        Observation::with_site_selection(Some(view), &rows, Some(OsStr::new("1")), Some(&selected));
    observation.after(rows[root].site, "unchanged", &rows);
    assert!(
        observation.tracked[..observation.len]
            .iter()
            .flatten()
            .all(|row| row.failure.is_none())
    );
    rows[alias].valid = false;
    observation.after(rows[alias].site, "alias-escape", &rows);
    observation.after(rows[root].site, "later", &rows);
    observation.component(root, Some(alias), false, &rows);
    observation.component(root, None, false, &rows);
    let tracked = observation.tracked[..observation.len]
        .iter()
        .flatten()
        .find(|row| row.index == alias)
        .unwrap();
    assert_eq!(tracked.failure.unwrap().site, rows[alias].site);
    assert_eq!(tracked.failure.unwrap().mode, "alias-escape");
    let parent = observation.tracked[..observation.len]
        .iter()
        .flatten()
        .find(|row| row.index == root)
        .unwrap();
    assert_eq!(parent.component, Some((false, Some(alias))));
    let before_write = rows.clone();
    let text = output(&observation, view, &rows, &BTreeSet::new());
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("candidate={alias} ")))
        .unwrap();
    assert!(line.contains("alias=true initial_valid=true final_valid=false"));
    assert!(text.contains(&format!(
        "first_observed_invalidation candidate={alias} mode=alias-escape"
    )));
    assert!(!text.contains("mode=later"));
    assert!(text.contains(&format!(
        "accepted_borrow=false component=Some((false, Some({alias})))"
    )));
    same_candidates(&rows, &before_write);

    let positive_before = original.clone();
    let mut positive = Observation::with_site_selection(
        Some(view),
        &original,
        Some(OsStr::new("1")),
        Some(&selected),
    );
    positive.component(root, None, true, &original);
    let accepted = BTreeSet::from([original[root].site]);
    let accepted_before = accepted.clone();
    let text = output(&positive, view, &original, &accepted);
    let line = text
        .lines()
        .find(|line| line.starts_with(&format!("candidate={root} ")))
        .unwrap();
    assert!(line.contains("accepted_borrow=true component=Some((true, None))"));
    assert!(!text.contains("first_observed_invalidation"));
    assert_eq!(accepted, accepted_before);
    same_candidates(&original, &positive_before);
    assert_eq!(source.canonical_encoding(), encoded.as_slice());
}

#[test]
fn storage_site96_tracking_cap_and_work_prefix_keep_existing_limits() {
    assert_eq!(
        (MAX_OWNED, MAX_TRACKED, MAX_STEPS, MAX_OUTPUT),
        (16, 64, 65_536, 32_768)
    );
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let first = candidates(view)[0];
    let selected = selection(view.body(), first.source_local, first.site);
    for count in [64, 65, 66] {
        let rows = vec![first; count];
        let before = rows.clone();
        let mut observation = Observation::with_site_selection(
            Some(view),
            &rows,
            Some(OsStr::new("1")),
            Some(&selected),
        );
        assert_eq!((observation.owned_len, observation.len), (1, 64));
        assert_eq!(observation.truncated, count > 64);
        assert_eq!(
            observation.remaining,
            MAX_STEPS - 16 - 17 - 17 * count.min(65)
        );
        assert_eq!(
            observation.tracked[..64]
                .iter()
                .flatten()
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            (0..64).collect::<Vec<_>>()
        );
        observation.remaining = 1;
        observation.after(first.site, "no-candidate-work", &rows);
        assert_eq!(observation.remaining, 0);
        assert!(observation.truncated);
        assert!(
            observation.tracked[..64]
                .iter()
                .flatten()
                .all(|row| row.failure.is_none())
        );
        same_candidates(&rows, &before);
    }
}

#[test]
fn storage_site96_bounded_output_and_write_failure_do_not_change_observation_or_source() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let rows = candidates(view);
    let selected = selection(view.body(), rows[0].source_local, rows[0].site);
    let observation =
        Observation::with_site_selection(Some(view), &rows, Some(OsStr::new("1")), Some(&selected));
    let before = rows.clone();
    let encoded = source.canonical_encoding().to_vec();
    let remaining = observation.remaining;
    let text = output(&observation, view, &rows, &BTreeSet::new());
    let mut bounded = Output {
        bytes: [0; MAX_OUTPUT],
        len: 0,
        truncated: false,
    };
    observation
        .write(&mut bounded, view, &rows, &BTreeSet::new(), |_| None)
        .unwrap();
    assert_eq!(&bounded.bytes[..bounded.len], text.as_bytes());
    assert!(!bounded.truncated);
    bounded
        .write_all(&vec![b'x'; MAX_OUTPUT - bounded.len])
        .unwrap();
    assert_eq!(bounded.len, 32_768);
    assert_eq!(
        observation
            .write(&mut bounded, view, &rows, &BTreeSet::new(), |_| None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WriteZero
    );
    assert!(bounded.truncated);
    assert_eq!(bounded.len, 32_768);
    struct Fails;
    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        observation
            .write(&mut Fails, view, &rows, &BTreeSet::new(), |_| None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(output(&observation, view, &rows, &BTreeSet::new()), text);
    assert_eq!(observation.remaining, remaining);
    same_candidates(&rows, &before);
    assert_eq!(source.canonical_encoding(), encoded.as_slice());
}
