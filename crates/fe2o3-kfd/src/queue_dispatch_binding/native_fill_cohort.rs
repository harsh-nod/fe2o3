//! Original-owner closed-fill cohorts, separate from singleton admission.
//!
//! This is checked composition, not a refinement theorem of batch preparation
//! or publication. Existing singleton proofs do not establish this composition.

use super::*;
use fe2o3_kernel_analysis::Gfx942FillKernelV1;

#[path = "native_fill_cohort/premises.rs"]
mod premises;
pub(super) use premises::{CohortStorageV1, NativeMemberCustodyV1};

/// One original checked executable, conditional packet and coherent output.
/// Construction is inert and does not supply compiler or launch authority.
pub struct Gfx942NativeFillCohortMemberV1<'a> {
    program: ValidatedKernelEnvelope<'a>,
    packet: Gfx942FixedDispatchPacketV1,
    output: Gfx942FixedDispatchDataV1,
}

impl<'a> Gfx942NativeFillCohortMemberV1<'a> {
    pub fn new(
        program: ValidatedKernelEnvelope<'a>,
        packet: Gfx942FixedDispatchPacketV1,
        output: Gfx942FixedDispatchDataV1,
    ) -> Self {
        Self {
            program,
            packet,
            output,
        }
    }

    /// Returns the same original owners, not reconstructed resource tokens.
    pub fn into_parts(
        self,
    ) -> (
        ValidatedKernelEnvelope<'a>,
        Gfx942FixedDispatchPacketV1,
        Gfx942FixedDispatchDataV1,
    ) {
        (self.program, self.packet, self.output)
    }
}

/// Rejected admission retains every original input before any native operation.
pub struct Gfx942NativeFillCohortFailureV1<'a, const N: usize> {
    members: [Gfx942NativeFillCohortMemberV1<'a>; N],
    error: Gfx942DispatchBindingErrorV1,
}

impl<const N: usize> core::fmt::Debug for Gfx942NativeFillCohortFailureV1<'_, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gfx942NativeFillCohortFailureV1")
            .field("original_member_count", &N)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<'a, const N: usize> Gfx942NativeFillCohortFailureV1<'a, N> {
    pub fn error(&self) -> &Gfx942DispatchBindingErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        [Gfx942NativeFillCohortMemberV1<'a>; N],
        Gfx942DispatchBindingErrorV1,
    ) {
        (self.members, self.error)
    }
}

/// A move-only cohort of 2..=16 independently owned closed-full64 fills.
///
/// Every member uses local program/output index zero before admission. Outputs
/// must be distinct whole coherent allocations; aggregate preparation preserves
/// one program, packet, patched kernarg subrange and DATA owner per member.
/// Only `WaitForPrior` is supported. Results require whole-cohort settlement;
/// this is not out-of-order completion, rolling admission or physical overlap.
/// It supplies no compiler/proof authority and cannot replace a singleton packet.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942FixedDispatchPacketV1, Gfx942NativeFillCohortV1};
/// fn singleton(_: Gfx942FixedDispatchPacketV1) {}
/// fn reject(cohort: Gfx942NativeFillCohortV1<'_, 3>) { singleton(cohort); }
/// ```
pub struct Gfx942NativeFillCohortV1<'a, const N: usize> {
    pub(in crate::queue) programs: Vec<ValidatedKernelEnvelope<'a>>,
    pub(in crate::queue) packets: [Gfx942FixedDispatchPacketV1; N],
    pub(in crate::queue) data: Vec<Gfx942FixedDispatchDataV1>,
}

impl<'a, const N: usize> Gfx942NativeFillCohortV1<'a, N> {
    pub fn admit(
        members: [Gfx942NativeFillCohortMemberV1<'a>; N],
    ) -> Result<Self, Gfx942NativeFillCohortFailureV1<'a, N>> {
        let mut programs = Vec::new();
        let mut data = Vec::new();
        let result = (|| {
            validate_count(N)?;
            for (index, member) in members.iter().enumerate() {
                if !member.packet.conditional_fill
                    || member.packet.ordering != AqlDispatchOrderingV1::WaitForPrior
                {
                    return Err(rejected(
                        index,
                        "cohort requires explicit conditional WaitForPrior members",
                    ));
                }
                let plan = plan_public_fixed_dispatch_resources(
                    core::slice::from_ref(&member.program),
                    core::array::from_ref(&member.packet),
                    &[member.output.layout()],
                    &[member.output.is_fully_initialized()],
                )?;
                conditional_fill::check_plan(
                    core::slice::from_ref(&member.program),
                    core::array::from_ref(&member.packet),
                    &plan,
                    PersistentFixedDispatchControlStateV1::Ordinary,
                )?
                .ok_or_else(|| rejected(index, "missing conditional member"))?;
                if members[..index].iter().any(|previous| {
                    previous.output.sdma_storage_identity() == member.output.sdma_storage_identity()
                }) {
                    return Err(rejected(index, "cohort output owner alias"));
                }
            }
            programs
                .try_reserve_exact(N)
                .map_err(|_| rejected(0, "cohort program backing"))?;
            data.try_reserve_exact(N)
                .map_err(|_| rejected(0, "cohort DATA backing"))?;
            Ok(())
        })();
        if let Err(error) = result {
            return Err(Gfx942NativeFillCohortFailureV1 { members, error });
        }
        let mut index = 0;
        // All fallible checks/backing precede these original-owner moves.
        let packets = members.map(|member| {
            let mut packet = member.packet;
            packet.program_index = index;
            packet.buffers[0].data_index = index;
            programs.push(member.program);
            data.push(member.output);
            index += 1;
            packet
        });
        Ok(Self {
            programs,
            packets,
            data,
        })
    }

    pub const fn member_count(&self) -> usize {
        N
    }
}

pub(super) fn rejected(packet: usize, detail: &'static str) -> Gfx942DispatchBindingErrorV1 {
    Gfx942DispatchBindingErrorV1::InvalidKernarg { packet, detail }
}

pub(super) fn validate_count(count: usize) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if !(2..=GFX942_MAX_FIXED_DISPATCH_DATA_V1).contains(&count) {
        return Err(rejected(
            0,
            "native fill cohort requires 2..=16 original members",
        ));
    }
    Ok(())
}

pub(super) fn check_plan<'a, const N: usize>(
    programs: &[ValidatedKernelEnvelope<'a>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    plan: &FixedDispatchPreparationPlanV1,
    control: PersistentFixedDispatchControlStateV1,
) -> Result<Vec<Gfx942FillKernelV1<'a>>, Gfx942DispatchBindingErrorV1> {
    validate_count(N)?;
    if programs.len() != N
        || plan.data.len() != N
        || plan.packets.len() != N
        || !matches!(control, PersistentFixedDispatchControlStateV1::Ordinary)
        || N.checked_mul(conditional_fill::KERNARG_BYTES) != Some(plan.kernarg_arena_bytes)
    {
        return Err(rejected(0, "native cohort exact owner and kernarg roster"));
    }
    let mut models = Vec::new();
    models
        .try_reserve_exact(N)
        .map_err(|_| rejected(0, "cohort model backing"))?;
    for (index, (input, program)) in packets.iter().zip(programs).enumerate() {
        let [binding] = input.buffers.as_ref() else {
            return Err(rejected(index, "cohort one whole output per member"));
        };
        let data = &plan.data[index];
        if !input.conditional_fill
            || input.ordering != AqlDispatchOrderingV1::WaitForPrior
            || input.program_index != index
            || input.kernarg_bytes.len() != conditional_fill::KERNARG_BYTES
            || input.dynamic_group_segment_bytes != 0
            || binding.explicit_argument_index != 0
            || binding.data_index != index
            || binding.data_byte_offset != 0
            || binding.byte_len == 0
            || binding.byte_len != data.layout.requested_bytes()
            || data.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
            || binding.completed_snapshot.is_some()
            || data.effect != Some(DeviceDataEffectV1::WriteOnly)
            || plan.packets[index].kernarg_offset != index * conditional_fill::KERNARG_BYTES
        {
            return Err(rejected(index, "cohort exact per-member resource shape"));
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
            return Err(rejected(index, "cohort full64 member coverage"));
        }
        validate_gfx942_kernel_profile(program)?;
        let model = Gfx942FillKernelV1::inspect(
            program.envelope().bytes(),
            program.selected_kernel_index(),
        )
        .map_err(|_| rejected(index, "cohort exact member machine profile"))?;
        if model.binding() != program.selected_binding()
            || model.kernarg_storage_bytes() != conditional_fill::KERNARG_BYTES as u64
        {
            return Err(rejected(index, "cohort selected member descriptor"));
        }
        models.push(model);
    }
    Ok(models)
}
