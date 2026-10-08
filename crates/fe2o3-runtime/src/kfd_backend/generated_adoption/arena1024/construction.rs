use super::*;
use crate::generated_source::arena1024::ArenaPacketsV1;
use fe2o3_kfd::{Gfx942IndependentFillArenaInputsV1, Gfx942NativeFillArenaInputsV1};

enum InputsV1<'a> {
    Ordered(Gfx942NativeFillArenaInputsV1<'a>),
    Independent(Gfx942IndependentFillArenaInputsV1<'a>),
    Independent2048(fe2o3_kfd::Gfx942IndependentFillArena2048InputsV1<'a>),
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn adopt_generated_arena_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        program: ValidatedKernelEnvelope<'_>,
        bytes: u64,
        storage: ArenaPreallocationV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.preflight_generated_cohort3_lane_v1()?;
        if self.requires_request_witness_v1()
            && self
                .composed_request_binding
                .as_ref()
                .is_none_or(|binding| !binding.is_live())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena original request unavailable",
            ));
        }
        if plan.profile.arena_slots().is_none()
            || plan.count != 1
            || roster.count != 1
            || bytes != roster.readback_bytes
            || storage
                .lower
                .as_ref()
                .is_none_or(|lower| !lower.matches(plan.profile))
            || storage
                .initialization
                .as_ref()
                .is_none_or(|source| source.len() as u64 != bytes)
            || Some(storage.cells.len()) != plan.profile.arena_slots()
            || storage
                .cells
                .iter()
                .any(|cell| cell.copied || !matches!(cell.receipt, ReceiptV1::Ready))
            || !self.validate_generated_shell_records_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.registry.is_none()
                    && record.arena.is_none()
                    && record.control.is_some()
                    && record.source_identity.matches(&roster.source_identity)
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena original preparation mismatch",
            ));
        }
        self.check_arena_device_v1(plan)?;
        self.generated_shells
            .get_mut(&plan.key)
            .unwrap_or_else(|| std::process::abort())
            .arena = Some(ArenaV1 {
            profile: plan.profile,
            phase: PhaseV1::Entering,
            data: None,
            storage,
            session: None,
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.acquire_generated_vm_v1()?;
            let memory = self
                .terminal_memory
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            let initialized = memory.initialize_host_visible_coherent_from_slice_v1(
                arena
                    .storage
                    .initialization
                    .as_ref()
                    .unwrap_or_else(|| std::process::abort()),
            );
            let initialized = initialized.map_err(|error| {
                self.generated_native_error_v1("arena common DATA initialization", error)
            })?;
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            arena.data = Some(Gfx942FixedDispatchDataV1::host_visible_initialized(
                initialized,
            ));
            // Original native DATA is rooted before this effect-free scratch refund.
            drop(arena.storage.initialization.take());
            self.bind_generated_arena_v1(plan.key, program)?;
            self.check_arena_device_v1(plan)?;
            self.generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Adopted;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    fn bind_generated_arena_v1(
        &mut self,
        key: u64,
        program: ValidatedKernelEnvelope<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let record = self
            .generated_shells
            .get_mut(&key)
            .unwrap_or_else(|| std::process::abort());
        let packets = match (record.plan.profile, &mut record.control) {
            (
                GeneratedProfileV1::NativeFillArena1024,
                generated_shells::GeneratedControlV1::Arena1024(packets),
            )
            | (
                GeneratedProfileV1::IndependentFillArena1024,
                generated_shells::GeneratedControlV1::IndependentArena1024(packets),
            )
            | (
                GeneratedProfileV1::IndependentFillArena2048,
                generated_shells::GeneratedControlV1::IndependentArena2048(packets),
            ) => packets,
            _ => std::process::abort(),
        };
        let arena = record
            .arena
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let packets_original = packets.take().unwrap_or_else(|| std::process::abort());
        let data = arena.data.take().unwrap_or_else(|| std::process::abort());
        let admitted = admit_inputs(record.plan.profile, program, packets_original, data);
        let inputs = match admitted {
            Ok(inputs) => inputs,
            Err((original, data, error)) => {
                *packets = Some(original);
                arena.data = Some(data);
                return Err(self.generated_native_error_v1("arena dense admission", error));
            }
        };
        let storage = arena
            .storage
            .lower
            .take()
            .unwrap_or_else(|| std::process::abort());
        let memory = self
            .terminal_memory
            .take()
            .unwrap_or_else(|| std::process::abort());
        let session = match (inputs, storage) {
            (InputsV1::Ordered(inputs), LowerStorageV1::Original(storage)) => memory
                .create_compute_aql_queue_with_native_fill_arena_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(SessionV1::Ordered),
            (InputsV1::Independent(inputs), LowerStorageV1::Original(storage)) => memory
                .create_compute_aql_queue_with_independent_fill_arena_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(SessionV1::Independent),
            (InputsV1::Independent2048(inputs), LowerStorageV1::Independent2048(storage)) => memory
                .create_compute_aql_queue_with_independent_fill_arena2048_v1(
                    128 * 1024,
                    inputs,
                    storage,
                )
                .map(SessionV1::Independent2048),
            _ => std::process::abort(),
        }
        .map_err(|error| self.generated_native_error_v1("arena primary construction", error))?;
        self.generated_shells
            .get_mut(&key)
            .and_then(|record| record.arena.as_mut())
            .unwrap_or_else(|| std::process::abort())
            .session = Some(session);
        self.observe_generated_queue_creation_v1(0);
        Ok(())
    }
}

type RefusedInputsV1 = (
    ArenaPacketsV1,
    Gfx942FixedDispatchDataV1,
    fe2o3_kfd::Gfx942DispatchBindingErrorV1,
);

// Refusal returns both original owning inputs without a second allocation.
#[allow(clippy::result_large_err)]
fn admit_inputs(
    profile: GeneratedProfileV1,
    program: ValidatedKernelEnvelope<'_>,
    packets: ArenaPacketsV1,
    data: Gfx942FixedDispatchDataV1,
) -> Result<InputsV1<'_>, RefusedInputsV1> {
    let result = match (profile, packets) {
        (GeneratedProfileV1::NativeFillArena1024, ArenaPacketsV1::Original(packets)) => {
            Gfx942NativeFillArenaInputsV1::admit_local_outputs(program, packets, data)
                .map(InputsV1::Ordered)
        }
        (GeneratedProfileV1::IndependentFillArena1024, ArenaPacketsV1::Original(packets)) => {
            Gfx942IndependentFillArenaInputsV1::admit_local_outputs(program, packets, data)
                .map(InputsV1::Independent)
        }
        (
            GeneratedProfileV1::IndependentFillArena2048,
            ArenaPacketsV1::Independent2048(packets),
        ) => {
            return fe2o3_kfd::Gfx942IndependentFillArena2048InputsV1::admit_local_outputs(
                program, packets, data,
            )
            .map(InputsV1::Independent2048)
            .map_err(|failure| {
                let (_, packets, data, error) = failure.into_parts();
                (ArenaPacketsV1::Independent2048(packets), data, error)
            });
        }
        _ => std::process::abort(),
    };
    result.map_err(|failure| {
        let (_, packets, data, error) = failure.into_parts();
        (ArenaPacketsV1::Original(packets), data, error)
    })
}
