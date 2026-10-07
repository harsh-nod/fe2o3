//! Bounded original receipt transitions, with distinct copied-result readiness.

use super::session::Poll;
use super::*;

impl KfdRuntimeBackendV1 {
    pub(crate) fn progress_generated_registry4_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_registry4_v1(plan, roster)?;
        if index >= plan.count {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry recipe index",
            ));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_registry4_device_v1(plan)?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            let session = native
                .session
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            let receipt = &mut native.receipts[index];
            let operation = match receipt {
                ReceiptV1::Ready | ReceiptV1::RetryReady => receipt.issue(|| session.submit(index)),
                ReceiptV1::Published(_) => {
                    let ReceiptV1::Published(batch) = core::mem::replace(
                        receipt,
                        ReceiptV1::HandedToLower(receipt::HandoffV1::Poll),
                    ) else {
                        std::process::abort();
                    };
                    match session.poll(batch) {
                        Ok(Poll::Pending(batch)) => {
                            *receipt = ReceiptV1::Published(batch);
                            Ok(())
                        }
                        Ok(Poll::Ready(completed)) => {
                            *receipt = ReceiptV1::Completed(completed);
                            Ok(())
                        }
                        Err(failure) => {
                            if let Some(original) = failure.refused {
                                *receipt = ReceiptV1::Published(original);
                            }
                            Err(failure.error)
                        }
                    }
                }
                #[allow(
                    clippy::result_large_err,
                    reason = "retry refusal returns the original completion inline without fallible allocation"
                )]
                ReceiptV1::Completed(_) => receipt
                    .recycle(|completed| {
                        session
                            .recycle(completed)
                            .map(|_| ())
                            .map_err(|failure| (failure.error, failure.retryable))
                    })
                    .map(|_| ()),
                ReceiptV1::Recycled => Ok(()),
                // Registry4 issues through the non-classifying receipt path.
                // Preserve unexpected singleton custody for terminal teardown.
                ReceiptV1::RejectedUnpublished { .. } | ReceiptV1::RejectedDisposed(_) => {
                    Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                        "registry cannot consume singleton rejected-publication custody",
                    ))
                }
                ReceiptV1::HandedToLower(_) => Err(
                    fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("registry unknown receipt"),
                ),
            };
            operation
                .map_err(|error| self.generated_native_error_v1("registry progress", error))?;
            self.check_registry4_device_v1(plan)?;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(self
            .generated_shells
            .get(&plan.key)
            .and_then(|record| record.registry.as_ref())
            .is_some_and(|native| matches!(native.receipts[index], ReceiptV1::Recycled)))
    }

    pub(crate) fn read_generated_registry4_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_registry4_v1(plan, roster)?;
        if index >= plan.count
            || plan.members[index]
                .is_none_or(|member| member.description.byte_len != destination.len() as u64)
            || !self
                .generated_shells
                .get(&plan.key)
                .and_then(|record| record.registry.as_ref())
                .is_some_and(|native| {
                    !native.copied[index] && matches!(native.receipts[index], ReceiptV1::Recycled)
                })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry original readback phase or length",
            ));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_registry4_device_v1(plan)?;
            let result = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .and_then(|native| native.session.as_mut())
                .unwrap_or_else(|| std::process::abort())
                .read_into(index, destination);
            result
                .map_err(|error| self.generated_native_error_v1("registry recipe copy", error))?;
            self.check_registry4_device_v1(plan)?;
            self.generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort())
                .copied[index] = true;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }
}
