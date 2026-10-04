//! Target-local mode0 and trap-clear adapter. No debugger acceptance claim.
use super::storage::{DebugEmptyRetirementTransportV1, abi};
use super::{Context, MetadataErrorV1 as E};
use crate::memory::MemoryBackend;
use rustix::ioctl::{Opcode, Setter, Updater};
const SET_TRAP_HANDLER: Opcode = 0x4018_4b13;
const RUNTIME_ENABLE: Opcode = fe2o3_kfd_uapi::AMDKFD_IOC_RUNTIME_ENABLE as Opcode;

pub(super) struct NativeEmptyRetirementV1<'a> {
    context: &'a mut Context,
    opener_pid: u32,
}
impl<'a> NativeEmptyRetirementV1<'a> {
    pub(super) fn new(context: &'a mut Context) -> Self {
        let opener_pid = context.backend.opener_pid();
        Self {
            context,
            opener_pid,
        }
    }
}
impl DebugEmptyRetirementTransportV1 for NativeEmptyRetirementV1<'_> {
    fn check_currentness(&mut self) -> Result<(), E> {
        if self.opener_pid == 0 || self.opener_pid != std::process::id() {
            return Err(E::ProcessChanged);
        }
        self.context
            .check_currentness(true)
            .map_err(|_| E::Currentness)
    }
    fn disable_runtime(&mut self) -> Result<(), E> {
        self.check_currentness()?;
        let expected = abi::RuntimeDebugEnableAbiV1 {
            r_debug: 0,
            mode_mask: 0,
            capabilities_mask: 0,
        };
        let mut observed = expected;
        // SAFETY: exact installed input/output ABI; actual same Context fd and
        // exclusive complete record. The sticky owner retains all storage on
        // an error, interrupt or malformed output. No retry is attempted.
        unsafe {
            rustix::ioctl::ioctl(
                self.context.backend.kfd_fd(),
                Updater::<RUNTIME_ENABLE, abi::RuntimeDebugEnableAbiV1>::new(&mut observed),
            )
        }
        .map_err(|_| E::NativeRuntime)?;
        exact_disable_output(expected, observed)
    }
    fn clear_trap(&mut self) -> Result<(), E> {
        self.check_currentness()?;
        let args = abi::SetTrapHandlerAbiV1 {
            trap_base: 0,
            trap_memory: 0,
            gpu_id: self
                .context
                .backend
                .engineering_debug_device()
                .observation()
                .kfd_gpu_id(),
            reserved: 0,
        };
        // SAFETY: actual same retained device, exact setter ABI. The only caller
        // has completed queue/event destruction and runtime-disable return.
        unsafe {
            rustix::ioctl::ioctl(
                self.context.backend.kfd_fd(),
                Setter::<SET_TRAP_HANDLER, abi::SetTrapHandlerAbiV1>::new(args),
            )
        }
        .map_err(|_| E::NativeTrap)?;
        Ok(())
    }
}

fn exact_disable_output(
    expected: abi::RuntimeDebugEnableAbiV1,
    observed: abi::RuntimeDebugEnableAbiV1,
) -> Result<(), E> {
    if observed == expected {
        Ok(())
    } else {
        Err(E::RuntimeOutput)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_disable_uses_exact_mode_zero_and_refuses_every_mutated_word() {
        let expected = abi::RuntimeDebugEnableAbiV1 {
            r_debug: 0,
            mode_mask: 0,
            capabilities_mask: 0,
        };
        assert_eq!(RUNTIME_ENABLE, 0xc010_4b25);
        assert_eq!(core::mem::size_of_val(&expected), 16);
        exact_disable_output(expected, expected).unwrap();
        for observed in [
            abi::RuntimeDebugEnableAbiV1 {
                r_debug: 1,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                mode_mask: 1,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                mode_mask: 3,
                ..expected
            },
            abi::RuntimeDebugEnableAbiV1 {
                capabilities_mask: 1,
                ..expected
            },
        ] {
            assert_eq!(
                exact_disable_output(expected, observed),
                Err(E::RuntimeOutput)
            );
        }
    }
    #[test]
    fn clear_trap_keeps_the_existing_exact_input_only_abi() {
        assert_eq!(SET_TRAP_HANDLER, 0x4018_4b13);
        assert_eq!(core::mem::size_of::<abi::SetTrapHandlerAbiV1>(), 24);
    }
}
