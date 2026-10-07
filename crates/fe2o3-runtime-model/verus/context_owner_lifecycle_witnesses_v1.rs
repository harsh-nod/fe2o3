// Concrete paired executions witness the mixed trace grammar; they do not replace induction.
verus! {

#[verifier::spinoff_prover]
fn lifecycle_constructor_origin_fixture_v1()
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleOriginV1>))
    ensures producer_represents(result.0, result.1),
        constructor_producer_initialized_v1(result.0, 7, 3, 1, 2),
        lifecycle_origin_relation_v1(result.2@), lifecycle_origin_input_shape_v1(result.2@),
        result.2@.actual_result == Ok(result.0), result.2@.model_result == Ok(result.1),
        result.2@.context == 7, result.2@.allocations == 3, result.2@.writers == 1, result.2@.reads == 2,
{
    let mut actual_observations = ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true],
        attempts: vec![],
    };
    let mut model_observations = logical::ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true],
        attempts: vec![],
    };
    let ghost actual_before = actual_observations;
    let ghost model_before = model_observations;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let constructed = constructor_producer_paired_exec_v1(&mut actual_observations, &mut model_observations,
        7, 3, 1, 2, Ghost(storage));
    assert(constructed.0.is_ok() && constructed.1.is_ok());
    let ghost origin = LifecycleOriginV1 {
        actual_before, actual_after: actual_observations, model_before, model_after: model_observations,
        context: 7, allocations: 3, writers: 1, reads: 2, actual_result: constructed.0, model_result: constructed.1,
    };
    let actual = constructed.0.unwrap();
    let model = match constructed.1 { Ok(owner) => owner, Err(_) => { assert(false); unreached() } };
    (actual, model, Ghost(origin))
}

#[verifier::spinoff_prover]
proof fn lifecycle_three_event_trace_v1(origin: LifecycleOriginV1,
    actual_states: Seq<ContextProducerReadJournalV1>, model_states: Seq<logical::ProducerReadContentsV1>,
    events: Seq<LifecyclePairedEventV1>, storage: logical::StorageCapacitiesV1)
    requires lifecycle_origin_relation_v1(origin), lifecycle_origin_input_shape_v1(origin),
        logical::storage_admission_v1(origin.allocations, origin.writers, storage),
        actual_states.len() == 4, model_states.len() == 4, events.len() == 3,
        origin.actual_result == Ok(actual_states[0]), origin.model_result == Ok(model_states[0]),
        lifecycle_event_relation_v1(actual_states[0], actual_states[1], model_states[0], model_states[1], events[0]),
        lifecycle_event_relation_v1(actual_states[1], actual_states[2], model_states[1], model_states[2], events[1]),
        lifecycle_event_relation_v1(actual_states[2], actual_states[3], model_states[2], model_states[3], events[2]),
        lifecycle_event_input_shape_v1(events[0]), lifecycle_event_input_shape_v1(events[1]), lifecycle_event_input_shape_v1(events[2]),
    ensures lifecycle_paired_trace_v1(origin, actual_states, model_states, events),
        logical::lifecycle_invariant_v1(model_states[3], storage, lifecycle_paired_history_v1(events, 3)),
        forall|i: int| 0 <= i < 3 ==> lifecycle_event_answers_v1(#[trigger] events[i]),
        forall|i: int| 0 <= i < 3 ==> lifecycle_event_domain_v1(actual_states[i], #[trigger] events[i]),
{
    assert forall|i: int| 0 <= i < events.len() implies lifecycle_event_relation_v1(
        actual_states[i], actual_states[i + 1], model_states[i], model_states[i + 1], #[trigger] events[i]) by {
        if i == 0 {} else if i == 1 {} else { assert(i == 2); }
    }
    assert(lifecycle_paired_trace_v1(origin, actual_states, model_states, events));
    lifecycle_constructor_origin_trace_v1(origin, actual_states, model_states, events, storage);
    assert forall|i: int| 0 <= i < events.len() implies lifecycle_event_input_shape_v1(#[trigger] events[i]) by {
        if i == 0 {} else if i == 1 {} else { assert(i == 2); }
    }
    lifecycle_reached_operation_domains_v1(origin, actual_states, model_states, events, storage);
    lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, 3);
}

spec fn lifecycle_registered_writer_fixture_v1(actual: ContextProducerReadJournalV1,
    reference: WriterReferenceV1) -> bool
{
    &&& reference.slot == 0
    &&& reference.key == WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Submission }
    &&& actual.stable.journal.context_generation == 7
    &&& actual.stable.journal.writer_capacity == 1
    &&& actual.stable.journal.registration_watermark == 5
    &&& actual.stable.journal.reserved_count == 1
    &&& actual.stable.journal.writers@.len() == 1
    &&& actual.stable.journal.writers@[0] == Some(WriterEntryV1::Reserved(reference.key))
    &&& actual.stable.journal.free@.len() == 0
}

#[verifier::spinoff_prover]
fn lifecycle_register_fixture_step_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1)
    -> (result: (WriterReferenceV1, logical::WriterReferenceV1, Ghost<LifecyclePairedEventV1>))
    requires producer_represents(*old(actual), *old(model)),
        constructor_producer_initialized_v1(*old(actual), 7, 3, 1, 2),
    ensures producer_represents(*final(actual), *final(model)),
        writer_reference_view(result.0) == result.1,
        lifecycle_registered_writer_fixture_v1(*final(actual), result.0),
        lifecycle_event_relation_v1(*old(actual), *final(actual), *old(model), *final(model), result.2@),
        lifecycle_event_input_shape_v1(result.2@),
        result.2@ == (LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::Register { key: result.0.key, result: Ok(result.0) },
            model: logical::LifecycleStepV1::Register { key: result.1.key, result: Ok(result.1) },
        }),
{
    hide(logical::issued_producer_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::producer_status_v1);
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let key = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 5, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(actual, model, key, model_key,
        Ghost(storage), Ghost(Seq::empty()));
    assert(registered.0.is_ok() && registered.1.is_ok());
    let ghost registration = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Register { key, result: registered.0 },
        model: logical::LifecycleStepV1::Register { key: model_key, result: registered.1 },
    };
    assert(lifecycle_event_relation_v1(before, *actual, model_before, *model, registration));
    let reference = registered.0.unwrap();
    let model_reference = match registered.1 { Ok(value) => value, Err(_) => { assert(false); unreached() } };
    (reference, model_reference, Ghost(registration))
}

#[verifier::spinoff_prover]
fn lifecycle_inspection_fixture_step_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1) -> (result: Ghost<LifecyclePairedEventV1>)
    requires producer_represents(*old(actual), *old(model)),
    ensures *final(actual) == *old(actual), *final(model) == *old(model),
        producer_represents(*final(actual), *final(model)),
        lifecycle_event_relation_v1(*old(actual), *final(actual), *old(model), *final(model), result@),
        lifecycle_event_input_shape_v1(result@),
        match result@ {
            LifecyclePairedEventV1::Observe { query, .. } => query == LifecycleQueryV1::InspectProducer { explicit: false },
            _ => false,
        },
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let inspected = inspection_producer_auto_paired_exec_v1(actual, model);
    let ghost inspection = LifecyclePairedEventV1::Observe {
        query: LifecycleQueryV1::InspectProducer { explicit: false },
        actual_answer: LifecycleAnswerV1::Owner(inspected.0), model_answer: LifecycleAnswerV1::Owner(inspected.1),
    };
    assert(lifecycle_event_relation_v1(before, *actual, model_before, *model, inspection));
    Ghost(inspection)
}

#[verifier::spinoff_prover]
fn lifecycle_abort_fixture_step_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1,
    reference: WriterReferenceV1, model_reference: logical::WriterReferenceV1)
    -> (result: Ghost<LifecyclePairedEventV1>)
    requires producer_represents(*old(actual), *old(model)),
        writer_reference_view(reference) == model_reference,
        lifecycle_registered_writer_fixture_v1(*old(actual), reference),
    ensures producer_represents(*final(actual), *final(model)),
        lifecycle_event_relation_v1(*old(actual), *final(actual), *old(model), *final(model), result@),
        lifecycle_event_input_shape_v1(result@),
        final(actual).stable.journal.reserved_count == 0,
        final(model).stable.journal.registration_watermark == 5,
        result@ == (LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::Abort { reference, capacity: 1, result: Ok(()) },
            model: logical::LifecycleStepV1::Abort { reference: model_reference, capacity: 1, result: Ok(()) },
        }),
{
    hide(logical::issued_producer_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::producer_status_v1);
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost registered_history = seq![model_reference];
    let aborted = owner_abort_historical_exec_v1(actual, model, reference, model_reference, 1,
        Ghost(storage), Ghost(registered_history));
    assert(aborted.0 == Ok(()) && aborted.1 == Ok(()));
    let ghost abortion = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Abort { reference, capacity: 1, result: aborted.0 },
        model: logical::LifecycleStepV1::Abort { reference: model_reference, capacity: 1, result: aborted.1 },
    };
    assert(lifecycle_event_relation_v1(before, *actual, model_before, *model, abortion));
    Ghost(abortion)
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_trace_witness_v1() -> (result: bool)
    ensures result,
{
    // The trace theorem owns invariant preservation; this wrapper records raw calls.
    hide(logical::lifecycle_invariant_v1);
    hide(producer_represents);
    hide(constructor_producer_initialized_v1);
    hide(lifecycle_registered_writer_fixture_v1);
    hide(lifecycle_origin_relation_v1);
    hide(lifecycle_origin_input_shape_v1);
    hide(lifecycle_paired_trace_v1);
    hide(lifecycle_event_relation_v1);
    hide(lifecycle_event_input_shape_v1);
    hide(lifecycle_event_answers_v1);
    hide(lifecycle_event_domain_v1);
    let (mut actual, mut model, origin_snapshot) = lifecycle_constructor_origin_fixture_v1();
    let ghost origin = origin_snapshot@;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost a0 = actual;
    let ghost m0 = model;
    let (reference, model_reference, registration_snapshot) = lifecycle_register_fixture_step_v1(&mut actual, &mut model);
    let ghost a1 = actual;
    let ghost m1 = model;
    let ghost registration = registration_snapshot@;
    assert(lifecycle_event_relation_v1(a0, a1, m0, m1, registration));

    let inspection_snapshot = lifecycle_inspection_fixture_step_v1(&mut actual, &mut model);
    let ghost a2 = actual;
    let ghost m2 = model;
    let ghost inspection = inspection_snapshot@;
    assert(lifecycle_event_relation_v1(a1, a2, m1, m2, inspection));

    let abortion_snapshot = lifecycle_abort_fixture_step_v1(&mut actual, &mut model, reference, model_reference);
    let ghost a3 = actual;
    let ghost m3 = model;
    let ghost abortion = abortion_snapshot@;
    assert(lifecycle_event_relation_v1(a2, a3, m2, m3, abortion));

    proof {
        let actual_states = seq![a0, a1, a2, a3];
        let model_states = seq![m0, m1, m2, m3];
        let events = seq![registration, inspection, abortion];
        lifecycle_three_event_trace_v1(origin, actual_states, model_states, events, storage);
        assert(events.len() == 3);
        reveal_with_fuel(lifecycle_paired_history_v1, 4);
        assert(lifecycle_paired_history_v1(events, 3).writers == seq![model_reference]);
        assert(actual.stable.journal.reserved_count == 0);
        assert(model.stable.journal.registration_watermark == 5);
    }
    true
}

#[verifier::spinoff_prover]
fn lifecycle_failed_constructor_trace_witness_v1() -> (result: bool)
    ensures result,
{
    let mut actual = ConstructorObservationsV1 { outcomes: vec![], attempts: vec![(99u8, 42usize)] };
    let mut model = logical::ConstructorObservationsV1 { outcomes: vec![], attempts: vec![(99u8, 42usize)] };
    let ghost actual_before = actual;
    let ghost model_before = model;
    // This ghost storage is deliberately insufficient: failure correspondence needs no admission.
    let ghost storage = logical::witness_storage_v1(0, 0);
    let constructed = constructor_producer_paired_exec_v1(&mut actual, &mut model, 7, 3, 1, 2, Ghost(storage));
    proof {
        owner_constructor_first_failure_v1(actual_before, actual, constructor_requests_v1(3, 1, 2),
            0, constructed.0.is_ok());
    }
    assert(constructed.0 == Err(ReadErrorV1::StorageAllocationFailed));
    assert(constructed.1 == Err(logical::ConstructorErrorV1::StorageAllocationFailed));
    let ghost origin = LifecycleOriginV1 {
        actual_before, actual_after: actual, model_before, model_after: model,
        context: 7, allocations: 3, writers: 1, reads: 2, actual_result: constructed.0, model_result: constructed.1,
    };
    proof {
        let actual_states = Seq::<ContextProducerReadJournalV1>::empty();
        let model_states = Seq::<logical::ProducerReadContentsV1>::empty();
        let events = Seq::<LifecyclePairedEventV1>::empty();
        assert(lifecycle_paired_trace_v1(origin, actual_states, model_states, events));
        lifecycle_constructor_origin_trace_v1(origin, actual_states, model_states, events, storage);
        lifecycle_reached_operation_domains_v1(origin, actual_states, model_states, events, storage);
        assert(actual.attempts@ =~= seq![(99u8, 42usize), (0u8, 1usize)]);
        assert(actual.attempts@ == model.attempts@);
    }
    true
}

}
