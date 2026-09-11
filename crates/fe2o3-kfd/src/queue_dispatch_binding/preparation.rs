//! One preparation sequencer with externally rooted native and descriptive custody.

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;

use super::*;
use crate::shared_memory::{
    GttCpuWritableV1, GttExecutableImmutableV1, RetainedDispatchDataRosterV1,
};

type CodeCpu = SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>;
type CodeImmutable = SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>;
type CodeMapped = SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>;
type KernargCpu = SharedGttAllocationV1<KernargGttV1, GttCpuWritableV1>;
type KernargMapped = SharedGttAllocationV1<KernargGttV1, GttGpuAccessibleMutableV1>;

// Both implementations forward primitives only; planning and sequencing stay here.
pub(crate) trait PreparationMemoryV1 {
    fn retain_data(
        &mut self,
        data: &mut Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<RetainedDispatchDataRosterV1, MemorySessionError>;
    fn allocate_code(&mut self, bytes: usize) -> Result<CodeCpu, MemorySessionError>;
    fn write_code<R>(
        &mut self,
        token: &mut CodeCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError>;
    fn seal_code(&mut self, token: CodeCpu) -> Result<CodeImmutable, MemorySessionError>;
    fn map_code(&mut self, token: CodeImmutable) -> Result<CodeMapped, MemorySessionError>;
    fn retain_code(&mut self, token: CodeMapped) -> Result<CodeAuthority, MemorySessionError>;
    fn allocate_kernarg(&mut self, bytes: usize) -> Result<KernargCpu, MemorySessionError>;
    fn write_kernarg<R>(
        &mut self,
        token: &mut KernargCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError>;
    fn map_kernarg(&mut self, token: KernargCpu) -> Result<KernargMapped, MemorySessionError>;
    fn retain_kernarg(
        &mut self,
        token: KernargMapped,
    ) -> Result<KernargAuthority, MemorySessionError>;
    fn quarantine(&mut self);
}

impl PreparationMemoryV1 for SharedGttMemorySessionV1 {
    fn retain_data(
        &mut self,
        data: &mut Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<RetainedDispatchDataRosterV1, MemorySessionError> {
        self.retain_fixed_dispatch_data_v1(data)
    }

    fn allocate_code(&mut self, bytes: usize) -> Result<CodeCpu, MemorySessionError> {
        self.allocate_executable(bytes)
    }

    fn write_code<R>(
        &mut self,
        token: &mut CodeCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.with_bytes_mut(token, f)
    }

    fn seal_code(&mut self, token: CodeCpu) -> Result<CodeImmutable, MemorySessionError> {
        self.seal_executable(token)
    }

    fn map_code(&mut self, token: CodeImmutable) -> Result<CodeMapped, MemorySessionError> {
        self.map_executable_to_gpu(token)
    }

    fn retain_code(&mut self, token: CodeMapped) -> Result<CodeAuthority, MemorySessionError> {
        self.retain_aql_dispatch_code_resource(token)
    }

    fn allocate_kernarg(&mut self, bytes: usize) -> Result<KernargCpu, MemorySessionError> {
        SharedGttMemorySessionV1::allocate_kernarg(self, bytes)
    }

    fn write_kernarg<R>(
        &mut self,
        token: &mut KernargCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.with_bytes_mut(token, f)
    }

    fn map_kernarg(&mut self, token: KernargCpu) -> Result<KernargMapped, MemorySessionError> {
        self.map_to_gpu(token)
    }

    fn retain_kernarg(
        &mut self,
        token: KernargMapped,
    ) -> Result<KernargAuthority, MemorySessionError> {
        self.retain_aql_dispatch_kernarg_resource(token)
    }

    fn quarantine(&mut self) {
        let _ = self.quarantine_queue_composition("fixed dispatch preparation failure");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparationStageV1 {
    Fresh,
    Generation,
    Plan,
    Capacity,
    DataRetention,
    CodeAllocate(usize),
    CodeMaterialize(usize),
    CodeSeal(usize),
    CodeMap(usize),
    CodeRetain(usize),
    CodeResolve(usize),
    KernargAllocate,
    KernargMaterialize,
    KernargMap,
    KernargRetain,
    PacketResolve(usize),
    Commit,
    Complete,
    Transferred,
}

enum CodeStageV1 {
    Empty,
    Allocating,
    Cpu(CodeCpu),
    Immutable(CodeImmutable),
    Mapped(CodeMapped),
    Retained(CodeAuthority),
    // An observation linking this stage to R88 custody, never a replacement token.
    InSession(SharedGttAllocationIdentityV1),
}

enum KernargStageV1 {
    Empty,
    Allocating,
    Cpu(KernargCpu),
    Mapped(KernargMapped),
    Retained(KernargAuthority),
    InSession(SharedGttAllocationIdentityV1),
}

#[derive(Clone, Copy)]
struct ProgramIdentityV1 {
    authenticated: KernelIdentityInputsV1,
    dispatch_abi: [u8; 32],
}

pub(crate) struct FixedDispatchPreparationCustodyV1<const N: usize> {
    packets: [Gfx942FixedDispatchPacketV1; N],
    original_data: Vec<Gfx942FixedDispatchDataV1>,
    retained_data: Option<RetainedDispatchDataRosterV1>,
    generation: Option<DispatchGenerationOwnerV1>,
    plan: Option<FixedDispatchPreparationPlanV1>,
    program_identity: Vec<ProgramIdentityV1>,
    code: Vec<CodeAuthority>,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    current_code: CodeStageV1,
    current_materialized_sha256: Option<[u8; 32]>,
    kernarg: KernargStageV1,
    prepared_packets: Vec<PreparedDispatchPacketV1>,
    data_authorities: Vec<DispatchDataAuthorityV1>,
    data_premises: Vec<RetainedDataPremiseV1>,
    control: PersistentFixedDispatchControlStateV1,
    completed: Option<DispatchResourceOwnerV1>,
    stage: PreparationStageV1,
    native_started: bool,
    failed: bool,
    #[cfg(test)]
    fault: Option<(PreparationStageV1, bool)>,
}

impl<const N: usize> FixedDispatchPreparationCustodyV1<N> {
    pub(crate) fn new(
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Self {
        Self {
            packets,
            original_data: data,
            retained_data: None,
            generation: None,
            plan: None,
            program_identity: Vec::new(),
            code: Vec::new(),
            code_identity: Vec::new(),
            current_code: CodeStageV1::Empty,
            current_materialized_sha256: None,
            kernarg: KernargStageV1::Empty,
            prepared_packets: Vec::new(),
            data_authorities: Vec::new(),
            data_premises: Vec::new(),
            control: PersistentFixedDispatchControlStateV1::Ordinary,
            completed: None,
            stage: PreparationStageV1::Fresh,
            native_started: false,
            failed: false,
            #[cfg(test)]
            fault: None,
        }
    }

    fn step(&mut self, stage: PreparationStageV1) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.stage = stage;
        #[cfg(test)]
        if let Some((at, panic)) = self.fault
            && at == stage
        {
            if panic {
                std::panic::panic_any(("dispatch preparation", stage));
            }
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "injected preparation stage",
            ));
        }
        Ok(())
    }

    pub(super) fn prepare_in_place<M: PreparationMemoryV1>(
        &mut self,
        memory: &mut M,
        programs: &[ValidatedKernelEnvelope<'_>],
        generation: Result<DispatchGenerationOwnerV1, Gfx942DispatchBindingErrorV1>,
        control: PersistentFixedDispatchControlStateV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.stage != PreparationStageV1::Fresh || self.failed {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.control = control;
        match generation {
            Ok(generation) => self.generation = Some(generation),
            Err(error) => {
                self.stage = PreparationStageV1::Generation;
                self.failed = true;
                return Err(error);
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.step(PreparationStageV1::Generation)?;
            self.prepare_inner(memory, programs)
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.failed = true;
                if self.native_started {
                    memory.quarantine();
                }
                Err(error)
            }
            Err(payload) => {
                self.failed = true;
                if self.native_started {
                    memory.quarantine();
                }
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn prepare_inner<M: PreparationMemoryV1>(
        &mut self,
        memory: &mut M,
        programs: &[ValidatedKernelEnvelope<'_>],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.step(PreparationStageV1::Plan)?;
        if let PersistentFixedDispatchControlStateV1::Attached(identity) = self.control
            && (identity.binding_count() != self.original_data.len()
                || identity.content_roles[..identity.binding_count()]
                    .iter()
                    .any(Option::is_none))
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.original_data.len(),
                detail: "persistent compute data cardinality",
            });
        }
        let layouts: Vec<_> = self
            .original_data
            .iter()
            .map(Gfx942FixedDispatchDataV1::layout)
            .collect();
        let initialized: Vec<_> = self
            .original_data
            .iter()
            .map(Gfx942FixedDispatchDataV1::is_fully_initialized)
            .collect();
        self.plan = Some(plan_public_fixed_dispatch_resources(
            programs,
            &self.packets,
            &layouts,
            &initialized,
        )?);
        self.step(PreparationStageV1::Capacity)?;
        let capacity_error =
            |_| Gfx942DispatchBindingErrorV1::InvalidCode("preparation output capacity");
        self.program_identity
            .try_reserve_exact(programs.len())
            .map_err(capacity_error)?;
        self.code
            .try_reserve_exact(programs.len())
            .map_err(capacity_error)?;
        self.code_identity
            .try_reserve_exact(programs.len())
            .map_err(capacity_error)?;
        self.prepared_packets
            .try_reserve_exact(N)
            .map_err(capacity_error)?;
        self.data_authorities
            .try_reserve_exact(self.original_data.len())
            .map_err(capacity_error)?;
        self.data_premises
            .try_reserve_exact(self.original_data.len())
            .map_err(capacity_error)?;
        for kernel in programs {
            self.program_identity.push(ProgramIdentityV1 {
                authenticated: kernel.identity_inputs(),
                dispatch_abi: kernel
                    .dispatch_abi_identity()
                    .unwrap_or_else(|| kernel.identity_inputs().closure_sha256()),
            });
        }
        self.step(PreparationStageV1::DataRetention)?;
        self.retained_data = Some(memory.retain_data(&mut self.original_data)?);
        let retained = self.retained_data.as_ref().expect("retained data roster");
        let plan = self.plan.as_ref().expect("stored preparation plan");
        if retained.len() != plan.data.len()
            || retained.iter().zip(&plan.data).any(|(data, plan)| {
                data.layout != plan.layout || data.fully_initialized != plan.fully_initialized
            })
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "retained data plan mismatch",
            ));
        }

        for (index, kernel) in programs.iter().enumerate() {
            self.step(PreparationStageV1::CodeAllocate(index))?;
            self.native_started = true;
            self.current_code = CodeStageV1::Allocating;
            self.current_code = CodeStageV1::Cpu(
                memory.allocate_code(self.plan.as_ref().unwrap().programs[index].image_len)?,
            );
            self.step(PreparationStageV1::CodeMaterialize(index))?;
            let CodeStageV1::Cpu(token) = &mut self.current_code else {
                unreachable!("new code token")
            };
            self.current_materialized_sha256 = Some(
                memory
                    .write_code(token, |bytes| {
                        kernel
                            .materialize_into(bytes)
                            .map(|()| Sha256::digest(bytes).into())
                    })?
                    .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("materialization"))?,
            );

            self.step(PreparationStageV1::CodeSeal(index))?;
            let CodeStageV1::Cpu(token) = &self.current_code else {
                unreachable!("materialized code token")
            };
            let marker = CodeStageV1::InSession(token.storage_identity());
            let CodeStageV1::Cpu(token) = core::mem::replace(&mut self.current_code, marker) else {
                unreachable!()
            };
            self.current_code = CodeStageV1::Immutable(memory.seal_code(token)?);

            self.step(PreparationStageV1::CodeMap(index))?;
            let CodeStageV1::Immutable(token) = &self.current_code else {
                unreachable!("sealed code token")
            };
            let marker = CodeStageV1::InSession(token.storage_identity());
            let CodeStageV1::Immutable(token) = core::mem::replace(&mut self.current_code, marker)
            else {
                unreachable!()
            };
            self.current_code = CodeStageV1::Mapped(memory.map_code(token)?);

            self.step(PreparationStageV1::CodeRetain(index))?;
            let CodeStageV1::Mapped(token) = &self.current_code else {
                unreachable!("mapped code token")
            };
            let marker = CodeStageV1::InSession(token.storage_identity());
            let CodeStageV1::Mapped(token) = core::mem::replace(&mut self.current_code, marker)
            else {
                unreachable!()
            };
            self.current_code = CodeStageV1::Retained(memory.retain_code(token)?);

            self.step(PreparationStageV1::CodeResolve(index))?;
            let CodeStageV1::Retained(allocation) = &self.current_code else {
                unreachable!("retained code token")
            };
            let descriptor_address = allocation
                .facts()
                .checked_gpu_subrange(
                    self.plan.as_ref().unwrap().programs[index].descriptor_offset,
                    KERNEL_DESCRIPTOR_BYTES_V1,
                    64,
                )
                .and_then(|address| ObservedGpuAddressV1::new(address).ok())
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                    "resolved descriptor address",
                ))?;
            let identity = ResolvedCodeIdentityV1 {
                authenticated: self.program_identity[index].authenticated,
                dispatch_abi_identity: self.program_identity[index].dispatch_abi,
                materialized_sha256: self
                    .current_materialized_sha256
                    .expect("materialized digest"),
                mapping: allocation.facts().mapping(),
                descriptor_address,
            };
            assert!(
                self.code.len() < self.code.capacity()
                    && self.code_identity.len() < self.code_identity.capacity()
            );
            let CodeStageV1::Retained(allocation) =
                core::mem::replace(&mut self.current_code, CodeStageV1::Empty)
            else {
                unreachable!()
            };
            self.code.push(allocation);
            self.code_identity.push(identity);
            self.current_materialized_sha256 = None;
        }

        self.step(PreparationStageV1::KernargAllocate)?;
        self.kernarg = KernargStageV1::Allocating;
        self.kernarg = KernargStageV1::Cpu(
            memory.allocate_kernarg(self.plan.as_ref().unwrap().kernarg_arena_bytes)?,
        );
        self.step(PreparationStageV1::KernargMaterialize)?;
        let KernargStageV1::Cpu(token) = &mut self.kernarg else {
            unreachable!("new kernarg token")
        };
        let plan = self.plan.as_ref().unwrap();
        let data = self.retained_data.as_ref().unwrap();
        memory.write_kernarg(token, |bytes| {
            bytes.fill(0);
            for (input, packet) in self.packets.iter().zip(&plan.packets) {
                let start = packet.kernarg_offset;
                let end = start + input.kernarg_bytes.len();
                let packet_bytes = &mut bytes[start..end];
                packet_bytes.copy_from_slice(&input.kernarg_bytes);
                for patch in &packet.patches {
                    let address = data[patch.data_index]
                        .authority
                        .checked_gpu_subrange(
                            patch.data_byte_offset,
                            patch.required_bytes,
                            patch.required_alignment,
                        )
                        .expect("public dispatch preflight checked pointer range");
                    packet_bytes[patch.byte_offset..patch.byte_offset + 8]
                        .copy_from_slice(&address.to_le_bytes());
                }
                match (
                    plan.programs[input.program_index].implicit_kernarg.as_ref(),
                    packet.implicit_kernarg,
                ) {
                    (Some(plan), Some(values)) => {
                        initialize_cov6_implicit_kernarg(packet_bytes, plan, values)
                    }
                    (None, None) => {}
                    _ => unreachable!("implicit-kernarg preflight plan/value mismatch"),
                }
            }
        })?;
        self.step(PreparationStageV1::KernargMap)?;
        let KernargStageV1::Cpu(token) = &self.kernarg else {
            unreachable!("materialized kernarg token")
        };
        let marker = KernargStageV1::InSession(token.storage_identity());
        let KernargStageV1::Cpu(token) = core::mem::replace(&mut self.kernarg, marker) else {
            unreachable!()
        };
        self.kernarg = KernargStageV1::Mapped(memory.map_kernarg(token)?);
        self.step(PreparationStageV1::KernargRetain)?;
        let KernargStageV1::Mapped(token) = &self.kernarg else {
            unreachable!("mapped kernarg token")
        };
        let marker = KernargStageV1::InSession(token.storage_identity());
        let KernargStageV1::Mapped(token) = core::mem::replace(&mut self.kernarg, marker) else {
            unreachable!()
        };
        self.kernarg = KernargStageV1::Retained(memory.retain_kernarg(token)?);

        for index in 0..N {
            self.step(PreparationStageV1::PacketResolve(index))?;
            let input = &self.packets[index];
            let packet = &self.plan.as_ref().unwrap().packets[index];
            let KernargStageV1::Retained(kernarg) = &self.kernarg else {
                unreachable!("retained kernarg")
            };
            let kernarg_address = kernarg
                .facts()
                .checked_gpu_subrange(
                    packet.kernarg_offset as u64,
                    input.kernarg_bytes.len() as u64,
                    packet.kernarg_alignment as u64,
                )
                .and_then(|address| ObservedGpuAddressV1::new(address).ok())
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet: index,
                    detail: "mapped kernarg address",
                })?;
            self.prepared_packets.push(PreparedDispatchPacketV1 {
                geometry: input.geometry,
                ordering: input.ordering,
                private_segment_size: packet.private_segment_size,
                group_segment_size: packet.group_segment_size,
                kernarg_address,
                kernarg_alignment: packet.kernarg_alignment as u64,
                kernarg_mapping: kernarg.facts().mapping(),
                kernarg_layout_identity: self.code_identity[input.program_index]
                    .dispatch_abi_identity,
                code_bound_kernarg_layout: true,
                code_index: input.program_index,
            });
        }
        self.commit()?;
        self.step(PreparationStageV1::Complete)
    }

    fn commit(&mut self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.step(PreparationStageV1::Commit)?;
        let retained = self.retained_data.as_mut().expect("retained data");
        let plan = self.plan.as_mut().expect("retained plan");
        let roles = match self.control {
            PersistentFixedDispatchControlStateV1::Ordinary => None,
            PersistentFixedDispatchControlStateV1::Attached(identity) => Some(identity),
            _ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        };
        assert!(self.completed.is_none() && self.generation.is_some());
        assert!(matches!(self.kernarg, KernargStageV1::Retained(_)));
        assert_eq!(retained.len(), plan.data.len());
        assert!(self.data_authorities.is_empty() && self.data_premises.is_empty());
        assert!(
            self.data_authorities.capacity() >= retained.len()
                && self.data_premises.capacity() >= retained.len()
        );
        // Everything below is a pre-capacity move. No callback, allocation or
        // fallible validation may separate these owners from completed custody.
        for (input, plan) in retained.drain(..).zip(plan.data.drain(..)) {
            self.data_authorities.push(input.authority);
            self.data_premises.push(RetainedDataPremiseV1 {
                layout: plan.layout,
                role_identity: [0; 32],
                valid_bytes: plan.layout.requested_bytes(),
                effect: plan.effect,
                initialized_content: input.initialized_content,
                fully_initialized: plan.fully_initialized,
                writable_ranges: plan.writable_ranges,
                completed_snapshots: plan.completed_snapshots,
            });
        }
        let KernargStageV1::Retained(kernarg) =
            core::mem::replace(&mut self.kernarg, KernargStageV1::Empty)
        else {
            unreachable!()
        };
        self.completed = Some(DispatchResourceOwnerV1 {
            code: core::mem::take(&mut self.code),
            code_identity: core::mem::take(&mut self.code_identity),
            kernarg,
            packets: core::mem::take(&mut self.prepared_packets),
            data: core::mem::take(&mut self.data_authorities),
            data_premises: core::mem::take(&mut self.data_premises),
            generation: self.generation.take().unwrap(),
            persistent_control: self.control,
        });
        if let Some(roles) = roles {
            let completed = self.completed.as_mut().unwrap();
            for (premise, role) in completed
                .data_premises
                .iter_mut()
                .zip(roles.content_roles.into_iter().flatten())
            {
                premise.role_identity = role.identity();
            }
        }
        Ok(())
    }

    pub(in crate::queue) fn completed(
        &self,
    ) -> Result<&DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
        if self.stage != PreparationStageV1::Complete || self.failed {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.completed
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }

    pub(in crate::queue) fn take_completed(
        &mut self,
    ) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
        if self.stage != PreparationStageV1::Complete || self.failed || self.completed.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.stage = PreparationStageV1::Transferred;
        Ok(self.completed.take().expect("checked completed dispatch"))
    }
}
