//! Sole production syscall adapter for the consuming no-queue owner.
//! No public raw addresses, native-token constructor, retry or teardown path.
use super::storage::{DebugActivationTransportV1, abi};
use super::{Context, MetadataErrorV1};
use rustix::ioctl::{Opcode, Setter, Updater};

const SET_TRAP_HANDLER: Opcode = 0x4018_4b13;
const RUNTIME_ENABLE: Opcode = fe2o3_kfd_uapi::AMDKFD_IOC_RUNTIME_ENABLE as Opcode;

pub(super) struct NativeNoQueueTransportV1<'a> {
    context: &'a mut Context,
    opener_pid: u32,
}
impl<'a> NativeNoQueueTransportV1<'a> {
    pub(super) fn new(context: &'a mut Context) -> Self {
        Self {
            context,
            opener_pid: std::process::id(),
        }
    }
}
impl DebugActivationTransportV1 for NativeNoQueueTransportV1<'_> {
    fn check_currentness(&mut self) -> Result<(), MetadataErrorV1> {
        if self.opener_pid == 0 || self.opener_pid != std::process::id() {
            return Err(MetadataErrorV1::ProcessChanged);
        }
        self.context
            .check_currentness(true)
            .map_err(|_| MetadataErrorV1::Currentness)
    }
    fn register_trap(&mut self, trap_base: u64, gpu_id: u32) -> Result<(), MetadataErrorV1> {
        let args = abi::SetTrapHandlerAbiV1 {
            trap_base,
            // No queue/dispatch capability is available from this owner.
            // This is NOT a claim that the handler tolerates sampling with null TMA.
            trap_memory: 0,
            gpu_id,
            reserved: 0,
        };
        // SAFETY: exact installed x86_64 KFD 1.18 input ABI. The sole caller
        // derived the address from its retained, revalidated actual trap mapping.
        // Retention was armed before VM acquisition, including errors/unwind.
        unsafe {
            rustix::ioctl::ioctl(
                self.context.backend.kfd_fd(),
                Setter::<SET_TRAP_HANDLER, abi::SetTrapHandlerAbiV1>::new(args),
            )
        }
        .map_err(|_| MetadataErrorV1::NativeTrap)?;
        Ok(())
    }
    fn enable_runtime(&mut self, root_address: u64) -> Result<(), MetadataErrorV1> {
        let expected = abi::RuntimeDebugEnableAbiV1 {
            r_debug: root_address,
            mode_mask: 3,
            capabilities_mask: 0,
        };
        let mut args = expected;
        // SAFETY: exact installed x86_64 KFD runtime-enable ABI, not the plain
        // queue-exception profile. Root and every reachable pointee are already
        // owned by the consuming process-lifetime-retained custody.
        unsafe {
            rustix::ioctl::ioctl(
                self.context.backend.kfd_fd(),
                Updater::<RUNTIME_ENABLE, abi::RuntimeDebugEnableAbiV1>::new(&mut args),
            )
        }
        .map_err(|_| MetadataErrorV1::NativeRuntime)?;
        // This pinned driver leaves all fields unchanged; unknown output is not
        // silently upgraded to capability support. Native effects may still exist.
        checked_runtime_output_v1(expected, args)
    }
}

fn checked_runtime_output_v1(
    expected: abi::RuntimeDebugEnableAbiV1,
    observed: abi::RuntimeDebugEnableAbiV1,
) -> Result<(), MetadataErrorV1> {
    if observed != expected {
        Err(MetadataErrorV1::RuntimeOutput)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_runtime_wire_is_mode_three_and_unknown_outputs_fail_closed() {
        let expected = abi::RuntimeDebugEnableAbiV1 {
            r_debug: 0x8000,
            mode_mask: 3,
            capabilities_mask: 0,
        };
        assert_eq!(core::mem::size_of_val(&expected), 16);
        assert_eq!(checked_runtime_output_v1(expected, expected), Ok(()));
        for observed in [
            abi::RuntimeDebugEnableAbiV1 {
                r_debug: 0,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                r_debug: 0x9000,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                mode_mask: 1,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                mode_mask: 0,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                capabilities_mask: 1,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                capabilities_mask: u32::MAX,
                ..expected
            },
        ] {
            assert_eq!(
                checked_runtime_output_v1(expected, observed),
                Err(MetadataErrorV1::RuntimeOutput)
            );
        }
    }
    #[test]
    fn private_opcodes_and_zero_tma_noqueue_profile_are_exact() {
        assert_eq!(SET_TRAP_HANDLER, 0x4018_4b13);
        assert_eq!(RUNTIME_ENABLE, 0xc010_4b25);
        let wire = abi::SetTrapHandlerAbiV1 {
            trap_base: 0x4000,
            trap_memory: 0,
            gpu_id: 7,
            reserved: 0,
        };
        assert_eq!(core::mem::size_of_val(&wire), 24);
        assert_eq!(wire.trap_memory, 0);
        assert_eq!(wire.reserved, 0);
        // No actual syscall or claim that this TMA is safe for execution.
    }
}
