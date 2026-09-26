//! Retained module-image payloads, separate from materialized executable backing.

use super::*;
use fe2o3_amdhsa_loader::{
    AdmittedProfile, KernelClosureError, OwnedValidatedEnvelope, OwnedValidatedKernelEnvelope,
    PlanError, validate_owned,
};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceCreditUsageV1, ResourceKindV1,
    ResourceVectorV1, RetainedResourceCreditsV1,
};

#[cfg(test)]
mod tests;

struct HostImageOwnerV1 {
    validated: Option<OwnedValidatedEnvelope>,
    credits: Option<RetainedResourceCreditsV1>,
}

impl Drop for HostImageOwnerV1 {
    fn drop(&mut self) {
        drop(self.validated.take());
        if let Some(credits) = self.credits.take() {
            let _ = credits.release_after_disposal();
        }
    }
}

// No raw owning-envelope accessor: cloning it would bypass image-credit custody.
#[derive(Clone)]
pub(super) struct ResidentModuleImageV1(Arc<HostImageOwnerV1>);

pub(super) struct ResidentKernelImageV1 {
    // Destroy this image alias before the shared owner's final disposal/refund.
    validated: OwnedValidatedKernelEnvelope,
    image: ResidentModuleImageV1,
}

impl Clone for ResidentKernelImageV1 {
    fn clone(&self) -> Self {
        Self {
            validated: self.validated.clone(),
            image: self.image.clone(),
        }
    }
}

impl ResidentModuleImageV1 {
    pub(super) fn load(
        image: &[u8],
        account: Option<&ResourceCreditAccountV1>,
    ) -> Result<Self, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        Self::load_with(
            image,
            account,
            |len| {
                let mut bytes = Vec::new();
                bytes.try_reserve_exact(len).map_err(|_| {
                    KfdRuntimeBackendV1::capacity("KFD module image allocation failed")
                })?;
                Ok(bytes)
            },
            |bytes| validate_owned(bytes, AdmittedProfile::Gfx942XnackOffCov6),
        )
    }

    fn load_with(
        image: &[u8],
        account: Option<&ResourceCreditAccountV1>,
        allocate: impl FnOnce(
            usize,
        )
            -> Result<Vec<u8>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
        validate: impl FnOnce(Vec<u8>) -> Result<OwnedValidatedEnvelope, PlanError>,
    ) -> Result<Self, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let bytes = u64::try_from(image.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("KFD module image size overflow"))?;
        let reservation = account
            .map(|account| {
                account
                    .reserve(
                        ResourceVectorV1::ZERO
                            .with(ResourceKindV1::ExecutableHostImageBytes, bytes),
                    )
                    .map_err(|error| {
                        KfdRuntimeBackendV1::capacity(format!(
                            "KFD module image admission: {error}"
                        ))
                    })
            })
            .transpose()?;
        let mut owned = allocate(image.len())?;
        if !owned.is_empty()
            || owned.capacity() < image.len()
            || (account.is_some() && owned.capacity() != image.len())
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "KFD module image capacity differs from reservation",
            ));
        }
        owned.extend_from_slice(image);
        let validated = validate(owned).map_err(|error| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("invalid AMDHSA module: {error:?}"),
            )
        })?;
        Ok(Self(Arc::new(HostImageOwnerV1 {
            validated: Some(validated),
            credits: reservation.map(|reservation| reservation.retain()),
        })))
    }

    pub(super) fn bytes(&self) -> &[u8] {
        self.0.validated.as_ref().expect("live image owner").bytes()
    }

    #[cfg(test)]
    pub(super) fn validation_passes(&self) -> u64 {
        self.0
            .validated
            .as_ref()
            .expect("live image owner")
            .validation_passes()
    }

    pub(super) fn bind_kernel(
        &self,
        name: &str,
    ) -> Result<ResidentKernelImageV1, KernelClosureError> {
        let validated = self
            .0
            .validated
            .as_ref()
            .expect("live image owner")
            .bind_kernel(name)?;
        Ok(ResidentKernelImageV1 {
            validated,
            image: self.clone(),
        })
    }
}

impl ResidentKernelImageV1 {
    pub(super) fn selected_kernel(&self) -> &fe2o3_hsaco::InspectedKernel {
        self.validated.selected_kernel()
    }

    #[cfg(test)]
    pub(super) fn semantic_binding_passes(&self) -> u64 {
        self.validated.semantic_binding_passes()
    }

    pub(super) fn validated(&self) -> ValidatedKernelEnvelope<'_> {
        self.validated.validated()
    }
}

impl KfdRuntimeBackendV1 {
    /// Configures one immutable ceiling for retained KFD module-image payloads.
    /// Call once before resource creation. Zero bytes denies nonempty images;
    /// the record limit must be in the resource account's supported range.
    ///
    /// Each copied image is charged before allocation, hashing or parsing.
    /// Kernels and prepared launches share that charge until their last image
    /// owner is disposed. Duplicate loads allocate and charge separately.
    /// Unconfigured backends preserve their existing unaccounted behavior.
    /// Parser/cache metadata, account/Arc/allocator overhead, transport copies,
    /// generated materialized images, native backing and other backends are
    /// excluded. This is not aggregate process or device accounting.
    pub fn configure_host_image_budget_v1(
        &mut self,
        max_resident_bytes: u64,
        max_resident_images: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_pristine_native_resource_configuration_v1()?;
        if self.host_image_account.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "host-image limits must be configured once before resource creation",
            ));
        }
        let account = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO
                .with(ResourceKindV1::ExecutableHostImageBytes, max_resident_bytes),
            max_resident_images,
        )
        .map_err(|error| match error {
            ResourceCreditErrorV1::InvalidRecordCapacity => Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "invalid host-image record limit",
            ),
            error => Self::capacity(format!("KFD host-image account: {error}")),
        })?;
        self.host_image_account = Some(account);
        Ok(())
    }

    /// Inert module-image payload usage, available before native startup and
    /// after terminal failure. `None` means unconfigured, not zero residency.
    pub fn host_image_usage_v1(&self) -> Option<ResourceCreditUsageV1> {
        self.host_image_account
            .as_ref()
            .map(ResourceCreditAccountV1::usage)
    }
}
