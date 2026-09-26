//! Allocation routing commits only after a child reports an owned allocation.

use super::*;
use crate::{RuntimeAllocationRequestWitnessV1, RuntimeRequestAllocationResultV1};
use std::panic::{AssertUnwindSafe, catch_unwind};

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn allocate_requested_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
        witness: RuntimeAllocationRequestWitnessV1<'_>,
    ) -> RuntimeRequestAllocationResultV1<KfdRuntimeBackendErrorV1> {
        let admission = (|| {
            self.require_live()?;
            if self.request_policy != multi_admission::MultiRequestPolicyV1::Required {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "multi-device backend has no required request profile",
                ));
            }
            let child = self.child_for_device(device)?;
            let backend = self.children.get(child).ok_or_else(|| {
                RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "invalid multi-device child index",
                ))
            })?;
            let valid = backend.request_binding_v1()?.is_some_and(|binding| {
                binding.backend_device_v1() == device && witness.matches_v1(binding, byte_len)
            });
            if !valid {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "missing or mismatched multi-device request witness",
                ));
            }
            Ok(child)
        })();
        match admission {
            Err(error) => RuntimeRequestAllocationResultV1::Outcome(self.latch(Err(error))),
            Ok(child) => self.route_allocation_v1(child, |backend| {
                backend.allocate_with_request_v1(device, kind, byte_len, alignment, witness)
            }),
        }
    }

    pub(super) fn route_allocation_v1(
        &mut self,
        child: usize,
        allocate: impl FnOnce(
            &mut KfdRuntimeBackendV1,
        ) -> RuntimeRequestAllocationResultV1<KfdRuntimeBackendErrorV1>,
    ) -> RuntimeRequestAllocationResultV1<KfdRuntimeBackendErrorV1> {
        use RuntimeRequestAllocationResultV1::Outcome;
        let prepared = (|| {
            self.require_live()?;
            if child >= self.children.len()
                || self.next_handle == 0
                || self.allocations.contains_key(&self.next_handle)
            {
                self.terminal = true;
                return Err(RuntimeBackendFailureV1::Terminal(
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Terminal,
                        "invalid multi-device allocation route state",
                    ),
                ));
            }
            let next = self.next_handle.checked_add(1).ok_or_else(|| {
                KfdRuntimeBackendV1::capacity("multi-device routing handle space exhausted")
            })?;
            Self::reserve_route(
                &mut self.allocations,
                "multi-device allocation route allocation failed",
            )?;
            Ok((self.next_handle, next))
        })();
        let (id, next) = match prepared {
            Ok(ids) => ids,
            Err(error) => return Outcome(Err(error)),
        };
        let result = match catch_unwind(AssertUnwindSafe(|| allocate(&mut self.children[child]))) {
            Ok(result) => result,
            Err(payload) => {
                self.terminal = true;
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    self.children[child].poison_terminal_v1();
                })
            }
        };
        match result {
            Outcome(Ok(RuntimeBackendAllocationOutcomeV1::Allocated(local))) => {
                if local == 0 {
                    self.terminal = true;
                    return Outcome(Err(self.children[child]
                        .terminal_error("child returned a zero allocation handle")));
                }
                // The concrete child enforces local uniqueness at insertion.
                // Routing storage and this vacant ID were reserved before effects.
                self.allocations.insert(id, RoutedHandleV1 { child, local });
                self.next_handle = next;
                Outcome(Ok(RuntimeBackendAllocationOutcomeV1::Allocated(id)))
            }
            Outcome(result) => Outcome(self.latch(result)),
            RuntimeRequestAllocationResultV1::Unsupported => {
                RuntimeRequestAllocationResultV1::Unsupported
            }
        }
    }
}

#[cfg(test)]
mod tests;
