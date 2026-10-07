//! Prepaid metadata and all original DATA are rooted before native construction.

use super::*;
use fe2o3_kfd::{Gfx942NativeFillCohortMemberV1, Gfx942NativeFillResidentRegistryInputsV1};

impl KfdRuntimeBackendV1 {
    pub(crate) fn adopt_generated_registry4_v1<const N: usize>(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        programs: [ValidatedKernelEnvelope<'_>; N],
        buffers: [&[crate::Gfx942KfdDispatchBufferV1]; N],
        storage: RegistryStorageV1,
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
                "registry original request unavailable",
            ));
        }
        if registry_count(plan.profile) != Some(N)
            || storage.profile() != plan.profile
            || plan.count != N
            || roster.count != N
            || !self.validate_generated_shell_records_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.registry.is_none()
                    && record.arena.is_none()
                    && record.control.is_some()
                    && record.source_identity.matches(&roster.source_identity)
            })
            || buffers.iter().enumerate().any(|(i, buffers)| {
                buffers.len() != 1
                    || plan.members[i].is_none_or(|member| {
                        member.description.byte_len != buffers[0].bytes().len() as u64
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry original control, source or DATA mismatch",
            ));
        }
        self.check_registry4_device_v1(plan)?;
        let mut data = Vec::new();
        data.try_reserve_exact(N)
            .map_err(|_| Self::capacity("registry original DATA roster"))?;
        if data.capacity() > fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
            return Err(Self::capacity("registry actual DATA capacity"));
        }
        self.generated_shells
            .get_mut(&plan.key)
            .unwrap_or_else(|| std::process::abort())
            .registry = Some(RegistryV1 {
            phase: PhaseV1::Entering,
            data,
            storage: Some(storage),
            session: None,
            receipts: core::array::from_fn(|_| ReceiptV1::Ready),
            copied: [false; 16],
            profile: plan.profile,
            cycle: 0,
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.acquire_generated_vm_v1()?;
            let initialized = {
                let memory = self
                    .terminal_memory
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort());
                let native = self
                    .generated_shells
                    .get_mut(&plan.key)
                    .and_then(|record| record.registry.as_mut())
                    .unwrap_or_else(|| std::process::abort());
                buffers.into_iter().try_for_each(|buffers| {
                    initialize_generated_data_v1(memory, buffers, &mut native.data)
                })
            };
            initialized.map_err(|error| {
                self.generated_native_error_v1("registry DATA initialization", error)
            })?;
            self.bind_generated_registry4_v1(plan.key, programs)?;
            self.check_registry4_device_v1(plan)?;
            self.generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort())
                .phase = PhaseV1::Adopted;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    fn bind_generated_registry4_v1<const N: usize>(
        &mut self,
        key: u64,
        programs: [ValidatedKernelEnvelope<'_>; N],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let record = self
            .generated_shells
            .get_mut(&key)
            .unwrap_or_else(|| std::process::abort());
        let packets: &mut [_] = match &mut record.control {
            generated_shells::GeneratedControlV1::Registry4(packets)
            | generated_shells::GeneratedControlV1::Registry4Repeat2(packets) => packets,
            generated_shells::GeneratedControlV1::Registry16(packets) => packets,
            _ => std::process::abort(),
        };
        let native = record
            .registry
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        if native.data.len() != N
            || packets.len() != N
            || packets.iter().any(Option::is_none)
            || native.storage.is_none()
        {
            std::process::abort();
        }
        let mut data: [_; N] = core::array::from_fn(|_| native.data.pop());
        let mut index = 0;
        let members = programs.map(|program| {
            let member = Gfx942NativeFillCohortMemberV1::new(
                program,
                packets[index]
                    .take()
                    .unwrap_or_else(|| std::process::abort()),
                data[N - 1 - index]
                    .take()
                    .unwrap_or_else(|| std::process::abort()),
            );
            index += 1;
            member
        });
        let inputs = match native.profile {
            GeneratedProfileV1::NativeFillRegistry4
            | GeneratedProfileV1::NativeFillRegistry4Repeat2 => {
                admit_members(exact_array(members), packets, &mut native.data).map(Inputs::Four)
            }
            GeneratedProfileV1::NativeFillRegistry16 => {
                admit_members(exact_array(members), packets, &mut native.data).map(Inputs::Sixteen)
            }
            _ => std::process::abort(),
        }
        .map_err(|error| self.generated_native_error_v1("registry admission", error))?;
        let native = self
            .generated_shells
            .get_mut(&key)
            .and_then(|record| record.registry.as_mut())
            .unwrap_or_else(|| std::process::abort());
        let storage = native
            .storage
            .take()
            .unwrap_or_else(|| std::process::abort());
        let memory = self
            .terminal_memory
            .take()
            .unwrap_or_else(|| std::process::abort());
        let session = match (storage, inputs) {
            (RegistryStorageV1::Once(storage), Inputs::Four(inputs)) => memory
                .create_compute_aql_queue_with_native_fill_registry_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(Session::Once),
            (RegistryStorageV1::Repeat2(storage), Inputs::Four(inputs)) => memory
                .create_compute_aql_queue_with_native_fill_registry_repeat2_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(Session::Repeat2),
            (RegistryStorageV1::Sixteen(storage), Inputs::Sixteen(inputs)) => memory
                .create_compute_aql_queue_with_native_fill_resident_registry_v1(
                    KFD_RUNTIME_RING_BYTES_V1,
                    inputs,
                    storage,
                )
                .map(Session::Sixteen),
            _ => std::process::abort(),
        }
        .map_err(|error| self.generated_native_error_v1("registry primary construction", error))?;
        self.generated_shells
            .get_mut(&key)
            .and_then(|record| record.registry.as_mut())
            .unwrap_or_else(|| std::process::abort())
            .session = Some(session);
        self.observe_generated_queue_creation_v1(0);
        Ok(())
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "all original programs, packets and DATA remain inline until rooted lower construction"
)]
enum Inputs<'a> {
    Four(Gfx942NativeFillResidentRegistryInputsV1<'a, 4>),
    Sixteen(Gfx942NativeFillResidentRegistryInputsV1<'a, 16>),
}

fn admit_members<'a, const N: usize>(
    members: [Gfx942NativeFillCohortMemberV1<'a>; N],
    packets: &mut [Option<Gfx942FixedDispatchPacketV1>],
    data: &mut Vec<Gfx942FixedDispatchDataV1>,
) -> Result<Gfx942NativeFillResidentRegistryInputsV1<'a, N>, fe2o3_kfd::Gfx942DispatchBindingErrorV1>
{
    match Gfx942NativeFillResidentRegistryInputsV1::admit(members) {
        Ok(inputs) => Ok(inputs),
        Err(failure) => {
            let (members, error) = failure.into_parts();
            for (index, member) in members.into_iter().enumerate() {
                let (_, packet, original) = member.into_parts();
                packets[index] = Some(packet);
                data.push(original);
            }
            Err(error)
        }
    }
}

fn exact_array<T, const IN: usize, const OUT: usize>(original: [T; IN]) -> [T; OUT] {
    if IN != OUT {
        std::process::abort();
    }
    let mut original = original.into_iter();
    core::array::from_fn(|_| original.next().unwrap_or_else(|| std::process::abort()))
}
