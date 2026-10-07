use super::*;
use fe2o3_kfd::{Gfx942IndependentFillArenaInputsV1, Gfx942NativeFillArenaInputsV1};

enum InputsV1<'a> {
    Ordered(Gfx942NativeFillArenaInputsV1<'a>),
    Independent(Gfx942IndependentFillArenaInputsV1<'a>),
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
        if !matches!(
            plan.profile,
            GeneratedProfileV1::NativeFillArena1024 | GeneratedProfileV1::IndependentFillArena1024
        ) || plan.count != 1
            || roster.count != 1
            || bytes != roster.readback_bytes
            || storage.lower.is_none()
            || storage
                .initialization
                .as_ref()
                .is_none_or(|source| source.len() as u64 != bytes)
            || storage.cells.len() != SLOTS
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
            ) => packets,
            _ => std::process::abort(),
        };
        let arena = record
            .arena
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let packets_original = packets.take().unwrap_or_else(|| std::process::abort());
        let data = arena.data.take().unwrap_or_else(|| std::process::abort());
        let admitted = match record.plan.profile {
            GeneratedProfileV1::NativeFillArena1024 => {
                Gfx942NativeFillArenaInputsV1::admit_local_outputs(program, packets_original, data)
                    .map(InputsV1::Ordered)
            }
            GeneratedProfileV1::IndependentFillArena1024 => {
                Gfx942IndependentFillArenaInputsV1::admit_local_outputs(
                    program,
                    packets_original,
                    data,
                )
                .map(InputsV1::Independent)
            }
            _ => std::process::abort(),
        };
        let inputs = match admitted {
            Ok(inputs) => inputs,
            Err(failure) => {
                let (_, original, data, error) = failure.into_parts();
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
        let session = match inputs {
            InputsV1::Ordered(inputs) => memory
                .create_compute_aql_queue_with_native_fill_arena_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(SessionV1::Ordered),
            InputsV1::Independent(inputs) => memory
                .create_compute_aql_queue_with_independent_fill_arena_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(SessionV1::Independent),
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
