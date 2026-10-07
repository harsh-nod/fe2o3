// Conditional post-access retirement suffix; no native-disposal authority.
include!("graph_reservation_retirement_definitions_v1.rs");
include!("../../fe2o3-runtime/src/context/graph/retirement_bodies.rs");
use vstd::std_specs::iter::IteratorSpec;

verus! {
broadcast use {vstd::seq_lib::group_seq_properties,
    vstd::map_lib::group_map_properties, vstd::set_lib::group_set_properties};

proof fn map_scan_equivalence<K, V>(map: Map<K, V>, sequence: Seq<&V>, predicate: spec_fn(V) -> bool)
    requires sequence.unref().to_set() == map.values(),
    ensures (exists|i: int| 0 <= i < sequence.len() && predicate(*sequence[i]))
        == (exists|key: K| map.contains_key(key) && predicate(map[key])),
{
    if exists|i: int| 0 <= i < sequence.len() && predicate(*sequence[i]) {
        let i = choose|i: int| 0 <= i < sequence.len() && predicate(*sequence[i]);
        assert(sequence.unref()[i] == *sequence[i]);
        assert(map.values().contains(*sequence[i]));
        let key = choose|key: K| map.contains_key(key) && map[key] == *sequence[i];
        assert(predicate(map[key]));
    }
    if exists|key: K| map.contains_key(key) && predicate(map[key]) {
        let key = choose|key: K| map.contains_key(key) && predicate(map[key]);
        assert(sequence.unref().to_set().contains(map[key]));
        let i = choose|i: int| 0 <= i < sequence.unref().len() && sequence.unref()[i] == map[key];
        assert(*sequence[i] == map[key]);
    }
}

impl<O> RuntimeReplicaStorageV1<O> {
    fn usage(&self) -> (out: RuntimeReplicaUsageV1)
        ensures out.capacity == self.slots.slots@.len(),
            out.pending == pending_count(self.slots.slots@, self.slots.slots@.len()),
            out.settled == settled_count(self.slots.slots@, self.slots.slots@.len()),
    {
        graph_replica_usage_body_v1!(verus_exec_expr, self, (index, pending, settled, count),
            [invariant
                count == self.slots.slots@.len(), index <= count, pending <= index,
                pending == pending_count(self.slots.slots@, index as nat),
             decreases count - index],
            [invariant
                count == self.slots.slots@.len(), index <= count, settled <= index,
                pending == pending_count(self.slots.slots@, count as nat),
                settled == settled_count(self.slots.slots@, index as nat),
             decreases count - index])
    }
}

impl<K: Eq + Hash, O> RuntimeContextV1<K, O> {
    fn has_unpublished_holds_v1(&self) -> (out: bool)
        requires vstd::std_specs::hash::obeys_key_model::<K>(),
        ensures out == self.unpublished(),
    {
        graph_unpublished_holds_body_v1!(verus_exec_expr, self, (record, values, found),
            [-> (matches: bool) ensures matches == record.unpublished.is_some()],
            [let ghost original = values.remaining();],
            [proof {
                if found {
                    let i = original.len() - values.remaining().len() - 1;
                    assert(0 <= i < original.len() && original[i].unpublished.is_some());
                } else {
                    assert forall|i: int| 0 <= i < original.len() implies
                        !original[i].unpublished.is_some() by {};
                }
                map_scan_equivalence(self.streams@, original,
                    |record: StreamRecordV1<O>| record.unpublished.is_some());
            }])
    }

    fn pending_replicas_v1(&self) -> (out: usize)
        ensures out == self.pending_replicas(),
    {
        graph_pending_replicas_body_v1!(verus_exec_expr, self)
    }

    fn release_graph_after_access_v1(&mut self, token: ContextGraphReservationV1)
        -> (out: Result<(), RuntimeValidationErrorV1>)
        requires vstd::std_specs::hash::obeys_key_model::<K>(),
        ensures
            final(self).retained_frame(*old(self)),
            out == if old(self).retirement_ready(token) { Ok(()) }
                else { Err(RuntimeValidationErrorV1::SubmissionPending) },
            out.is_err() ==> *final(self) == *old(self),
            out.is_ok() ==> final(self).graph_reservation == None,
    {
        proof {
            assert(vstd::std_specs::hash::spec_hash_map_len(&self.submissions) == self.submissions@.dom().len());
            assert(vstd::std_specs::hash::spec_hash_map_len(&self.events) == self.events@.dom().len());
            assert(self.submissions@.dom().is_empty() == (self.submissions@.dom().len() == 0));
            assert(self.events@.dom().is_empty() == (self.events@.dom().len() == 0));
            assert(self.generated_issues@.is_empty() == self.generated_issues@.dom().is_empty());
        }
        graph_reservation_retirement_body_v1!(verus_exec_expr, self, token, (stream, values, found),
            [-> (matches: bool) ensures matches == stream.generated.is_some()],
            [let ghost original = values.remaining();],
            [proof {
                if found {
                    let i = original.len() - values.remaining().len() - 1;
                    assert(0 <= i < original.len() && original[i].generated.is_some());
                } else {
                    assert forall|i: int| 0 <= i < original.len() implies
                        !original[i].generated.is_some() by {};
                }
                map_scan_equivalence(self.streams@, original,
                    |record: StreamRecordV1<O>| record.generated.is_some());
                assert(found == self.generated());
            }])
    }
}

// Actual executable constructors/calls inhabit the successful suffix and each
// retained stream/table scan. These are inert model owners, not native handles.
fn retirement_cases_are_inhabited(mode: u8)
    requires mode < 5,
{
    let token = ContextGraphReservationV1 { context_generation: 7, local: 11 };
    let replicas = if mode >= 3 {
        Some(RuntimeReplicaStorageV1 { slots: HostMetadataTableV1 {
            slots: vec![ReplicaSlotV1 { incarnation: 1,
                state: if mode == 3 { ReplicaStateV1::Pending(21u64) }
                    else { ReplicaStateV1::Settled(31u64) } }],
            credits: Some(91u64),
        } })
    } else { None };
    let mut context = RuntimeContextV1::<u64, u64> {
        scope_epoch: 1, replicas, backend: 2, context_generation: 7,
        devices: vec![3], streams: HashMap::new(), backend_streams: HashSet::new(),
        allocations: HashMap::new(), backend_allocations: HashSet::new(), allocation_admission: 4,
        versions: Some(5), modules: HashMap::new(), backend_modules: HashSet::new(),
        kernels: HashMap::new(), events: HashMap::new(), backend_events: HashSet::new(),
        submissions: HashMap::new(), backend_submissions: HashSet::new(),
        scalar_peer_copies: HashMap::new(), producer_launches: HashMap::new(),
        same_device_copies: HashMap::new(), segmented_peer_copies: HashMap::new(),
        generated_issues: HashMap::new(), completion_callbacks: HashMap::new(),
        completion_callback_count: 0, completion_callback_panic_count: 17,
        next_identity: 19, terminal: false, graph_reservation: Some(token),
        native_pair_reservation: None, graph_issue_closed: true,
    };
    context.streams.insert(23, StreamRecordV1 {
        backend_stream: 29, device: 3,
        unpublished: if mode == 1 { Some(37) } else { None },
        generated: if mode == 2 { Some(41) } else { None },
    });
    proof { reveal_with_fuel(pending_count, 2); }
    assert(context.streams@.dom() =~= set![23u64]);
    assert(context.streams@[23u64].unpublished.is_some() == (mode == 1));
    assert(context.streams@[23u64].generated.is_some() == (mode == 2));
    assert forall|key: u64| context.streams@.contains_key(key) implies
        (context.streams@[key].unpublished.is_some() == (mode == 1)
            && context.streams@[key].generated.is_some() == (mode == 2)) by {
        assert(key == 23);
    };
    if mode == 1 { assert(context.streams@.contains_key(23u64)
        && context.streams@[23u64].unpublished.is_some()); }
    if mode == 2 { assert(context.streams@.contains_key(23u64)
        && context.streams@[23u64].generated.is_some()); }
    assert(context.graph_reservation == Some(token));
    assert(context.pending_replicas() == if mode == 3 { 1nat } else { 0nat });
    assert(context.unpublished() == (mode == 1));
    assert(context.generated() == (mode == 2));
    assert(context.retirement_ready(token) == (mode == 0 || mode == 4));
    let ghost before = context;
    let result = context.release_graph_after_access_v1(token);
    assert(result.is_ok() == (mode == 0 || mode == 4));
    assert(context.retained_frame(before));
    if result.is_err() { assert(context.graph_reservation == Some(token)); }
}
}
