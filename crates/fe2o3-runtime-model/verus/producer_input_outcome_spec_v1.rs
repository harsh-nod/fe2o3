// Shared per-input outcome formulas, membership scans and correspondence lemmas.
verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn allocation_less(a: RuntimeAllocationIdV1, b: RuntimeAllocationIdV1) -> bool {
    a.context_generation < b.context_generation
        || a.context_generation == b.context_generation && a.local < b.local
}
impl vstd::std_specs::cmp::PartialOrdSpecImpl for RuntimeAllocationIdV1 {
    open spec fn obeys_partial_cmp_spec() -> bool { true }
    closed spec fn partial_cmp_spec(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.context_generation < other.context_generation
            || self.context_generation == other.context_generation && self.local < other.local
        { Some(core::cmp::Ordering::Less) }
        else if *self == *other { Some(core::cmp::Ordering::Equal) }
        else { Some(core::cmp::Ordering::Greater) }
    }
}

fn enrollment(id: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1, byte_extent: u64)
    -> (out: ContextAllocationEnrollmentV1)
    ensures out == (ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 { context_generation: id.context_generation, local: id.local },
        device: ContextJournalDeviceKeyV1 { context_generation: device.context_generation, local: device.local },
        byte_extent }),
{
    context_allocation_enrollment_body_v1!(id, device, byte_extent)
}

type StatusResult = Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>;
type Payload = (ContextAllocationWriteV1, u64, u64, ContextWriterReferenceV1, ContextProducerReadStatusV1);
struct Header {
    result: Result<Payload, ContextVersionJournalErrorV1>, active: usize, queued: usize, calls: Seq<Call>,
}
struct Outcome { result: StatusResult, active: usize, queued: usize, calls: Seq<Call> }

fn producer_dependency_contains_v1(dependencies: &[ScalarPeerDependencyV1], dependency: &ScalarPeerDependencyV1)
    -> (found: bool)
    ensures found == dependencies@.contains(*dependency),
{
    producer_dependency_contains_body!(verus_exec_expr, dependencies, dependency, index,
        [invariant
            index <= dependencies.len(),
            forall|j: int| 0 <= j < index ==> dependencies@[j] != *dependency,
         decreases dependencies.len() - index,])
}

fn producer_source_pair_contains_v1(sources: &[ContextReadSourceV1], source: &ContextReadSourceV1)
    -> (found: bool)
    ensures found == (exists|j: int| 0 <= j < sources@.len()
        && sources@[j].region == source.region && sources@[j].record == source.record),
{
    producer_source_pair_contains_body!(verus_exec_expr, sources, source, index,
        [invariant
            index <= sources.len(),
            forall|j: int| 0 <= j < index
                ==> !(sources@[j].region == source.region && sources@[j].record == source.record),
         decreases sources.len() - index,])
}

spec fn header<R>(root: &Root<R>, index: usize, active: usize, queued: usize,
    consumer: ContextWriterKeyV1, returns: Returns) -> Header
    recommends index < root.inputs@.len(),
{
    let invalid = ContextVersionJournalErrorV1::InvalidReference;
    match root.inputs@[index as int].request {
        ProducerReadRequestV1::Active(request) => {
            if active >= root.references@.len() { Header { result: Err(invalid), active, queued, calls: seq![] } }
            else {
                let reference = root.references@[active as int];
                if active >= root.requests@.len() || root.requests@[active as int] != request
                    || reference.consumer != consumer
                    || root.references@[0].incarnation as int + (active as u64) as int != reference.incarnation as int
                { Header { result: Err(invalid), active, queued, calls: seq![] } }
                else {
                    let calls = seq![Call::ActiveLookup(reference)];
                    match returns.active_lookup {
                        Err(error) => Header { result: Err(error), active, queued, calls },
                        Ok(found) => if found != request { Header { result: Err(invalid), active, queued, calls } }
                        else {
                            let calls = calls.push(Call::ActiveStatus(reference));
                            let result = match returns.active_status {
                                Err(error) => Err(error),
                                Ok(status) => Ok((ContextAllocationWriteV1 { allocation: request.read.allocation,
                                    device: request.read.device, byte_extent: request.read.byte_extent },
                                    request.read.byte_offset, request.read.byte_len, request.producer, status)),
                            };
                            Header { result, active: (active + 1) as usize, queued, calls }
                        },
                    }
                }
            }
        },
        ProducerReadRequestV1::Queued(request) => {
            if queued >= root.queued_references@.len() { Header { result: Err(invalid), active, queued, calls: seq![] } }
            else {
                let reference = root.queued_references@[queued as int];
                if queued >= root.queued_requests@.len() || root.queued_requests@[queued as int] != request
                    || reference.consumer != consumer
                    || root.queued_references@[0].incarnation as int + (queued as u64) as int != reference.incarnation as int
                { Header { result: Err(invalid), active, queued, calls: seq![] } }
                else {
                    let calls = seq![Call::QueuedLookup(reference)];
                    match returns.queued_lookup {
                        Err(error) => Header { result: Err(error), active, queued, calls },
                        Ok(found) => if found != request { Header { result: Err(invalid), active, queued, calls } }
                        else {
                            let calls = calls.push(Call::QueuedStatus(reference));
                            let result = match returns.queued_status {
                                Err(error) => Err(error),
                                Ok(status) => Ok((request.allocation, request.byte_offset, request.byte_len, request.producer, status)),
                            };
                            Header { result, active, queued: (queued + 1) as usize, calls }
                        },
                    }
                }
            }
        },
    }
}

spec fn locally_bound<C, L, P, D>(context: &Context<C, L, P, D>, id: RuntimeSubmissionIdV1,
    input: ProducerInputV1, launch: bool) -> bool
{
    if launch {
        context.producer_launches@.contains_key(id)
        && context.producer_launches@[id].dependencies_held
        && context.producer_launches@[id].dependencies@.contains(input.dependency)
        && exists|i: int| 0 <= i < context.producer_launches@[id].sources@.len()
            && context.producer_launches@[id].sources@[i].region == input.source.region
            && context.producer_launches@[id].sources@[i].record == input.source.record
    } else {
        context.scalar_peer_copies@.contains_key(id)
        && context.scalar_peer_copies@[id].directed.is_some()
        && context.scalar_peer_copies@[id].dependencies_held
        && context.scalar_peer_copies@[id].dependencies@.contains(input.dependency)
        && context.scalar_peer_copies@[id].source.region == input.source.region
        && context.scalar_peer_copies@[id].source.record == input.source.record
    }
}

spec fn outcome<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, index: usize, active: usize, queued: usize,
    returns: Returns) -> Outcome
    recommends index < owner.root.inputs@.len(),
{
    let h = header(&owner.root, index, active, queued, consumer, returns);
    let input = owner.root.inputs@[index as int];
    let source = input.source;
    let invalid = ContextVersionJournalErrorV1::InvalidReference;
    match h.result {
        Err(error) => Outcome { result: Err(error), active: h.active, queued: h.queued, calls: h.calls },
        Ok((allocation, offset, bytes, producer, status)) => {
            let local = locally_bound(&owner.context, id, input, launch)
                && producer.key == (ContextWriterKeyV1 { context_generation: input.dependency.submission.context_generation,
                    local: input.dependency.submission.local, kind: ContextWriterKindV1::Submission })
                && input.dependency.submission.local < id.local
                && (index == 0 || allocation_less(owner.root.inputs@[index as int - 1].source.region.allocation, source.region.allocation))
                && owner.context.allocations@.contains_key(source.region.allocation)
                && owner.context.allocations@[source.region.allocation] == source.record
                && owner.context.backend_allocations@.contains(source.record.backend_allocation);
            if !local { Outcome { result: Err(invalid), active: h.active, queued: h.queued, calls: h.calls } }
            else {
                let calls = h.calls.push(Call::Credit(source.region.allocation, source.record.device, source.record.byte_len));
                if !returns.credit { Outcome { result: Err(invalid), active: h.active, queued: h.queued, calls } }
                else {
                    let calls = calls.push(Call::Live(source.region.allocation, source.record));
                    let result = match returns.live {
                        Err(error) => Err(error),
                        Ok(live) => if live == allocation.allocation
                            && allocation.device == (ContextJournalDeviceKeyV1 { context_generation: source.record.device.context_generation,
                                local: source.record.device.local })
                            && allocation.byte_extent == source.record.byte_len
                            && offset == source.region.byte_offset && bytes == source.region.byte_len
                        { Ok(status) } else { Err(invalid) },
                    };
                    Outcome { result, active: h.active, queued: h.queued, calls }
                }
            }
        },
    }
}

proof fn allocation_order_correspondence(a: RuntimeAllocationIdV1, b: RuntimeAllocationIdV1)
    ensures
        (vstd::std_specs::cmp::PartialOrdSpec::partial_cmp_spec(&a, &b)
            matches Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal))
            <==> !allocation_less(a, b),
{
}

broadcast proof fn trace_push_after_prefix(prefix: Seq<Call>, suffix: Seq<Call>, call: Call)
    ensures #[trigger] (prefix + suffix.push(call)) == (prefix + suffix).push(call),
{
    Seq::push_distributes_over_add(prefix, suffix, call);
}

}
