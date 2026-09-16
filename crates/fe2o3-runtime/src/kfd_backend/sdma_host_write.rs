//! Borrowed host writes keep owners rooted through error conversion and unwind.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::DirectionalSdmaOpsV1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) fn resume_sdma_owner_panic_v1(
    payload: Box<dyn std::any::Any + Send>,
    poison: impl FnOnce(),
) -> ! {
    // Even a secondary payload destructor must not replace the original panic.
    core::mem::forget(catch_unwind(AssertUnwindSafe(poison)));
    resume_unwind(payload)
}

impl KfdRuntimeBackendV1 {
    fn settle_sdma_host_write_v1<T>(
        &mut self,
        result: std::thread::Result<Result<T, String>>,
        operation: &'static str,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => {
                self.poison_terminal_v1();
                Err(RuntimeBackendFailureV1::Terminal(
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Terminal,
                        format!("{operation}: {error}"),
                    ),
                ))
            }
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }

    pub(super) fn initialize_sdma_host_v1(
        &mut self,
        mut buffer: SdmaBufferOwnerV1,
        bytes: &[u8],
        operation: &'static str,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.directional_sdma_ops_v1()
                .write_host(&mut buffer, 0, bytes)
        }));
        if matches!(&result, Ok(Ok(()))) {
            return Ok(buffer);
        }
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Buffer(buffer));
        self.settle_sdma_host_write_v1(result, operation)?;
        unreachable!("failed host write cannot settle successfully")
    }

    pub(super) fn write_indexed_sdma_host_v1<T>(
        &mut self,
        allocation: u64,
        operation: &'static str,
        write: impl FnOnce(&mut DirectionalSdmaOpsV1<'_>, &mut SdmaBufferOwnerV1) -> Result<T, String>,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let buffer = match &mut self
                .allocations
                .get_mut(&allocation)
                .expect("admitted host allocation remains indexed")
                .sdma_storage
            {
                KfdRuntimeSdmaStorageV1::Host(buffer) => buffer,
                _ => unreachable!("checked host storage"),
            };
            #[cfg(test)]
            if let Some(driver) = self.scripted_sdma.as_mut() {
                return write(&mut DirectionalSdmaOpsV1::Scripted(driver), buffer);
            }
            write(
                &mut DirectionalSdmaOpsV1::Native(
                    self.queue
                        .as_mut()
                        .expect("persistent SDMA allocation retains queue"),
                ),
                buffer,
            )
        }));
        self.settle_sdma_host_write_v1(result, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_write_original_panic_survives_secondary_poison_and_payload_drop_panics() {
        struct Secondary;
        impl Drop for Secondary {
            fn drop(&mut self) {
                panic!("secondary panic payload was dropped");
            }
        }
        let original = Box::new(173_u64);
        let address = &*original as *const u64;
        let poison_calls = std::cell::Cell::new(0);
        let payload = catch_unwind(AssertUnwindSafe(|| {
            resume_sdma_owner_panic_v1(original, || {
                poison_calls.set(poison_calls.get() + 1);
                std::panic::panic_any(Secondary)
            })
        }))
        .unwrap_err();
        assert_eq!(poison_calls.get(), 1);
        let recovered = payload.downcast::<u64>().unwrap();
        assert_eq!(&*recovered as *const u64, address);
        assert_eq!(*recovered, 173);
    }
}
