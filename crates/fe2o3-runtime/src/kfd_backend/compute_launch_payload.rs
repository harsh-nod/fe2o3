//! Optional retained launch-slice accounting, not aggregate host/native memory.

use super::*;
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1, RetainedResourceCreditsV1,
};
use std::ops::Deref;

// No Clone or mutable recipe accessor: an Arc alias shares the existing debit,
// while a distinct owned copy must pass admission again.
pub(super) struct RetainedComputeLaunchV1 {
    recipe: Option<OwnedComputeLaunchV1>,
    credits: Option<RetainedResourceCreditsV1>,
}

fn payload_bytes(kernarg_len: usize, binding_len: usize) -> Option<u64> {
    let binding_bytes = binding_len.checked_mul(core::mem::size_of::<BackendBindingV1>())?;
    if kernarg_len > isize::MAX as usize || binding_bytes > isize::MAX as usize {
        return None;
    }
    u64::try_from(kernarg_len.checked_add(binding_bytes)?).ok()
}

fn allocate_exact<T>(
    len: usize,
) -> Result<Vec<T>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD retained launch allocation failed"))?;
    Ok(values)
}

fn fill_exact<T: Copy>(
    mut values: Vec<T>,
    source: &[T],
    accounted: bool,
) -> Result<Box<[T]>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    if !values.is_empty()
        || values.capacity() < source.len()
        || (accounted && values.capacity() != source.len())
    {
        return Err(KfdRuntimeBackendV1::capacity(
            "KFD retained launch capacity differs from reservation",
        ));
    }
    values.extend_from_slice(source);
    // The accounted path already has exact capacity and needs no shrink.
    Ok(values.into_boxed_slice())
}

impl RetainedComputeLaunchV1 {
    pub(super) fn copy_from(
        launch: BackendLaunchV1<'_>,
        account: Option<&ResourceCreditAccountV1>,
    ) -> Result<Arc<Self>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        Self::copy_with(launch, account, allocate_exact, allocate_exact)
    }

    fn copy_with(
        launch: BackendLaunchV1<'_>,
        account: Option<&ResourceCreditAccountV1>,
        allocate_kernarg: impl FnOnce(
            usize,
        ) -> Result<
            Vec<u8>,
            RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
        >,
        allocate_bindings: impl FnOnce(
            usize,
        ) -> Result<
            Vec<BackendBindingV1>,
            RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
        >,
    ) -> Result<Arc<Self>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let bytes = payload_bytes(launch.explicit_kernarg.len(), launch.bindings.len())
            .ok_or_else(|| KfdRuntimeBackendV1::capacity("KFD retained launch size overflow"))?;
        let reservation = account
            .map(|account| {
                account
                    .reserve(
                        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
                    )
                    .map_err(|error| {
                        KfdRuntimeBackendV1::capacity(format!(
                            "KFD retained launch admission: {error}"
                        ))
                    })
            })
            .transpose()?;
        let explicit_kernarg = fill_exact(
            allocate_kernarg(launch.explicit_kernarg.len())?,
            launch.explicit_kernarg,
            account.is_some(),
        )?;
        let bindings = fill_exact(
            allocate_bindings(launch.bindings.len())?,
            launch.bindings,
            account.is_some(),
        )?;
        Ok(Arc::new(Self {
            recipe: Some(OwnedComputeLaunchV1 {
                stream: launch.stream,
                kernel: launch.kernel,
                explicit_kernarg,
                bindings,
                geometry: launch.geometry,
                semantic_launch: launch.semantic_launch,
            }),
            credits: reservation.map(|reservation| reservation.retain()),
        }))
    }

    #[cfg(test)]
    pub(super) fn unaccounted_for_test(recipe: OwnedComputeLaunchV1) -> Self {
        Self {
            recipe: Some(recipe),
            credits: None,
        }
    }
}

impl Deref for RetainedComputeLaunchV1 {
    type Target = OwnedComputeLaunchV1;

    fn deref(&self) -> &Self::Target {
        self.recipe.as_ref().expect("retained compute recipe")
    }
}

impl fmt::Debug for RetainedComputeLaunchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), formatter)
    }
}

impl PartialEq for RetainedComputeLaunchV1 {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl Eq for RetainedComputeLaunchV1 {}

fn dispose_then_refund<T>(storage: T, credits: Option<RetainedResourceCreditsV1>) {
    drop(storage);
    if let Some(credits) = credits {
        let _ = credits.release_after_disposal();
    }
}

impl Drop for RetainedComputeLaunchV1 {
    fn drop(&mut self) {
        dispose_then_refund(self.recipe.take(), self.credits.take());
    }
}

#[cfg(test)]
mod tests;
