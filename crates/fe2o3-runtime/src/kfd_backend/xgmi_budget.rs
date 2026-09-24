//! Ordered endpoint budgets reuse the native session's backing accounts.

#![forbid(unsafe_code)]

use super::*;
use fe2o3_kfd::{
    Gfx942DeviceBackingUsageV1, Gfx942HostVisibleBackingUsageV1, Gfx942XgmiAllocationDispositionV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Optional limits for one endpoint, fixed at construction. These cover padded
/// PUBLIC device backing and ordinary coherent GTT (including completion words),
/// not AQL rings, userptr controls, VM bootstrap, metadata or total process memory.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KfdNativeXgmiBackingBudgetV1 {
    pub device: Option<Gfx942DeviceBackingBudgetV1>,
    pub host_visible: Option<Gfx942HostVisibleBackingBudgetV1>,
}

/// Inert endpoint accounting, available even after terminal failure or shutdown.
/// `None` means unconfigured, not zero memory. This grants no release authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdNativeXgmiBackingUsageV1 {
    pub backend_device: u64,
    pub device: Option<Gfx942DeviceBackingUsageV1>,
    pub host_visible: Option<Gfx942HostVisibleBackingUsageV1>,
}

impl KfdNativeXgmiRuntimeBackendV1 {
    /// Observes the original argument-ordered endpoints without native calls.
    pub fn backing_usage_v1(&self) -> [KfdNativeXgmiBackingUsageV1; 2] {
        std::array::from_fn(|index| KfdNativeXgmiBackingUsageV1 {
            backend_device: self.descriptions[index].backend_device,
            device: self.sessions[index].device_backing_usage_v1(),
            host_visible: self.sessions[index].host_visible_backing_usage_v1(),
        })
    }
}

pub(super) fn acquire_sessions<D, S, E>(
    devices: [D; 2],
    budgets: [KfdNativeXgmiBackingBudgetV1; 2],
    mut acquire: impl FnMut(D, KfdNativeXgmiBackingBudgetV1) -> Result<S, E>,
) -> Result<[S; 2], E> {
    let [first, second] = devices;
    let first = acquire(first, budgets[0])?;
    // No inverse transition can return the first consumed device authority.
    // Neither an error nor unwind may abandon that acquired VM through Drop.
    let second = match catch_unwind(AssertUnwindSafe(|| acquire(second, budgets[1]))) {
        Ok(Ok(session)) => session,
        Ok(Err(_)) | Err(_) => std::process::abort(),
    };
    Ok([first, second])
}

pub(super) fn allocate<A, E: fmt::Display>(
    terminal: &mut bool,
    operation: impl FnOnce() -> Result<A, E>,
    disposition: impl FnOnce(&E) -> Gfx942XgmiAllocationDispositionV1,
) -> Result<A, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        operation().map_err(|error| {
            let rejected =
                disposition(&error) == Gfx942XgmiAllocationDispositionV1::RejectedCapacity;
            if !rejected {
                *terminal = true;
            }
            let detail = format!("native XGMI allocation: {error}");
            if rejected {
                KfdNativeXgmiRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    detail,
                )
            } else {
                RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    detail,
                ))
            }
        })
    }));
    match outcome {
        Ok(result) => result,
        Err(payload) => {
            *terminal = true;
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
mod tests;
