// Every relevant presence/error combination, partitioned to keep each solver
// obligation bounded. Corrupt roots exercise adapter retention, not reachability
// through Context prevalidation.
 selected_reader_witness_body {
    ($syntax:ident, $stable:expr, $producer:expr, $record:expr, $bad_stable:expr, $bad_producer:expr) => {
        $syntax!({
            let stable_present = $stable;
            let producer_present = $producer;
            let record_present = $record;
            let bad_stable = $bad_stable;
            let bad_producer = $bad_producer;
            let (actual, model, trace) = lifecycle_mixed_acquired_fixture_v1();
            proof { lifecycle_reader_reached_v1(trace@, actual, model); }
            let original_stable = actual.stable.leases[0].unwrap().reference;
            let original_producer = actual.reservations[0].unwrap().reference;
            let id = original_stable.consumer.local;
            let stable = ContextReadLeaseReferenceV1 { incarnation: if bad_stable { 0 } else { original_stable.incarnation },
                ..original_stable };
            let producer = ContextProducerReadReferenceV1 { incarnation: if bad_producer { 0 } else { original_producer.incarnation },
                ..original_producer };
            let stable_marker = if stable_present { Some(SelectedReaderMarkerV1 { first: stable, count: 1 }) } else { None };
            let producer_marker = if producer_present { Some(SelectedReaderMarkerV1 { first: producer, count: 1 }) } else { None };
            // An unrelated writer identity must not become an early input-release check.
            let writer = Some(WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
                context_generation: 7, local: 999, kind: WriterKindV1::Submission } });
            let mut state = SelectedReadersV1 {
                stable: SelectedEntryV1 { id, value: if stable_present {
                    Some(SelectedReaderRootV1 { marker: stable_marker, references: vec![stable] })
                } else { None } },
                producer: SelectedEntryV1 { id, value: if producer_present {
                    Some(SelectedReaderRootV1 { marker: producer_marker, references: vec![producer] })
                } else { None } },
                records: SelectedEntryV1 { id, value: if record_present {
                    Some(SelectedReaderRecordV1 { journal_read: stable_marker,
                        journal_producer_read: producer_marker, journal_writer: writer })
                } else { None } },
                journal: SelectedReaderJournalV1 { actual, model, last_model_result: Ok(()) },
            };
            proof {
                reveal_with_fuel(stable_release_scan_v1, 3);
                reveal_with_fuel(producer_release_scan_v1, 3);
                reveal_with_fuel(stable_released_leases_v1, 3);
            }
            let completed = state.release_inputs_v1(id);
            let stable_released = stable_present && !bad_stable;
            let producer_released = producer_present && !(stable_present && bad_stable) && !bad_producer;
            assert(completed == if stable_present && bad_stable || producer_present && bad_producer {
                Err(ReadErrorV1::InvalidReference)
            } else { Ok(()) });
            assert(state.stable.value.is_some() == (stable_present && !stable_released));
            assert(state.producer.value.is_some() == (producer_present && !producer_released));
            assert(state.journal.actual.stable.free_reads@ == if stable_released { seq![1usize, 0] } else { seq![1usize] });
            assert(state.journal.actual.free@ == if producer_released { seq![1usize, 0] } else { seq![1usize] });
            assert(state.journal.actual.stable.readers@ == if stable_released { seq![0usize, 0, 0] } else { seq![0usize, 1, 0] });
            assert(state.journal.actual.counts@ == if producer_released { seq![0usize, 0, 0] } else { seq![1usize, 0, 0] });
            assert(state.records.value.is_some() == record_present);
            if let Some(record) = state.records.value {
                assert(record.journal_read == if stable_released { None } else { stable_marker });
                assert(record.journal_producer_read == if producer_released { None } else { producer_marker });
                assert(record.journal_writer == writer);
            }
            true
        })
    };
}

verus! {

#[verifier::spinoff_prover]
fn selected_reader_success_witness_v1(stable_present: bool, producer_present: bool, record_present: bool)
    -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    hide(producer_represents);
    hide(logical::release_execution_relation_v1);
    hide(logical::producer_release_execution_relation_v1);
    selected_reader_witness_body!(verus_exec_expr, stable_present, producer_present, record_present, false, false)
}

#[verifier::spinoff_prover]
fn selected_reader_stable_error_witness_v1(producer_present: bool, record_present: bool, bad_producer: bool)
    -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    hide(producer_represents);
    hide(logical::release_execution_relation_v1);
    hide(logical::producer_release_execution_relation_v1);
    selected_reader_witness_body!(verus_exec_expr, true, producer_present, record_present, true, bad_producer)
}

#[verifier::spinoff_prover]
fn selected_reader_producer_error_witness_v1(stable_present: bool, record_present: bool)
    -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    hide(producer_represents);
    hide(logical::release_execution_relation_v1);
    hide(logical::producer_release_execution_relation_v1);
    selected_reader_witness_body!(verus_exec_expr, stable_present, true, record_present, false, true)
}

}
