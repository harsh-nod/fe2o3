//! Explicit independent2048 wrappers; no public conversion to the 1024 family.

use super::*;

/// Exact prepaid 2048-packet descriptor roster. The original account is charged
/// before any initializer runs. This does not admit a native queue or machine.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942IndependentFillArena2048PacketsV1 as Large,
///                 Gfx942NativeFillArenaPacketsV1 as Original};
/// fn refuse(packets: Large) -> Original { packets }
/// ```
pub struct Gfx942IndependentFillArena2048PacketsV1(Gfx942NativeFillArenaPacketsV1);

impl Gfx942IndependentFillArena2048PacketsV1 {
    pub fn try_new(
        account: &ResourceCreditAccountV1,
        initialize: impl FnMut(usize) -> Gfx942FixedDispatchPacketV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Gfx942NativeFillArenaPacketsV1::try_new_with_capacity(
            account,
            initialize,
            ArenaCapacityV1::Independent2048,
        )
        .map(Self)
    }

    pub fn packets(
        &self,
    ) -> &[Gfx942FixedDispatchPacketV1; GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1] {
        self.0.as_array()
    }
}

/// Prepaid original slot generations and retained native premise storage for
/// the independent2048 family only. Native allocations remain separately funded.
pub struct Gfx942IndependentFillArena2048StorageV1(
    pub(in crate::queue) Gfx942NativeFillArenaStorageV1,
);

impl Gfx942IndependentFillArena2048StorageV1 {
    pub fn preallocate(
        account: ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Gfx942NativeFillArenaStorageV1::preallocate_with_capacity(
            account,
            ArenaCapacityV1::Independent2048,
        )
        .map(Self)
    }
}

/// Original executable, common DATA and exactly 2048 independent disjoint-WO
/// packets. Only the existing closed fill machine/argument profile is accepted.
/// This inert admission is not native execution or a measured depth guarantee.
pub struct Gfx942IndependentFillArena2048InputsV1<'a>(
    pub(in crate::queue) Gfx942NativeFillArenaInputsV1<'a>,
);

impl<'a> Gfx942IndependentFillArena2048InputsV1<'a> {
    // Every original owner is returned inline on refusal, without another allocation.
    #[allow(clippy::result_large_err)]
    pub fn admit_local_outputs(
        program: ValidatedKernelEnvelope<'a>,
        packets: Gfx942IndependentFillArena2048PacketsV1,
        output: Gfx942FixedDispatchDataV1,
    ) -> Result<Self, Gfx942IndependentFillArena2048FailureV1<'a>> {
        Gfx942NativeFillArenaInputsV1::admit_local_outputs_with_order(
            program,
            packets.0,
            output,
            ArenaOrderV1::IndependentDisjointWriteOnly,
        )
        .map(Self)
        .map_err(Gfx942IndependentFillArena2048FailureV1)
    }
}

/// Effect-free admission refusal carrying all original 2048 input owners.
pub struct Gfx942IndependentFillArena2048FailureV1<'a>(Gfx942NativeFillArenaFailureV1<'a>);

impl core::fmt::Debug for Gfx942IndependentFillArena2048FailureV1<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gfx942IndependentFillArena2048FailureV1")
            .field("error", self.error())
            .finish_non_exhaustive()
    }
}

impl<'a> Gfx942IndependentFillArena2048FailureV1<'a> {
    pub fn error(&self) -> &Gfx942DispatchBindingErrorV1 {
        self.0.error()
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedKernelEnvelope<'a>,
        Gfx942IndependentFillArena2048PacketsV1,
        Gfx942FixedDispatchDataV1,
        Gfx942DispatchBindingErrorV1,
    ) {
        let (program, packets, output, error) = self.0.into_parts();
        (
            program,
            Gfx942IndependentFillArena2048PacketsV1(packets),
            output,
            error,
        )
    }
}

#[cfg(test)]
#[path = "independent2048_tests.rs"]
mod tests;
