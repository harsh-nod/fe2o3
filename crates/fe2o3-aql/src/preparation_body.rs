// Inert preparation bodies shared with host-side execution proofs.
macro_rules! aql_field_body {
    ($syntax:ident, $value:ident, $field:tt) => {
        $syntax!({ $value.$field })
    };
}

macro_rules! aql_address_body {
    ($syntax:ident, $raw:ident) => {
        $syntax!({
            if $raw == 0 {
                return Err(AqlAddressObservationError::Zero);
            }
            Ok(Self($raw))
        })
    };
}

macro_rules! aql_alignment_body {
    ($syntax:ident, $value:ident, $alignment:ident) => {
        $syntax!({
            if $alignment == 0 || $alignment > 4096 || $alignment & ($alignment - 1) != 0 {
                return Err(AqlAddressObservationError::InvalidRequiredAlignment);
            }
            if $value.0 & ($alignment - 1) != 0 {
                return Err(AqlAddressObservationError::Misaligned);
            }
            Ok($value)
        })
    };
}

macro_rules! aql_dispatch_preparation_body {
    ($syntax:ident, $geometry:ident, $private:ident, $group:ident, $kernel:ident,
     $kernarg:ident, $alignment:ident, $signal:ident, $ordering:ident) => {
        $syntax!({
            let $kernel = match $kernel.require_alignment(64) {
                Ok(value) => value,
                Err(error) => return Err(AqlDispatchPacketError::KernelObject(error)),
            };
            let $kernarg = match $kernarg.require_alignment($alignment) {
                Ok(value) => value,
                Err(error) => return Err(AqlDispatchPacketError::Kernarg(error)),
            };
            let $signal = match $signal.require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64) {
                Ok(value) => value,
                Err(error) => return Err(AqlDispatchPacketError::CompletionSignal(error)),
            };
            let workgroup = $geometry.workgroup();
            let grid = $geometry.grid();
            let packet = Self {
                full_header: (u32::from($geometry.dimensions()) << 16)
                    | u32::from(AQL_INVALID_PACKET_HEADER_V1),
                workgroup_size_x: workgroup[0],
                workgroup_size_y: workgroup[1],
                workgroup_size_z: workgroup[2],
                reserved0: 0,
                grid_size_x: grid[0],
                grid_size_y: grid[1],
                grid_size_z: grid[2],
                private_segment_size: $private,
                group_segment_size: $group,
                kernel_object: $kernel.raw(),
                kernarg_address: $kernarg.raw(),
                reserved2: 0,
                completion_signal: $signal.raw(),
            };
            Ok(AqlPreparedKernelDispatchV1 {
                packet,
                ordering: $ordering,
            })
        })
    };
}

macro_rules! aql_boxed_batch_body {
    ($syntax:ident, $packets:ident, $n:ident) => {
        $syntax!({
            if $n == 0 {
                return Err(AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount);
            }
            if $n > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
                return Err(
                    AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                        requested: $n,
                        maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2,
                    },
                );
            }
            Ok(Self { packets: $packets })
        })
    };
}
