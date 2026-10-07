use super::*;

#[test]
fn snapshot_atomic_and_collective_routes_forward_input_custody() {
    let mut f = Fixture::new(2);
    let args = f.reads();
    let submission = f
        .context
        .launch_snapshot_v1(
            f.stream,
            &f.kernel,
            &args.encode_explicit_kernarg_v1(),
            &args.bindings_v1(),
            geometry(),
            &[],
        )
        .unwrap();
    finish_forwarded(f, submission, args);

    let mut f = Fixture::new(2);
    f.context.backend.execution_capabilities.atomics = true;
    let args = f.reads();
    let contract = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Add,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::Relaxed,
        failure_order: None,
        weak: false,
        geometry: geometry(),
    };
    let submission = f
        .context
        .launch_atomic(f.stream, &f.kernel, &args, contract, &[])
        .unwrap();
    assert_eq!(f.context.backend.last_atomic_contract, Some(contract));
    finish_forwarded(f, submission, args);

    let mut f = Fixture::new(2);
    f.context.backend.execution_capabilities.collectives = true;
    let args = f.reads();
    let contract = RuntimeCollectiveLaunchContractV1 {
        operation: RuntimeCollectiveOperationV1::ReduceSum,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::AcquireRelease,
        participants: 64,
        geometry: geometry(),
    };
    let submission = f
        .context
        .launch_collective(f.stream, &f.kernel, &args, contract, &[])
        .unwrap();
    assert_eq!(f.context.backend.last_collective_contract, Some(contract));
    finish_forwarded(f, submission, args);
}

#[test]
fn malformed_read_only_handles_retain_all_consumers_and_charge_each_source_once() {
    for duplicate in [false, true] {
        let mut f = Fixture::new(4);
        let args = f.reads();
        let handle = if duplicate {
            let first = f.launch(args.0.clone());
            f.context
                .on_completion(&first, |_| panic!("ambiguous handle must not notify"))
                .unwrap();
            first.backend_submission
        } else {
            0
        };
        f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(
            f.context
                .launch(f.stream, &f.kernel, &args, geometry(), &[]),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        assert!(f.context.is_terminal());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        let retained = 2 * (1 + usize::from(duplicate));
        assert_eq!(f.context.version_journal_read_records_v1(), Some(retained));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        let usage = f
            .context
            .allocation_admission_usage_v1(f.context.devices()[0].id())
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, 3);
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::AllocationRecords),
            3
        );
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
            192
        );
        let pending = f.context.backend.pending_kernel_reads.clone();
        assert_eq!(pending[&handle].bindings, f.backend_bindings(&args));
        for _ in 0..2 {
            let report = f.context.cleanup();
            assert!(!report.is_complete());
            assert_eq!(report.reader_journal_records_v1(), retained);
        }
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert_eq!(f.context.backend.pending_kernel_reads, pending);
        assert!(f.context.backend.cleanup_log.is_empty());
    }
}

#[test]
fn ambiguous_observation_keeps_pure_readers_with_or_without_admission_credits() {
    for credits in [false, true] {
        for route in 0..3 {
            let mut f = Fixture::configured(2, true, credits);
            let args = f.reads();
            let mut submission = f.launch(args.0.clone());
            let result = catch_unwind(AssertUnwindSafe(|| match route {
                0 => {
                    f.context.backend.first_wait_failure = MockWaitFailure::TerminalFirst;
                    f.context.wait(&mut submission, Duration::ZERO).map(|_| ())
                }
                _ => {
                    f.context.backend.cancel_failure = if route == 1 {
                        MockMemoryFailure::Terminal
                    } else {
                        MockMemoryFailure::Panic
                    };
                    f.context.cancel(&mut submission).map(|_| ())
                }
            }));
            match (route, result) {
                (0, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                    assert_eq!(error.0, "wait terminal")
                }
                (1, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                    assert_eq!(error.0, "allocation terminal failure")
                }
                (2, Err(payload)) => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"scripted allocation adapter panic")
                ),
                _ => panic!("original observation failure was replaced"),
            }
            assert!(f.context.is_terminal());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
            assert!(f.context.backend.observed_kernel_reads.is_empty());
            let pending = f.context.backend.pending_kernel_reads.clone();
            assert_eq!(
                pending[&submission.backend_submission].bindings,
                f.backend_bindings(&args)
            );
            let usage = f
                .context
                .allocation_admission_usage_v1(f.context.devices()[0].id())
                .unwrap();
            if credits {
                let usage = usage.unwrap();
                assert_eq!(usage.quarantined_records, 3);
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::AllocationRecords),
                    3
                );
            } else {
                assert!(usage.is_none());
            }
            assert!(!f.context.cleanup().is_complete());
            assert!(!f.context.cleanup().is_complete());
            assert_eq!(f.context.backend.pending_kernel_reads, pending);
            assert!(f.context.backend.cleanup_log.is_empty());
        }
    }
}

#[test]
fn callback_panic_and_repeated_observation_cannot_release_a_later_read_batch() {
    let mut f = Fixture::new(2);
    let args = f.reads();
    let mut first = f.launch(args.0.clone());
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let callback_observed = observed.clone();
    f.context
        .on_completion(&first, move |status| {
            callback_observed.lock().unwrap().push(status);
            panic!("after kernel read completion");
        })
        .unwrap();
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    f.assert_observed(first.backend_submission, &args);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    assert_eq!(
        *observed.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    let mut next = f.launch(args.0.clone());
    let calls = f.context.backend.wait_call_count;
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    assert_eq!(f.context.backend.wait_call_count, calls);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    assert_eq!(
        *observed.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    f.context.wait(&mut next, Duration::ZERO).unwrap();
    f.assert_observed(next.backend_submission, &args);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn dropped_reader_token_keeps_inputs_until_cleanup_quiesces_its_stream() {
    for credits in [false, true] {
        let mut f = Fixture::configured(2, true, credits);
        let args = f.reads();
        let backend_submission = {
            let submission = f.launch(args.0.clone());
            submission.backend_submission
        };
        assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        reserved(f.context.release_allocation(f.allocations[0]));
        let bindings = f.backend_bindings(&args);
        let report = f.context.cleanup();
        assert!(report.is_complete());
        assert_eq!(report.reader_journal_records_v1(), 0);
        assert!(f.context.backend.pending_kernel_reads.is_empty());
        let expected: Vec<_> = bindings
            .into_iter()
            .enumerate()
            .map(|(ordinal, binding)| MockObservedKernelRead {
                submission: backend_submission,
                ordinal,
                binding,
                bytes: pattern(ordinal)[8..16].to_vec(),
            })
            .collect();
        assert_eq!(f.context.backend.observed_kernel_reads, expected);
        assert!(f.context.backend.memory.is_empty());
    }
}

#[test]
fn stale_internal_preparation_retains_provisional_root_before_any_backend_submission() {
    let mut f = Fixture::new(2);
    let sources: Vec<_> = f.allocations[..2]
        .iter()
        .map(|&id| ContextReadSourceV1 {
            region: RuntimeMemoryRegionV1 {
                allocation: id,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            record: f.context.allocations[&id],
        })
        .collect();
    let prepared = f.context.prepare_submission_readers_v1(&sources).unwrap();
    // Deliberately split the private preflight/Begin phases; no public race is claimed.
    f.context
        .write_allocation(f.allocations[0], 0, &[9])
        .unwrap();
    let neighbor = state(&f.context, f.allocations[2]);
    let id = RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_id().unwrap());
    assert_eq!(
        f.context
            .begin_submission_readers_v1(id, prepared, SubmissionWriterDomainV1::Ordinary)
            .unwrap_err(),
        RuntimeValidationErrorV1::InvalidBackendDescription
    );
    assert!(f.context.is_terminal());
    assert!(f.context.submissions.is_empty());
    assert_eq!(f.context.backend.submit_count, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .retained_read_count(),
        0
    );
    let originals = f
        .context
        .versions
        .as_ref()
        .unwrap()
        .submission_reader_sources_for_test_v1(id);
    assert_eq!(originals.len(), sources.len());
    for (actual, expected) in originals.iter().zip(&sources) {
        assert_eq!(actual.region, expected.region);
        assert_eq!(actual.record, expected.record);
    }
    let usage = f
        .context
        .allocation_admission_usage_v1(f.context.devices()[0].id())
        .unwrap()
        .unwrap();
    assert_eq!(usage.quarantined_records, 3);
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::AllocationRecords),
        3
    );
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
        192
    );
    assert_eq!(state(&f.context, f.allocations[2]), neighbor);
    for _ in 0..2 {
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert_eq!(report.reader_journal_records_v1(), 1);
    }
    assert!(f.context.backend.cleanup_log.is_empty());
}
