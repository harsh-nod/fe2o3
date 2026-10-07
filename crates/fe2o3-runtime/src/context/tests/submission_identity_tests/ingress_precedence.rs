use super::*;

#[test]
fn wait_validates_identity_before_deadline_but_drain_checks_expiration_first() {
    for completed in [false, true] {
        for coordinate in COORDINATES {
            let mut fixture = Fixture::new(completed);
            let mut token = fixture.substitute(coordinate);
            let before_token = token_snapshot(&token);
            let before = snapshot(&fixture.context, &fixture.probes);
            assert!(matches!(
                fixture.context.wait(&mut token, Duration::MAX),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::UnknownSubmission
                ))
            ));
            assert_eq!(token_snapshot(&token), before_token);
            assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
            assert!(matches!(
                fixture.context.drain(&mut token, Instant::now()),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            ));
            assert_eq!(token_snapshot(&token), before_token);
            assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
            fixture.cleanup();
        }
        let mut fixture = Fixture::new(completed);
        let before = snapshot(&fixture.context, &fixture.probes);
        let mut token = copy_token(&fixture.target);
        let result = fixture.context.wait(&mut token, Duration::MAX);
        if completed {
            assert_eq!(result.unwrap(), RuntimePollV1::Succeeded);
            assert_eq!(token.completion, Some(RuntimePollV1::Succeeded));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            ));
            assert_eq!(token.completion, None);
        }
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
        let before_token = token_snapshot(&token);
        assert!(matches!(
            fixture.context.drain(&mut token, Instant::now()),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        ));
        assert_eq!(token_snapshot(&token), before_token);
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
        fixture.cleanup();
    }
}

#[test]
fn actual_terminal_backend_failure_precedes_ingress_identity_except_pure_query() {
    for completed in [false, true] {
        for ingress in INGRESSES {
            for coordinate in COORDINATES.into_iter().map(Some).chain([None]) {
                let mut fixture = Fixture::new(completed);
                fixture.context.backend.flush_failure = MockFlushFailure::Terminal;
                assert!(matches!(
                    fixture.context.flush_stream(fixture.target.stream),
                    Err(RuntimeErrorV1::BackendTerminal(_))
                ));
                assert!(fixture.context.is_terminal());
                let token = coordinate
                    .map_or_else(|| copy_token(&fixture.target), |c| fixture.substitute(c));
                if ingress == Ingress::Wait {
                    reject_deadlines_at_context_gate(
                        &mut fixture.context,
                        &fixture.probes,
                        &token,
                        RuntimeValidationErrorV1::ContextTerminal,
                    );
                }
                if ingress == Ingress::Query && coordinate.is_none() {
                    let before = snapshot(&fixture.context, &fixture.probes);
                    assert_eq!(
                        fixture.context.query_submission(&token),
                        Ok(if completed {
                            RuntimeCompletionStatusV1::Succeeded
                        } else {
                            RuntimeCompletionStatusV1::Pending
                        })
                    );
                    assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
                } else {
                    reject(
                        &mut fixture,
                        Some(token),
                        ingress,
                        if ingress == Ingress::Query {
                            RuntimeValidationErrorV1::UnknownSubmission
                        } else {
                            RuntimeValidationErrorV1::ContextTerminal
                        },
                    );
                }
                let before = snapshot(&fixture.context, &fixture.probes);
                let report = fixture.context.cleanup();
                assert!(report.is_terminal());
                assert!(!report.is_complete());
                assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
                assert!(fixture.foreign.cleanup().is_complete());
                let probes = fixture.probes.clone();
                drop(fixture);
                assert_eq!(counts(&probes[0]), (usize::from(completed), 1));
                assert_eq!(counts(&probes[1]), (0, 1));
            }
        }
    }
}

#[test]
fn genuine_graph_reservation_preserves_query_and_public_ingress_precedence() {
    for terminal in [false, true] {
        for ingress in INGRESSES {
            for foreign_token in [false, true] {
                let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
                let mut foreign = RuntimeContextV1::open(MockBackend::default()).unwrap();
                let args = AddArguments {
                    allocation,
                    scalar: 3,
                };
                let action = context
                    .prepare_graph_launch_v1(
                        stream,
                        &kernel,
                        &args.encode_explicit_kernarg_v1(),
                        &args.bindings_v1(),
                        geometry(),
                    )
                    .unwrap();
                let reservation = context.reserve_graph_v1(1).unwrap();
                let mut submission = context.submit_graph_action_v1(reservation, action).unwrap();
                assert_eq!(context.submissions.len(), 1);
                assert_eq!(context.backend.submit_count, 1);
                if terminal {
                    context.backend.flush_failure = MockFlushFailure::Terminal;
                    assert!(matches!(
                        context.flush_with_graph_access_v1(stream, Some(reservation)),
                        Err(RuntimeErrorV1::BackendTerminal(_))
                    ));
                }
                let mut token = copy_token(&submission.original);
                if foreign_token {
                    token.id.context_generation = foreign.context_generation;
                }
                if ingress == Ingress::Wait {
                    reject_deadlines_at_context_gate(
                        &mut context,
                        &[],
                        &token,
                        if terminal {
                            RuntimeValidationErrorV1::ContextTerminal
                        } else {
                            RuntimeValidationErrorV1::ContextReserved
                        },
                    );
                }
                if ingress == Ingress::Query && !foreign_token {
                    let before = snapshot(&context, &[]);
                    assert_eq!(
                        context.query_submission(&token),
                        Ok(RuntimeCompletionStatusV1::Pending)
                    );
                    assert_eq!(snapshot(&context, &[]), before);
                } else {
                    reject_context(
                        &mut context,
                        &[],
                        Some(token),
                        ingress,
                        if ingress == Ingress::Query {
                            RuntimeValidationErrorV1::UnknownSubmission
                        } else if terminal {
                            RuntimeValidationErrorV1::ContextTerminal
                        } else {
                            RuntimeValidationErrorV1::ContextReserved
                        },
                    );
                }
                if terminal {
                    let before = snapshot(&context, &[]);
                    let report = context.cleanup();
                    assert!(report.is_terminal());
                    assert!(report.is_graph_reserved());
                    assert_eq!(snapshot(&context, &[]), before);
                } else {
                    assert_eq!(
                        context
                            .poll_with_graph_access_v1(&mut submission.original, Some(reservation))
                            .unwrap(),
                        RuntimePollV1::Pending
                    );
                    assert_eq!(
                        context
                            .poll_with_graph_access_v1(&mut submission.original, Some(reservation))
                            .unwrap(),
                        RuntimePollV1::Succeeded
                    );
                    context
                        .release_graph_submission_v1(reservation, &submission)
                        .unwrap();
                    context.close_graph_issue_v1(reservation).unwrap();
                    context.release_graph_v1(reservation).unwrap();
                    assert!(context.cleanup().is_complete());
                }
                assert!(foreign.cleanup().is_complete());
            }
        }
    }
}
