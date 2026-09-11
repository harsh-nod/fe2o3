//! Private primitive forwarding for the one primary-construction sequence.

use super::*;
use crate::shared_memory::Gfx942DeviceMemoryDispatchAuthorityV1;

pub(in crate::queue::live) trait PrimaryMemoryV1:
    construction::RingMemoryV1
{
    fn allocate_ring(
        &mut self,
        backing: QueueRingBackingV1,
        bytes: usize,
    ) -> Result<CpuRingAuthorityV1, MemorySessionError>;
    fn initialize_ring(
        &mut self,
        ring: &mut CpuRingAuthorityV1,
    ) -> Result<Result<(), NativeAqlSubmissionErrorV1>, MemorySessionError>;
    fn allocate_userptr_aql_control(
        &mut self,
    ) -> Result<Cpu<UserptrAqlControlGttV1>, MemorySessionError>;
    fn allocate_host_visible_coherent(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<HostVisibleCoherentGttV1>, MemorySessionError>;
    fn allocate_executable(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<ExecutableGttV1>, MemorySessionError>;
    fn with_bytes_mut<P: GttProfileV1, R>(
        &mut self,
        token: &mut Cpu<P>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError>;
    fn preflight_cpu_queue_token_v1<P: GttProfileV1>(
        &self,
        token: &Cpu<P>,
    ) -> Result<(), MemorySessionError>;
    fn preflight_mapped_queue_token_v1<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &self,
        token: &Mapped<P>,
    ) -> Result<(), MemorySessionError>;
    fn preflight_immutable_queue_token_v1(&self, token: &Sealed) -> Result<(), MemorySessionError>;
    fn preflight_executable_queue_token_v1(
        &self,
        token: &Executable,
    ) -> Result<(), MemorySessionError>;
    fn map_to_gpu<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &mut self,
        token: Cpu<P>,
    ) -> Result<Mapped<P>, MemorySessionError>;
    fn seal_executable(
        &mut self,
        token: Cpu<ExecutableGttV1>,
    ) -> Result<Sealed, MemorySessionError>;
    fn map_executable_to_gpu(&mut self, token: Sealed) -> Result<Executable, MemorySessionError>;
    fn retain_aql_control_resource(
        &mut self,
        token: Mapped<UserptrAqlControlGttV1>,
    ) -> Result<ControlAuthority, MemorySessionError>;
    fn retain_aql_completion_signal_resource(
        &mut self,
        token: Mapped<HostVisibleCoherentGttV1>,
    ) -> Result<CompletionSignalAuthority, MemorySessionError>;
    fn retain_aql_eop_resource(
        &mut self,
        token: Executable,
    ) -> Result<EopAuthority, MemorySessionError>;
    fn retain_aql_context_save_resource(
        &mut self,
        token: Executable,
    ) -> Result<ContextSaveAuthority, MemorySessionError>;
    fn check_queue_currentness(&mut self) -> Result<(), MemorySessionError>;
    fn queue_model_device(&self) -> fe2o3_runtime_model::ModelDeviceAdmissionV1;
    fn take_queue_model_foundation(&mut self)
    -> Result<QueueModelFoundationV1, MemorySessionError>;
    fn take_queue_model_foundation_with_dispatch_memory(
        &mut self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<QueueModelFoundationV1, MemorySessionError>;
    fn authenticate_queue_model_foundation(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError>;
    fn opener_pid(&self) -> u32;
    fn create_queue(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs>;
    fn update_queue(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs>;
    fn destroy_queue(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs>;
}

impl PrimaryMemoryV1 for SharedGttMemorySessionV1 {
    fn allocate_ring(
        &mut self,
        backing: QueueRingBackingV1,
        bytes: usize,
    ) -> Result<CpuRingAuthorityV1, MemorySessionError> {
        CpuRingAuthorityV1::allocate(self, backing, bytes)
    }
    fn initialize_ring(
        &mut self,
        ring: &mut CpuRingAuthorityV1,
    ) -> Result<Result<(), NativeAqlSubmissionErrorV1>, MemorySessionError> {
        ring.initialize_invalid(self)
    }
    fn allocate_userptr_aql_control(
        &mut self,
    ) -> Result<Cpu<UserptrAqlControlGttV1>, MemorySessionError> {
        Self::allocate_userptr_aql_control(self)
    }
    fn allocate_host_visible_coherent(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<HostVisibleCoherentGttV1>, MemorySessionError> {
        Self::allocate_host_visible_coherent(self, bytes)
    }
    fn allocate_executable(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<ExecutableGttV1>, MemorySessionError> {
        Self::allocate_executable(self, bytes)
    }
    fn with_bytes_mut<P: GttProfileV1, R>(
        &mut self,
        token: &mut Cpu<P>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        Self::with_bytes_mut(self, token, f)
    }
    fn preflight_cpu_queue_token_v1<P: GttProfileV1>(
        &self,
        token: &Cpu<P>,
    ) -> Result<(), MemorySessionError> {
        Self::preflight_cpu_queue_token_v1(self, token)
    }
    fn preflight_mapped_queue_token_v1<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &self,
        token: &Mapped<P>,
    ) -> Result<(), MemorySessionError> {
        Self::preflight_mapped_queue_token_v1(self, token)
    }
    fn preflight_immutable_queue_token_v1(&self, token: &Sealed) -> Result<(), MemorySessionError> {
        Self::preflight_immutable_queue_token_v1(self, token)
    }
    fn preflight_executable_queue_token_v1(
        &self,
        token: &Executable,
    ) -> Result<(), MemorySessionError> {
        Self::preflight_executable_queue_token_v1(self, token)
    }
    fn map_to_gpu<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &mut self,
        token: Cpu<P>,
    ) -> Result<Mapped<P>, MemorySessionError> {
        Self::map_to_gpu(self, token)
    }
    fn seal_executable(
        &mut self,
        token: Cpu<ExecutableGttV1>,
    ) -> Result<Sealed, MemorySessionError> {
        Self::seal_executable(self, token)
    }
    fn map_executable_to_gpu(&mut self, token: Sealed) -> Result<Executable, MemorySessionError> {
        Self::map_executable_to_gpu(self, token)
    }
    fn retain_aql_control_resource(
        &mut self,
        token: Mapped<UserptrAqlControlGttV1>,
    ) -> Result<ControlAuthority, MemorySessionError> {
        Self::retain_aql_control_resource(self, token)
    }
    fn retain_aql_completion_signal_resource(
        &mut self,
        token: Mapped<HostVisibleCoherentGttV1>,
    ) -> Result<CompletionSignalAuthority, MemorySessionError> {
        Self::retain_aql_completion_signal_resource(self, token)
    }
    fn retain_aql_eop_resource(
        &mut self,
        token: Executable,
    ) -> Result<EopAuthority, MemorySessionError> {
        Self::retain_aql_eop_resource(self, token)
    }
    fn retain_aql_context_save_resource(
        &mut self,
        token: Executable,
    ) -> Result<ContextSaveAuthority, MemorySessionError> {
        Self::retain_aql_context_save_resource(self, token)
    }
    fn check_queue_currentness(&mut self) -> Result<(), MemorySessionError> {
        Self::check_queue_currentness(self)
    }
    fn queue_model_device(&self) -> fe2o3_runtime_model::ModelDeviceAdmissionV1 {
        Self::queue_model_device(self)
    }
    fn take_queue_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        Self::take_queue_model_foundation(self)
    }
    fn take_queue_model_foundation_with_dispatch_memory(
        &mut self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        Self::take_queue_model_foundation_with_dispatch_memory(self, authorities)
    }
    fn authenticate_queue_model_foundation(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError> {
        Self::authenticate_queue_model_foundation(self, foundation)
    }
    fn opener_pid(&self) -> u32 {
        Self::opener_pid(self)
    }
    fn create_queue(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs> {
        let status = match crate::queue_linux::create_queue(self.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }
    fn update_queue(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs> {
        let status = match crate::queue_linux::update_queue(self.kfd_fd(), &args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }
    fn destroy_queue(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs> {
        let status = match crate::queue_linux::destroy_queue(self.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }
}
