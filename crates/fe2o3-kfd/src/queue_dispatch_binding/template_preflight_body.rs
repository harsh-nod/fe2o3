// Read-only host metadata checks. Retained facts are inspected, not authenticated
// against a live mapping, and no resource authority or epoch is consumed.
macro_rules! dispatch_template_packet_count_body {
    ($syntax:ident, $n:ident) => {
        $syntax!({
            if $n == 0 {
                return Err(Gfx942DispatchBindingErrorV1::ZeroPacketCount);
            }
            if $n > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
                return Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
                    requested: $n,
                    maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize,
                });
            }
            Ok(())
        })
    };
}

macro_rules! dispatch_template_data_vm_body {
    ($syntax:ident, $owner:ident) => {
        $syntax!({
            match $owner {
                Self::Device(authority) => authority.facts().vm(),
                Self::HostVisible(authority) => authority.facts().mapping().allocation.vm,
            }
        })
    };
}

macro_rules! dispatch_template_preflight_body {
    ($syntax:ident, $owner:ident, $n:ident, $queue:ident) => {
        dispatch_template_preflight_body!(@annotated $syntax, $owner, $n, $queue,
            index, [], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $n:ident, $queue:ident, $index:ident,
     [$($code_invariants:tt)*], [$($code_step:tt)*],
     [$($data_invariants:tt)*], [$($data_step:tt)*],
     [$($order_initial:tt)*], [$($order_invariants:tt)*],
     [$($order_before:tt)*], [$($order_step:tt)*]) => {
        $syntax!({
            $owner.generation.ensure_not_poisoned()?;
            validate_packet_count::<$n>()?;
            if $owner.packets.len() != $n
                || $owner.code_identity.len() != $owner.code.len()
                || $owner.data.len() != $owner.data_premises.len()
            {
                return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
            }
            let mut $index = 0;
            while $index < $owner.code_identity.len() $($code_invariants)* {
                if $owner.code_identity[$index].mapping.allocation.vm != $queue.vm {
                    return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
                }
                $($code_step)*
                $index += 1;
            }
            if $owner.kernarg.facts().mapping().allocation.vm != $queue.vm {
                return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
            }
            let mut $index = 0;
            while $index < $owner.data.len() $($data_invariants)* {
                if $owner.data[$index].vm() != $queue.vm {
                    return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
                }
                $($data_step)*
                $index += 1;
            }
            let mut $index = 0;
            $($order_initial)*
            while $index < $owner.packets.len() $($order_invariants)* {
                $($order_before)*
                if $owner.packets[$index].ordering != AqlDispatchOrderingV1::WaitForPrior {
                    return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                        packet: $index,
                        detail: "multi-inflight recipe requires wait-for-prior ordering",
                    });
                }
                $($order_step)*
                $index += 1;
            }
            let (_, generation, _) = $owner.generation.preflight_reservation($queue)?;
            Ok(generation)
        })
    };
}
