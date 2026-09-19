//! Inert protocol controls only; these do not qualify source or simulated I.
use super::*;

fn inert_observation(case: Case, reverse: bool) -> Observation6 {
    let mut roots = ROOTS.map(str::to_owned).to_vec();
    if reverse {
        roots.reverse();
    }
    let endpoint = |output_digest| shifts::ShiftObservation {
        batch: case.batch,
        source_digest: [1; 32],
        original_digest: [2; 32],
        output_digest,
        roots: roots
            .iter()
            .map(|root| {
                let ordinal = simulation::masked_shift::root_ordinal(root).unwrap() as u8;
                let owner = if case.batch.retained {
                    format!("helper-{root}")
                } else {
                    root.clone()
                };
                shifts::RootObservation {
                    root: root.clone(),
                    source_function: [ordinal + 10; 32],
                    source_binding: [ordinal + 20; 32],
                    original_entry: format!("n-{root}"),
                    original_operation_owner: format!("n-{owner}"),
                    entry: format!("o-{root}"),
                    operation_owner: owner.clone(),
                    native_symbol: owner,
                }
            })
            .collect(),
    };
    Observation6 {
        case,
        before: endpoint([3; 32]),
        output: endpoint([4; 32]),
        original_order: roots.clone(),
        before_order: roots.clone(),
        output_order: roots,
        // Independent source roster order is joined by exact ID, not position.
        source_roots: ROOTS
            .iter()
            .enumerate()
            .map(|(ordinal, root)| census::SourceRoot {
                name: (*root).into(),
                function: [ordinal as u8 + 10; 32],
                body: [ordinal as u8 + 30; 32],
            })
            .collect(),
        erased_digest: None,
        policy: 6,
        passes: vec![
            "integer-neutral-canonicalization".into(),
            "dead-code-elimination".into(),
        ],
        pass_changed: vec![false, true],
        execution_sha256: [5; 32],
        continuation_sha256: [6; 32],
        replay_work: 7,
        simulation: simulation::masked_shift::inert_report_framing(case.batch, [4; 32]),
        llvm_sha256: [8; 32],
        llvm_bytes: 9,
        descriptor_roots: 2,
    }
}

#[test]
fn fixed6_masked_matrix_preserves_64_cases_128_roots_and_four_child_modes() {
    let cases = cases();
    assert_eq!(cases.len(), 64);
    assert_eq!(
        cases
            .iter()
            .map(|case| case.name())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        64
    );
    assert_eq!(cases.len() * ROOTS.len(), 128);
    assert_eq!(
        cases.len()
            * [
                Mode::Observe,
                Mode::Extract,
                Mode::ExtractCensus,
                Mode::MissingProof
            ]
            .len(),
        256
    );
    for case in cases {
        assert_eq!(Batch::parse(case.batch.name()), Some(case.batch));
        for reverse in [false, true] {
            let report = inert_observation(case, reverse);
            validate_observation(case, &report).unwrap();
            let subjects = observed_subjects(
                FixtureCase::MaskedShift(case),
                &Outcome::ObservedMasked(Box::new(report)),
            )
            .unwrap();
            assert_eq!((subjects.0, subjects.1, subjects.2.len()), ([8; 32], 9, 2));
        }
        let mut unchanged = inert_observation(case, false);
        unchanged.before.output_digest = unchanged.output.output_digest;
        unchanged.pass_changed = vec![false, false];
        validate_observation(case, &unchanged).unwrap();
    }
    // This is a separate matrix, not a replacement or enlargement of the
    // existing identity 64/320/32/256 denominator.
    let identities = super::super::cases();
    assert_eq!(
        (
            identities.len(),
            identities.iter().filter(|case| case.opt0).count()
        ),
        (64, 32)
    );
    assert_eq!(identities.len() * super::super::ROOTS.len(), 320);
}

#[test]
fn masked_fixed6_reports_reject_foreign_source_helpers_outputs_and_execution() {
    for name in ["masked-u32", "wrapping-u8-retained"] {
        let case = Case {
            batch: Batch::parse(name).unwrap(),
            target: Target::Gfx942,
        };
        let good = serde_json::to_value(inert_observation(case, true)).unwrap();
        for (path, replacement) in [
            ("/policy", serde_json::json!(5)),
            ("/erased_digest", serde_json::json!(vec![1; 32])),
            ("/before/source_digest/0", serde_json::json!(99)),
            ("/output/original_digest/0", serde_json::json!(99)),
            ("/output/output_digest/0", serde_json::json!(99)),
            ("/source_roots/0/function/0", serde_json::json!(99)),
            ("/source_roots/0/body", serde_json::json!(vec![0; 32])),
            ("/before/roots/0/source_function/0", serde_json::json!(99)),
            ("/before/roots/0/source_binding/0", serde_json::json!(99)),
            (
                "/before/roots/0/original_entry",
                serde_json::json!("foreign"),
            ),
            (
                "/before/roots/0/original_operation_owner",
                serde_json::json!("foreign"),
            ),
            ("/output/roots/0/entry", serde_json::json!("foreign")),
            (
                "/output/roots/0/operation_owner",
                serde_json::json!("foreign"),
            ),
            (
                "/output/roots/0/native_symbol",
                serde_json::json!("foreign"),
            ),
            ("/execution_sha256", serde_json::json!(vec![0; 32])),
            ("/continuation_sha256", serde_json::json!(vec![0; 32])),
            ("/passes/0", serde_json::json!("dead-code-elimination")),
            ("/pass_changed", serde_json::json!([true])),
            ("/replay_work", serde_json::json!(0)),
            ("/llvm_sha256", serde_json::json!(vec![0; 32])),
            ("/llvm_bytes", serde_json::json!(0)),
            ("/descriptor_roots", serde_json::json!(1)),
            (
                "/simulation/scenarios/0/grid",
                serde_json::json!([128, 1, 1]),
            ),
            (
                "/simulation/scenarios/0/workgroup",
                serde_json::json!([32, 1, 1]),
            ),
            (
                "/simulation/scenarios/0/deterministic_replays",
                serde_json::json!(1),
            ),
        ] {
            let mut changed = good.clone();
            *changed.pointer_mut(path).unwrap() = replacement;
            let report: Observation6 = serde_json::from_value(changed).unwrap();
            assert!(
                validate_observation(case, &report).is_err(),
                "{name} {path}"
            );
        }
        for path in [
            "/original_order",
            "/before_order",
            "/output_order",
            "/source_roots",
            "/before/roots",
            "/output/roots",
            "/simulation/scenarios",
        ] {
            for mutation in 0..3 {
                let mut changed = good.clone();
                let rows = changed.pointer_mut(path).unwrap().as_array_mut().unwrap();
                match mutation {
                    0 => {
                        rows.pop();
                    }
                    1 => rows[1] = rows[0].clone(),
                    2 => rows.push(rows[0].clone()),
                    _ => unreachable!(),
                }
                let report: Observation6 = serde_json::from_value(changed).unwrap();
                assert!(
                    validate_observation(case, &report).is_err(),
                    "{name} {path} {mutation}"
                );
            }
        }
        let mut report = inert_observation(case, false);
        report.before_order.reverse();
        assert!(validate_observation(case, &report).is_err());
        let mut report = inert_observation(case, false);
        report.output.batch.retained = !report.output.batch.retained;
        assert!(validate_observation(case, &report).is_err());
        let mut foreign = case;
        foreign.target = Target::Gfx950;
        assert!(validate_observation(foreign, &inert_observation(case, false)).is_err());
    }
}

#[test]
fn fixed6_child_protocol_binds_fixture_family_arguments_case_and_mode() {
    let case = cases()[0];
    let request = Request {
        case: FixtureCase::MaskedShift(case),
        mode: Mode::Observe,
        args_sha256: [7; 32],
        source: Vec::new(),
    };
    let framing = |request: Request| {
        serde_json::to_vec(&Report {
            request,
            result: Ok(Outcome::ObservedMasked(Box::new(inert_observation(
                case, false,
            )))),
        })
        .unwrap()
    };
    let good = framing(request.clone());
    assert!(matches!(
        decode_report(Some(0), Some(&good), &request),
        Ok(Outcome::ObservedMasked(_))
    ));
    for status in [None, Some(1), Some(101), Some(134), Some(137)] {
        assert!(decode_report(status, Some(&good), &request).is_err());
    }
    for bytes in [None, Some(&b"not-json"[..]), Some(&b"{}"[..])] {
        assert!(decode_report(Some(0), bytes, &request).is_err());
    }
    for foreign in [
        Request {
            args_sha256: [8; 32],
            ..request.clone()
        },
        Request {
            case: FixtureCase::MaskedShift(cases()[1]),
            ..request.clone()
        },
        Request {
            case: FixtureCase::IntegerIdentity(super::super::cases()[0]),
            ..request.clone()
        },
        Request {
            mode: Mode::Extract,
            ..request.clone()
        },
        Request {
            source: vec![FileStamp {
                path: "foreign".into(),
                sha256: [1; 32],
            }],
            ..request.clone()
        },
    ] {
        assert!(decode_report(Some(0), Some(&good), &foreign).is_err());
    }
    let identity = Request {
        case: FixtureCase::IntegerIdentity(super::super::cases()[0]),
        ..request.clone()
    };
    assert!(decode_report(Some(0), Some(&framing(identity.clone())), &identity).is_err());
    for mode in [Mode::Extract, Mode::ExtractCensus, Mode::MissingProof] {
        let request = Request {
            mode,
            ..request.clone()
        };
        assert!(decode_report(Some(0), Some(&framing(request.clone())), &request).is_err());
    }
    let refused = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Err("actual source refused".into()),
    })
    .unwrap();
    assert!(decode_report(Some(0), Some(&refused), &request).is_err());
    assert!(decode_report(Some(101), Some(&refused), &request).is_err());
    assert!(
        observed_subjects(
            identity.case,
            &Outcome::ObservedMasked(Box::new(inert_observation(case, false)))
        )
        .is_err()
    );
}

#[test]
fn fixed6_source_census_requires_the_current_closed_fixture_leaf() {
    let masked = FixtureCase::MaskedShift(cases()[0]);
    let identity = FixtureCase::IntegerIdentity(super::super::cases()[0]);
    let stamp = |case: FixtureCase, byte| FileStamp {
        path: PathBuf::from(format!("/workspace/{BASE}/src/{}", case.source_leaf())),
        sha256: [byte; 32],
    };
    let mask_stamp = stamp(masked, 1);
    let identity_stamp = stamp(identity, 2);
    assert_eq!(
        active_fixture_hash(&[mask_stamp.clone(), identity_stamp.clone()], masked),
        Ok([1; 32])
    );
    assert_eq!(
        active_fixture_hash(&[mask_stamp.clone(), identity_stamp.clone()], identity),
        Ok([2; 32])
    );
    assert!(active_fixture_hash(&[identity_stamp], masked).is_err());
    assert!(active_fixture_hash(std::slice::from_ref(&mask_stamp), identity).is_err());
    assert!(active_fixture_hash(&[mask_stamp.clone(), mask_stamp], masked).is_err());
    assert_eq!(masked.roots(), &ROOTS);
    assert_eq!(identity.roots(), &super::super::ROOTS);
}
