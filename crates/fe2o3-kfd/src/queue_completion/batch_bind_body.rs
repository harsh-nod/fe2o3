// All fallible preparation precedes the phase-only commit. The annotation
// hooks are empty in production and expose the same loops to verification.
macro_rules! completion_select_slots_body {
    ($syntax:ident, $owner:ident, $n:ident) => {
        completion_select_slots_body!(@annotated $syntax, $owner, $n, slots, index,
            [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $n:ident, $slots:ident, $index:ident,
     [$($initial:tt)*], [$($invariants:tt)*], [$($before:tt)*], [$($step:tt)*]) => {
        $syntax!({
            let mut $slots = Vec::<CompletionSlotLeaseV1>::new();
            let mut $index = 0;
            $($initial)*
            while $index < COMPLETION_SIGNAL_CAPACITY_V1 && $slots.len() < $n
                $($invariants)*
            {
                $($before)*
                let record = &$owner.slots[$index];
                if record.phase == CompletionSlotPhaseV1::Available {
                    $slots.push(CompletionSlotLeaseV1 {
                        index: $index as u32,
                        generation: record.generation,
                    });
                }
                $($step)*
                $index += 1;
            }
            $slots
        })
    };
}

macro_rules! completion_dispatch_binding_body {
    ($syntax:ident, $owner:ident, $binding:ident) => {
        $syntax!({
            if $binding.queue != $owner.queue {
                return Err(Gfx942CompletionErrorV1::WrongQueueGeneration);
            }
            if $binding.dispatch_generation == 0
                || $binding.code.allocation.vm != $owner.queue.vm
                || $binding.kernarg.allocation.vm != $owner.queue.vm
            {
                return Err(Gfx942CompletionErrorV1::WrongVmGeneration);
            }
            Ok(())
        })
    };
}

macro_rules! completion_commit_batch_body {
    ($syntax:ident, $owner:ident, $bound:ident, $next:ident, $n:ident) => {
        completion_commit_batch_body!(@annotated $syntax, $owner, $bound, $next,
            $n, index, [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $bound:ident, $next:ident, $n:ident,
     $index:ident, [$($initial:tt)*], [$($invariants:tt)*], [$($step:tt)*]) => {
        $syntax!({
            $($initial)*
            let mut $index = 0;
            while $index < $n $($invariants)* {
                let slot = &$bound.retention.slots[$index];
                $owner.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Bound {
                    batch_id: $owner.next_batch_id,
                };
                $($step)*
                $index += 1;
            }
            $owner.next_batch_id = $next;
            $bound
        })
    };
}

macro_rules! completion_bind_batch_body {
    ($syntax:ident, $owner:ident, $templates:ident, $n:ident) => {
        completion_bind_batch_body!(@annotated $syntax, $owner, $templates, $n,
            next_batch_id, slots, prepared, index, dispatches, packets, bound,
            [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $templates:ident, $n:ident,
     $next:ident, $slots:ident, $prepared:ident, $index:ident, $dispatches:ident,
     $packets:ident, $bound:ident, [$($initial:tt)*], [$($packet_invariants:tt)*],
     [$($packet_before:tt)*], [$($packet_step:tt)*], [$($dispatch_invariants:tt)*],
     [$($dispatch_step:tt)*], [$($before_commit:tt)*]) => {
        $syntax!({
            $owner.require_ready()?;
            validate_packet_count::<$n>()?;
            let $next = $owner.next_batch_id.checked_add(1)
                .ok_or(Gfx942CompletionErrorV1::BatchIdentityExhausted)?;
            let $slots = $owner.select_available_slots($n);
            if $slots.len() != $n {
                return Err(Gfx942CompletionErrorV1::InsufficientSignals);
            }
            let mut $prepared = Vec::<AqlPreparedKernelDispatchV1>::with_capacity($n);
            let mut $index = 0;
            $($initial)*
            while $index < $n $($packet_invariants)* {
                $($packet_before)*
                let template = &$templates.values[$index];
                let slot = &$slots[$index];
                $owner.validate_dispatch_binding(template.generations)?;
                let offset = u64::from(slot.index)
                    .checked_mul(AMD_SIGNAL_BYTES_V1 as u64)
                    .ok_or(Gfx942CompletionErrorV1::InvalidArena("completion slot offset"))?;
                let raw = $owner.gpu_base.checked_add(offset)
                    .ok_or(Gfx942CompletionErrorV1::InvalidArena("completion slot address"))?;
                let signal = match ObservedGpuAddressV1::new(raw) {
                    Ok(signal) => signal,
                    Err(_error) => return Err(Gfx942CompletionErrorV1::InvalidArena("completion address")),
                };
                let packet = match AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
                    template.geometry, template.private_segment_size, template.group_segment_size,
                    template.kernel_object, template.kernarg_address, template.kernarg_alignment,
                    signal, template.ordering,
                ) {
                    Ok(packet) => packet,
                    Err(error) => return Err(Gfx942CompletionErrorV1::PacketBinding(error)),
                };
                $prepared.push(packet);
                $($packet_step)*
                $index += 1;
            }
            let $packets: Box<[AqlPreparedKernelDispatchV1; $n]> = $prepared
                .into_boxed_slice().try_into()
                .map_err(|_error| Gfx942CompletionErrorV1::InvalidArena("packet array conversion"))?;
            let $packets = match AqlPreparedKernelDispatchBatchV2::try_from_boxed_packets($packets) {
                Ok(packets) => packets,
                Err(error) => return Err(Gfx942CompletionErrorV1::BatchConstruction(error)),
            };
            let $slots: Box<[CompletionSlotLeaseV1; $n]> = $slots
                .into_boxed_slice().try_into()
                .map_err(|_error| Gfx942CompletionErrorV1::InvalidArena("slot array conversion"))?;
            let mut $dispatches = Vec::<CompletionDispatchGenerationBindingV1>::with_capacity($n);
            let mut $index = 0;
            while $index < $n $($dispatch_invariants)* {
                $dispatches.push($templates.values[$index].generations);
                $($dispatch_step)*
                $index += 1;
            }
            let $dispatches: Box<[CompletionDispatchGenerationBindingV1; $n]> = $dispatches
                .into_boxed_slice().try_into()
                .map_err(|_error| Gfx942CompletionErrorV1::InvalidArena("dispatch array conversion"))?;
            let retention = CompletionBatchRetentionV1 {
                batch_id: $owner.next_batch_id, queue: $owner.queue,
                signal_mapping: $owner.signal_mapping, slots: $slots,
                dispatches: $dispatches, last_packet_id: None,
            };
            let $bound = BoundCompletionBatchV1 { packets: $packets, retention };
            $($before_commit)*
            Ok($owner.commit_bound_batch($bound, $next))
        })
    };
}
