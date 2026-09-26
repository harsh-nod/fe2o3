//! Immutable qualification capacity and its host metadata custody.

use super::*;
use fe2o3_kfd::{Gfx942FixedDispatchCapacityProfileV1, Gfx942FixedDispatchCapacityV1};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceVectorV1,
    host_metadata_table_payload_bytes_v1,
};

#[derive(Default)]
pub(super) struct RuntimeDispatchCapacityV1 {
    native: Gfx942FixedDispatchCapacityV1,
    account: Option<ResourceCreditAccountV1>,
}

impl RuntimeDispatchCapacityV1 {
    #[cfg(feature = "scale-qualification")]
    fn qualification_1024(account: ResourceCreditAccountV1) -> Self {
        Self {
            native: Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone()),
            account: Some(account),
        }
    }

    pub(super) fn native(&self) -> &Gfx942FixedDispatchCapacityV1 {
        &self.native
    }

    pub(super) fn is_scaled(&self) -> bool {
        self.native.profile() == Gfx942FixedDispatchCapacityProfileV1::Qualification1024
    }

    pub(super) fn custody_limit(&self) -> usize {
        if self.is_scaled() {
            self.native.profile().slots()
        } else {
            MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1
        }
    }
}

pub(super) struct RuntimeDispatchStateV1 {
    pub(super) capacity: RuntimeDispatchCapacityV1,
    pub(super) primary: RuntimeComputePipelineV1,
    pub(super) auxiliary: Vec<NativeComputeLaneRuntimeV1>,
}

impl RuntimeDispatchStateV1 {
    pub(super) fn try_new(
        capacity: RuntimeDispatchCapacityV1,
    ) -> Result<Self, ResourceCreditErrorV1> {
        let primary = RuntimeComputePipelineV1::try_vacant(
            capacity.native.profile(),
            capacity.account.as_ref(),
        )?;
        let mut auxiliary = Vec::new();
        auxiliary
            .try_reserve_exact(KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1 - 1)
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        for _ in 1..KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1 {
            auxiliary.push(NativeComputeLaneRuntimeV1 {
                owner_stream: None,
                active: None,
                pipeline: RuntimeComputePipelineV1::try_vacant(
                    capacity.native.profile(),
                    capacity.account.as_ref(),
                )?,
                resident_data: None,
                recycled_dispatch: None,
            });
        }
        Ok(Self {
            capacity,
            primary,
            auxiliary,
        })
    }
}

impl RuntimeAllocationCustodyV1 {
    pub(super) fn try_new(
        capacity: &RuntimeDispatchCapacityV1,
    ) -> Result<Self, ResourceCreditErrorV1> {
        let len = if capacity.is_scaled() {
            capacity.custody_limit()
        } else {
            1
        };
        let reservation = capacity
            .account
            .as_ref()
            .map(|account| {
                let bytes =
                    host_metadata_table_payload_bytes_v1::<RuntimeAllocationCustodyOwnerV1>(len)
                        .ok_or(ResourceCreditErrorV1::AllocationFailed)?;
                account.reserve(
                    ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
                )
            })
            .transpose()?;
        let mut owners = VecDeque::new();
        if capacity.is_scaled() {
            owners.try_reserve_exact(len)
        } else {
            owners.try_reserve(1)
        }
        .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        if capacity.is_scaled() && owners.capacity() != len {
            return Err(ResourceCreditErrorV1::AllocationFailed);
        }
        Ok(Self {
            owners,
            sole_stream: None,
            owner_counts: [0; 2],
            metadata_credits: reservation.map(|reservation| reservation.retain()),
        })
    }
}

impl core::fmt::Debug for RuntimeAllocationCustodyV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RuntimeAllocationCustodyV1")
            .field("owners", &self.owners)
            .field("sole_stream", &self.sole_stream)
            .field("owner_counts", &self.owner_counts)
            .field("accounted", &self.metadata_credits.is_some())
            .finish()
    }
}

impl Drop for RuntimeAllocationCustodyV1 {
    fn drop(&mut self) {
        drop(core::mem::take(&mut self.owners));
        if let Some(credits) = self.metadata_credits.take() {
            let _ = credits.release_after_disposal();
        }
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn require_default_dispatch_capacity_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.dispatch_capacity.is_scaled() {
            Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "scaled qualification supports only materialized HostVisible exact vecadd",
            ))
        } else {
            Ok(())
        }
    }

    /// Opens the exact vecadd development profile with 1024 epochs per lane.
    ///
    /// Both runtime pipeline tables are reserved before opening KFD. Native
    /// epoch tables and per-allocation custody rosters share the same account.
    /// The budget covers requested table payloads only, not aggregate memory;
    /// include native replacement peaks and one record per live table. This
    /// grants no generated, DeviceLocal, persistent, atomic or collective launch
    /// authority and is not evidence of native scale or performance qualification.
    #[cfg(feature = "scale-qualification")]
    pub fn open_gfx942_vecadd_scale_qualification_v1(
        device_unique_id: u64,
        host_table_budget_bytes: u64,
        max_host_table_reservations: usize,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if device_unique_id == 0 {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "device unique id must be nonzero",
            ));
        }
        let admitted = crate::qualification_gfx942_vecadd_v1::admit_gfx942_vecadd_qualification_v1(
        )
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            )
        })?;
        let dispatch = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(
                ResourceKindV1::ControlResidentBytes,
                host_table_budget_bytes,
            ),
            max_host_table_reservations,
        )
        .and_then(|account| {
            RuntimeDispatchStateV1::try_new(RuntimeDispatchCapacityV1::qualification_1024(account))
        })
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::Capacity, error.to_string())
        })?;
        let device = Self::open_checked_device_v1(device_unique_id)?;
        Ok(Self::new_with_dispatch_state_v1(
            Self::describe_device_v1(&device),
            Some(device),
            KfdRuntimeLaunchGateV1::ExactGfx942Vecadd(admitted),
            StagingBudgetsV1 {
                max_allocation_bytes: KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1,
                max_context_bytes: KFD_RUNTIME_MAX_STAGED_CONTEXT_BYTES_V1,
            },
            dispatch,
        ))
    }

    /// Reports only this opt-in profile's host-table ledger, not GPU or total memory.
    #[cfg(feature = "scale-qualification")]
    pub fn scale_qualification_host_table_usage_v1(
        &self,
    ) -> Option<fe2o3_resource_accounting::ResourceCreditUsageV1> {
        self.dispatch_capacity
            .account
            .as_ref()
            .map(ResourceCreditAccountV1::usage)
    }
}

#[cfg(all(test, feature = "scale-qualification"))]
pub(super) mod tests;
