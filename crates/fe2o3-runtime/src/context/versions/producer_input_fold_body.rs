// The native adapter performs one ordered validation per reached input. Proof
// adapters must replay those individual observations, not a frozen credit ledger.
macro_rules! producer_input_fold_body {
    ($syntax:ident, $observations:ident, $invalid_reference:ident,
     ($index:ident, $aggregate:ident, $active_index:ident, $queued_index:ident, $input_count:ident),
     [$($invariants:tt)*], [$($before_step:tt)*], [$($after_step:tt)*]) => {
        $syntax!({
            let $input_count = $observations.input_count();
            let mut $aggregate = ContextProducerReadStatusV1::Success;
            let mut $index = 0usize;
            let mut $active_index = 0usize;
            let mut $queued_index = 0usize;
            while $index < $input_count
                $($invariants)*
            {
                $($before_step)*
                let status = $observations.validate($index, &mut $active_index, &mut $queued_index)?;
                // Unknown is an aggregate value, not permission to skip later validation.
                $aggregate = match ($aggregate, status) {
                    (ContextProducerReadStatusV1::Unknown, _)
                    | (_, ContextProducerReadStatusV1::Unknown) => ContextProducerReadStatusV1::Unknown,
                    (ContextProducerReadStatusV1::NoEffect, _)
                    | (_, ContextProducerReadStatusV1::NoEffect) => ContextProducerReadStatusV1::NoEffect,
                    (ContextProducerReadStatusV1::Pending, _)
                    | (_, ContextProducerReadStatusV1::Pending) => ContextProducerReadStatusV1::Pending,
                    _ => ContextProducerReadStatusV1::Success,
                };
                $index += 1;
                $($after_step)*
            }
            if $active_index != $observations.active_count()
                || $queued_index != $observations.queued_count()
            {
                return Err($invalid_reference);
            }
            Ok($aggregate)
        })
    };
}

macro_rules! producer_dependency_contains_body {
    ($syntax:ident, $dependencies:ident, $dependency:ident, $index:ident,
     [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $dependencies.len()
                $($invariants)*
            {
                if &$dependencies[$index] == $dependency {
                    return true;
                }
                $index += 1;
            }
            false
        })
    };
}

macro_rules! producer_source_pair_contains_body {
    ($syntax:ident, $sources:ident, $source:ident, $index:ident,
     [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $sources.len()
                $($invariants)*
            {
                let original = &$sources[$index];
                if original.region == $source.region && original.record == $source.record {
                    return true;
                }
                $index += 1;
            }
            false
        })
    };
}

macro_rules! producer_input_validate_body {
    ($syntax:ident, $context:ident, $root:ident, $id:ident,
     $consumer:ident, $launch:ident, $index:ident, $active_index:ident,
     $queued_index:ident, $observations:ident) => {
        $syntax!({
            use ContextVersionJournalErrorV1 as E;
            let input = &$root.inputs[$index];
            let source = input.source;
            let (allocation, byte_offset, byte_len, producer, status) = match input.request {
                ProducerReadRequestV1::Active(request) => {
                    let reference = *$root
                        .references
                        .get(*$active_index)
                        .ok_or(E::InvalidReference)?;
                    if $root.requests.get(*$active_index) != Some(&request)
                        || reference.consumer != $consumer
                        || $root.references[0]
                            .incarnation
                            .checked_add(*$active_index as u64)
                            != Some(reference.incarnation)
                        || $observations.observe_active_lookup(reference)? != request
                    {
                        return Err(E::InvalidReference);
                    }
                    *$active_index += 1;
                    (
                        fe2o3_runtime_model::ContextAllocationWriteV1 {
                            allocation: request.read.allocation,
                            device: request.read.device,
                            byte_extent: request.read.byte_extent,
                        },
                        request.read.byte_offset,
                        request.read.byte_len,
                        request.producer,
                        $observations.observe_active_status(reference)?,
                    )
                }
                ProducerReadRequestV1::Queued(request) => {
                    let reference = *$root
                        .queued_references
                        .get(*$queued_index)
                        .ok_or(E::InvalidReference)?;
                    if $root.queued_requests.get(*$queued_index) != Some(&request)
                        || reference.consumer != $consumer
                        || $root.queued_references[0]
                            .incarnation
                            .checked_add(*$queued_index as u64)
                            != Some(reference.incarnation)
                        || $observations.observe_queued_lookup(reference)? != request
                    {
                        return Err(E::InvalidReference);
                    }
                    *$queued_index += 1;
                    (
                        request.allocation,
                        request.byte_offset,
                        request.byte_len,
                        request.producer,
                        $observations.observe_queued_status(reference)?,
                    )
                }
            };
            let bound = if $launch {
                match $context.producer_launches.get(&$id) {
                    Some(launch) => {
                        launch.dependencies_held
                            && producer_dependency_contains_v1(
                                &launch.dependencies,
                                &input.dependency,
                            )
                            && producer_source_pair_contains_v1(&launch.sources, &source)
                    }
                    None => false,
                }
            } else {
                match $context.scalar_peer_copies.get(&$id) {
                    Some(peer) => {
                        peer.directed.is_some()
                            && peer.dependencies_held
                            && producer_dependency_contains_v1(
                                &peer.dependencies,
                                &input.dependency,
                            )
                            && peer.source.region == source.region
                            && peer.source.record == source.record
                    }
                    None => false,
                }
            };
            if !bound
                || producer.key
                    != (ContextWriterKeyV1 {
                        context_generation: input.dependency.submission.context_generation,
                        local: input.dependency.submission.local,
                        kind: ContextWriterKindV1::Submission,
                    })
                || input.dependency.submission.local >= $id.local
                || $index > 0
                    && $root.inputs[$index - 1].source.region.allocation >= source.region.allocation
                || $context.allocations.get(&source.region.allocation) != Some(&source.record)
                || !$context
                    .backend_allocations
                    .contains(&source.record.backend_allocation)
                || !$observations.observe_expected_credit(
                    source.region.allocation,
                    source.record.device,
                    source.record.byte_len,
                )
                || $observations.observe_live(source.region.allocation, &source.record)?
                    != allocation.allocation
                || allocation.device
                    != enrollment(
                        source.region.allocation,
                        source.record.device,
                        source.record.byte_len,
                    )
                    .device
                || allocation.byte_extent != source.record.byte_len
                || byte_offset != source.region.byte_offset
                || byte_len != source.region.byte_len
            {
                return Err(E::InvalidReference);
            }
            // Resolved reservations outlive their writer slot; never revalidate admission.
            Ok(status)
        })
    };
}
