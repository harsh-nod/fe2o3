//! Closed subrange composition over one original coherent DATA owner.
//!
//! The existing machine checker validates each actual output range. Disjoint
//! range composition and the private queue owner are checked Rust boundaries,
//! not a new executable-refinement or native-concurrency theorem.

use super::*;
use fe2o3_kernel_analysis::Gfx942FillKernelV1;

#[path = "native_fill_arena/premises.rs"]
mod premises;
pub(super) use premises::ArenaPremisesV1;
#[path = "native_fill_arena/packets.rs"]
mod packets;
pub use packets::Gfx942NativeFillArenaPacketsV1;
#[path = "native_fill_arena/storage.rs"]
mod storage;
pub(in crate::queue) use storage::ArenaRecipeV1;

#[cfg(test)]
#[path = "native_fill_arena/tests.rs"]
mod tests;
pub use storage::Gfx942NativeFillArenaStorageV1;

/// Exact single-use slot count shared by the two distinct closed arena profiles.
pub const GFX942_NATIVE_FILL_ARENA_SLOTS_V1: usize = 1024;
pub(super) const SLOTS: usize = GFX942_NATIVE_FILL_ARENA_SLOTS_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::queue) enum ArenaOrderV1 {
    Ordered,
    IndependentDisjointWriteOnly,
}

impl ArenaOrderV1 {
    pub(in crate::queue) const fn packet_order(self) -> AqlDispatchOrderingV1 {
        match self {
            Self::Ordered => AqlDispatchOrderingV1::WaitForPrior,
            Self::IndependentDisjointWriteOnly => AqlDispatchOrderingV1::Independent,
        }
    }
}

/// Original executable, packets and one common coherent DATA allocation.
/// Admission is inert; no compiler, currentness or execution authority is added.
/// Each packet must select its exact dense subrange using local program/DATA
/// index zero. No suballocation token can be extracted from this owner.
pub struct Gfx942NativeFillArenaInputsV1<'a> {
    pub(in crate::queue) programs: Vec<ValidatedKernelEnvelope<'a>>,
    pub(in crate::queue) packets: Gfx942NativeFillArenaPacketsV1,
    pub(in crate::queue) data: Vec<Gfx942FixedDispatchDataV1>,
    pub(in crate::queue) order: ArenaOrderV1,
}

/// Distinct inert admission of 1024 independent fill packets over disjoint
/// write-only subranges. It retains the exact original executable and DATA;
/// no native authority or measured out-of-order completion follows.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942IndependentFillArenaInputsV1 as Independent,
///                 Gfx942NativeFillArenaInputsV1 as Ordered};
/// fn refuse<'a>(inputs: Independent<'a>) -> Ordered<'a> { inputs }
/// ```
pub struct Gfx942IndependentFillArenaInputsV1<'a>(
    pub(in crate::queue) Gfx942NativeFillArenaInputsV1<'a>,
);

impl<'a> Gfx942IndependentFillArenaInputsV1<'a> {
    /// Accepts only explicitly independent original packets, then checks the
    /// complete dense WO partition and exact closed fill machine profile.
    // Refusal returns every original owner without allocating an error wrapper.
    #[allow(clippy::result_large_err)]
    pub fn admit_local_outputs(
        program: ValidatedKernelEnvelope<'a>,
        packets: Gfx942NativeFillArenaPacketsV1,
        output: Gfx942FixedDispatchDataV1,
    ) -> Result<Self, Gfx942NativeFillArenaFailureV1<'a>> {
        Gfx942NativeFillArenaInputsV1::admit_local_outputs_with_order(
            program,
            packets,
            output,
            ArenaOrderV1::IndependentDisjointWriteOnly,
        )
        .map(Self)
    }
}

/// Refusal returns all original owners before native preparation.
pub struct Gfx942NativeFillArenaFailureV1<'a> {
    program: ValidatedKernelEnvelope<'a>,
    packets: Gfx942NativeFillArenaPacketsV1,
    output: Gfx942FixedDispatchDataV1,
    error: Gfx942DispatchBindingErrorV1,
}

impl core::fmt::Debug for Gfx942NativeFillArenaFailureV1<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gfx942NativeFillArenaFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<'a> Gfx942NativeFillArenaFailureV1<'a> {
    pub fn error(&self) -> &Gfx942DispatchBindingErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedKernelEnvelope<'a>,
        Gfx942NativeFillArenaPacketsV1,
        Gfx942FixedDispatchDataV1,
        Gfx942DispatchBindingErrorV1,
    ) {
        (self.program, self.packets, self.output, self.error)
    }
}

impl<'a> Gfx942NativeFillArenaInputsV1<'a> {
    /// Places exact original local-offset-zero output descriptors into one dense
    /// arena, then performs the same closed machine/shape admission as `admit`.
    /// This inert placement adds no execution authority. Refusal restores every
    /// original local offset and returns the original packet table and DATA.
    #[allow(clippy::result_large_err)]
    pub fn admit_local_outputs(
        program: ValidatedKernelEnvelope<'a>,
        packets: Gfx942NativeFillArenaPacketsV1,
        output: Gfx942FixedDispatchDataV1,
    ) -> Result<Self, Gfx942NativeFillArenaFailureV1<'a>> {
        Self::admit_local_outputs_with_order(program, packets, output, ArenaOrderV1::Ordered)
    }

    #[allow(clippy::result_large_err)]
    fn admit_local_outputs_with_order(
        program: ValidatedKernelEnvelope<'a>,
        mut packets: Gfx942NativeFillArenaPacketsV1,
        output: Gfx942FixedDispatchDataV1,
        order: ArenaOrderV1,
    ) -> Result<Self, Gfx942NativeFillArenaFailureV1<'a>> {
        if let Err(error) = packets.place_local_outputs(output.layout().requested_bytes()) {
            return Err(Gfx942NativeFillArenaFailureV1 {
                program,
                packets,
                output,
                error,
            });
        }
        Self::admit_with_order(program, packets, output, order).map_err(|mut failure| {
            failure.packets.restore_local_outputs();
            failure
        })
    }

    // Returning original packets/DATA avoids a second fallible owner allocation.
    #[allow(clippy::result_large_err)]
    pub fn admit(
        program: ValidatedKernelEnvelope<'a>,
        packets: Gfx942NativeFillArenaPacketsV1,
        output: Gfx942FixedDispatchDataV1,
    ) -> Result<Self, Gfx942NativeFillArenaFailureV1<'a>> {
        Self::admit_with_order(program, packets, output, ArenaOrderV1::Ordered)
    }

    #[allow(clippy::result_large_err)]
    fn admit_with_order(
        program: ValidatedKernelEnvelope<'a>,
        packets: Gfx942NativeFillArenaPacketsV1,
        output: Gfx942FixedDispatchDataV1,
        order: ArenaOrderV1,
    ) -> Result<Self, Gfx942NativeFillArenaFailureV1<'a>> {
        let mut programs = Vec::new();
        let mut data = Vec::new();
        let result = (|| {
            let plan = plan_fixed_dispatch_resources_with_order(
                core::slice::from_ref(&program),
                packets.packets(),
                &[output.layout()],
                &[output.is_fully_initialized()],
                order.packet_order(),
            )?;
            check_plan(
                core::slice::from_ref(&program),
                packets.packets(),
                &plan,
                PersistentFixedDispatchControlStateV1::Ordinary,
                order,
            )?;
            programs
                .try_reserve_exact(1)
                .map_err(|_| rejected(0, "arena program capacity"))?;
            data.try_reserve_exact(1)
                .map_err(|_| rejected(0, "arena DATA capacity"))?;
            Ok(())
        })();
        if let Err(error) = result {
            return Err(Gfx942NativeFillArenaFailureV1 {
                program,
                packets,
                output,
                error,
            });
        }
        programs.push(program);
        data.push(output);
        Ok(Self {
            programs,
            packets,
            data,
            order,
        })
    }
}

pub(super) fn rejected(packet: usize, detail: &'static str) -> Gfx942DispatchBindingErrorV1 {
    Gfx942DispatchBindingErrorV1::InvalidKernarg { packet, detail }
}

pub(super) fn check_plan<'a, const N: usize>(
    programs: &[ValidatedKernelEnvelope<'a>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    plan: &FixedDispatchPreparationPlanV1,
    control: PersistentFixedDispatchControlStateV1,
    order: ArenaOrderV1,
) -> Result<Gfx942FillKernelV1<'a>, Gfx942DispatchBindingErrorV1> {
    if N != SLOTS
        || programs.len() != 1
        || plan.programs.len() != 1
        || plan.data.len() != 1
        || plan.packets.len() != SLOTS
        || plan.kernarg_arena_bytes != SLOTS * conditional_fill::KERNARG_BYTES
        || !matches!(control, PersistentFixedDispatchControlStateV1::Ordinary)
    {
        return Err(rejected(0, "arena exact source and common owner roster"));
    }
    let data = &plan.data[0];
    if data.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
        || data.effect != Some(DeviceDataEffectV1::WriteOnly)
        || data.writable_ranges.len() != SLOTS
    {
        return Err(rejected(0, "arena original coherent WO partition"));
    }
    let mut end = 0u64;
    for (index, input) in packets.iter().enumerate() {
        let [binding] = input.buffers.as_ref() else {
            return Err(rejected(index, "arena one output subrange per slot"));
        };
        if !input.conditional_fill
            || input.ordering != order.packet_order()
            || input.program_index != 0
            || input.kernarg_bytes.len() != conditional_fill::KERNARG_BYTES
            || input.dynamic_group_segment_bytes != 0
            || binding.explicit_argument_index != 0
            || binding.data_index != 0
            || binding.data_byte_offset != end
            || binding.byte_len == 0
            || binding.completed_snapshot.is_some()
            || plan.packets[index].kernarg_offset != index * conditional_fill::KERNARG_BYTES
        {
            return Err(rejected(index, "arena dense original subrange shape"));
        }
        let count = u64::from_le_bytes(input.kernarg_bytes[8..16].try_into().unwrap());
        let grid = input.geometry.grid();
        if count.checked_mul(4) != Some(binding.byte_len)
            || input.geometry.workgroup() != [64, 1, 1]
            || grid[0] == 0
            || !grid[0].is_multiple_of(64)
            || grid[1..] != [1, 1]
            || count > u64::from(grid[0])
        {
            return Err(rejected(index, "arena full64 slot coverage"));
        }
        end = end
            .checked_add(binding.byte_len)
            .ok_or_else(|| rejected(index, "arena range overflow"))?;
    }
    if end != data.layout.requested_bytes() || !data.completed_snapshots.is_empty() {
        return Err(rejected(0, "arena exact original logical DATA extent"));
    }
    let program = &programs[0];
    validate_gfx942_kernel_profile(program)?;
    let model =
        Gfx942FillKernelV1::inspect(program.envelope().bytes(), program.selected_kernel_index())
            .map_err(|_| rejected(0, "arena exact machine profile"))?;
    if model.binding() != program.selected_binding()
        || model.kernarg_storage_bytes() != conditional_fill::KERNARG_BYTES as u64
    {
        return Err(rejected(0, "arena exact selected descriptor"));
    }
    Ok(model)
}
