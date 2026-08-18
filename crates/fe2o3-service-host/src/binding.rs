use fe2o3_host_api::{
    LoadOutcomeV1, LoadResultV1, LoadedObjectIdV1, ServiceInstanceIdV1,
    TaskSchemaIdV1 as HostTaskSchemaIdV1,
};
use fe2o3_service_model::{
    AllocationRoleV1, EvidenceStatusV1, IdentityDigestV1, LifecycleStateV1, PersistentPlanIdV1,
    PropertyClaimsV1, SchedulerModelIdV1, ServiceExecutableIdV1, ServiceModelConfigV1,
    ServicePropertyV1, ServiceRunIdV1, ServiceRunInputV1, ServiceStateV1, SlotIdV1, SlotKeyV1,
    TaskSchemaIdV1 as ModelTaskSchemaIdV1,
};

use crate::{BindingFieldV1, ServiceHostErrorV1};

/// Copyable exact identity and epoch key for one described service run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceKeyV1 {
    service_run_id: ServiceRunIdV1,
    service_instance_identity: ServiceInstanceIdV1,
    service_executable_id: ServiceExecutableIdV1,
    model_task_schema_id: ModelTaskSchemaIdV1,
    host_task_schema_id: HostTaskSchemaIdV1,
    scheduler_model_id: SchedulerModelIdV1,
    service_epoch: u64,
    queue_identity: IdentityDigestV1,
    queue_epoch: u64,
    queue_ordinal: u16,
    loaded_object_identity: LoadedObjectIdV1,
    load_generation: u64,
}

impl ServiceKeyV1 {
    /// Returns the canonical model run identity.
    pub const fn service_run_id(self) -> ServiceRunIdV1 {
        self.service_run_id
    }

    /// Returns the host service-instance commitment.
    pub const fn service_instance_identity(self) -> ServiceInstanceIdV1 {
        self.service_instance_identity
    }

    /// Returns the canonical service-executable commitment.
    pub const fn service_executable_id(self) -> ServiceExecutableIdV1 {
        self.service_executable_id
    }

    /// Returns the canonical model task-schema commitment.
    pub const fn model_task_schema_id(self) -> ModelTaskSchemaIdV1 {
        self.model_task_schema_id
    }

    /// Returns the corresponding host task-schema commitment.
    pub const fn host_task_schema_id(self) -> HostTaskSchemaIdV1 {
        self.host_task_schema_id
    }

    /// Returns the canonical scheduler-model commitment.
    pub const fn scheduler_model_id(self) -> SchedulerModelIdV1 {
        self.scheduler_model_id
    }

    /// Returns the nonzero service epoch.
    pub const fn service_epoch(self) -> u64 {
        self.service_epoch
    }

    /// Returns the canonical queue allocation commitment.
    pub const fn queue_identity(self) -> IdentityDigestV1 {
        self.queue_identity
    }

    /// Returns the nonzero queue allocation epoch.
    pub const fn queue_epoch(self) -> u64 {
        self.queue_epoch
    }

    /// Returns the queue allocation ordinal in the canonical run input.
    pub const fn queue_ordinal(self) -> u16 {
        self.queue_ordinal
    }

    /// Returns the exact host loaded-object commitment.
    pub const fn loaded_object_identity(self) -> LoadedObjectIdV1 {
        self.loaded_object_identity
    }

    /// Returns the exact nonzero host load generation.
    pub const fn load_generation(self) -> u64 {
        self.load_generation
    }
}

/// Exact cross-crate contract for one authority-free service description.
///
/// The caller-supplied commitments remain untrusted. Construction checks only
/// structural correspondence and does not establish load, execution, proof,
/// quiescence, progress, or release authority.
#[derive(Debug)]
pub struct ServiceContractV1<'contract> {
    model_config: &'contract ServiceModelConfigV1,
    run_input: &'contract ServiceRunInputV1,
    properties: &'contract PropertyClaimsV1,
    persistent_plan_id: PersistentPlanIdV1,
    key: ServiceKeyV1,
}

impl<'contract> ServiceContractV1<'contract> {
    /// Binds canonical model/run contracts to exact host load commitments.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model_config: &'contract ServiceModelConfigV1,
        run_input: &'contract ServiceRunInputV1,
        properties: &'contract PropertyClaimsV1,
        persistent_plan_id: PersistentPlanIdV1,
        service_instance_identity: ServiceInstanceIdV1,
        host_task_schema_id: HostTaskSchemaIdV1,
        load_result: &LoadResultV1,
    ) -> Result<Self, ServiceHostErrorV1> {
        model_config
            .validate()
            .map_err(|_| ServiceHostErrorV1::InvalidModelConfiguration)?;
        run_input
            .validate()
            .map_err(|_| ServiceHostErrorV1::InvalidRunContract)?;
        if model_config.service_epoch == 0 || run_input.service_epoch == 0 {
            return Err(ServiceHostErrorV1::ZeroEpoch {
                field: BindingFieldV1::ServiceEpoch,
            });
        }
        if model_config.service_epoch != run_input.service_epoch {
            return Err(mismatch(BindingFieldV1::ServiceEpoch));
        }
        if model_config.task_schema_id.digest().as_bytes()
            != host_task_schema_id.digest().as_bytes()
        {
            return Err(mismatch(BindingFieldV1::TaskSchema));
        }
        if run_input.runtime_context_id.as_bytes()
            != load_result.runtime_context_identity().digest().as_bytes()
        {
            return Err(mismatch(BindingFieldV1::RuntimeContext));
        }
        let LoadOutcomeV1::Loaded {
            loaded_object_identity,
            load_generation,
        } = load_result.outcome()
        else {
            return Err(ServiceHostErrorV1::LoadNotSuccessful);
        };
        let queue = run_input
            .allocations
            .iter()
            .find(|allocation| allocation.role == AllocationRoleV1::Queue)
            .ok_or(ServiceHostErrorV1::InvalidRunContract)?;
        if queue.allocation_epoch == 0 {
            return Err(ServiceHostErrorV1::ZeroEpoch {
                field: BindingFieldV1::QueueEpoch,
            });
        }
        if queue.allocation_identity != model_config.queue_identity {
            return Err(mismatch(BindingFieldV1::QueueIdentity));
        }
        Ok(Self {
            model_config,
            run_input,
            properties,
            persistent_plan_id,
            key: ServiceKeyV1 {
                service_run_id: model_config.run_id,
                service_instance_identity,
                service_executable_id: run_input.executable_id,
                model_task_schema_id: model_config.task_schema_id,
                host_task_schema_id,
                scheduler_model_id: model_config.scheduler_model_id,
                service_epoch: model_config.service_epoch,
                queue_identity: model_config.queue_identity,
                queue_epoch: queue.allocation_epoch,
                queue_ordinal: queue.ordinal,
                loaded_object_identity,
                load_generation,
            },
        })
    }

    /// Returns the canonical model configuration by borrow.
    pub const fn model_config(&self) -> &'contract ServiceModelConfigV1 {
        self.model_config
    }

    /// Returns the canonical service-run identity input by borrow.
    pub const fn run_input(&self) -> &'contract ServiceRunInputV1 {
        self.run_input
    }

    /// Returns the persistent-plan commitment.
    pub const fn persistent_plan_id(&self) -> PersistentPlanIdV1 {
        self.persistent_plan_id
    }

    /// Returns the exact service/run/queue key.
    pub const fn key(&self) -> ServiceKeyV1 {
        self.key
    }

    /// Returns exactly the cancellation-safety classification.
    pub fn cancellation_claim(&self) -> EvidenceStatusV1 {
        self.properties.get(ServicePropertyV1::CancellationSafe)
    }

    /// Returns exactly the quiescence-safety classification.
    pub fn quiescence_claim(&self) -> EvidenceStatusV1 {
        self.properties.get(ServicePropertyV1::QuiescenceSafe)
    }

    /// Returns exactly the service-progress classification.
    pub fn progress_claim(&self) -> EvidenceStatusV1 {
        self.properties.get(ServicePropertyV1::ServiceProgress)
    }

    /// Returns every independent canonical property classification by borrow.
    pub const fn properties(&self) -> &'contract PropertyClaimsV1 {
        self.properties
    }

    pub(crate) fn validate_model_state(
        &self,
        state: &ServiceStateV1,
        expected: LifecycleStateV1,
    ) -> Result<(), ServiceHostErrorV1> {
        if state.config != *self.model_config || state.lifecycle != expected {
            return Err(mismatch(BindingFieldV1::ModelState));
        }
        state
            .validate_global_invariants()
            .map_err(|_| ServiceHostErrorV1::InvalidModelState)
    }

    pub(crate) fn validate_quiescent_model_state(
        &self,
        state: &ServiceStateV1,
        expected: LifecycleStateV1,
    ) -> Result<(), ServiceHostErrorV1> {
        self.validate_model_state(state, expected)?;
        if !state.is_quiescent() {
            return Err(ServiceHostErrorV1::ModelNotQuiescent);
        }
        Ok(())
    }
}

/// Exact queue slot, queue epoch, and logical/encoded generation binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueSlotBindingV1 {
    slot: SlotKeyV1,
    queue_epoch: u64,
    encoded_generation: u64,
}

impl QueueSlotBindingV1 {
    /// Creates the exact slot binding derived from a service contract.
    pub fn for_slot(
        contract: &ServiceContractV1<'_>,
        slot_id: SlotIdV1,
        logical_generation: u64,
    ) -> Result<Self, ServiceHostErrorV1> {
        if slot_id.0 >= contract.model_config.queue_capacity {
            return Err(mismatch(BindingFieldV1::Slot));
        }
        Ok(Self {
            slot: contract.model_config.slot_key(slot_id, logical_generation),
            queue_epoch: contract.key.queue_epoch,
            encoded_generation: logical_generation % contract.model_config.generation_modulus,
        })
    }

    /// Wraps untrusted parts for structural validation and hostile testing.
    pub const fn from_untrusted_parts(
        slot: SlotKeyV1,
        queue_epoch: u64,
        encoded_generation: u64,
    ) -> Self {
        Self {
            slot,
            queue_epoch,
            encoded_generation,
        }
    }

    /// Returns the canonical slot key.
    pub const fn slot(self) -> SlotKeyV1 {
        self.slot
    }

    /// Returns the queue allocation epoch.
    pub const fn queue_epoch(self) -> u64 {
        self.queue_epoch
    }

    /// Returns the concrete encoded generation.
    pub const fn encoded_generation(self) -> u64 {
        self.encoded_generation
    }

    /// Checks exact service/run/queue/slot/epoch/generation correspondence.
    pub fn validate_for(self, contract: &ServiceContractV1<'_>) -> Result<(), ServiceHostErrorV1> {
        let key = contract.key;
        if self.slot.run_id != key.service_run_id {
            return Err(mismatch(BindingFieldV1::ServiceRun));
        }
        if self.slot.service_epoch != key.service_epoch {
            return Err(mismatch(BindingFieldV1::ServiceEpoch));
        }
        if self.slot.queue_identity != key.queue_identity {
            return Err(mismatch(BindingFieldV1::QueueIdentity));
        }
        if self.queue_epoch != key.queue_epoch {
            return Err(mismatch(BindingFieldV1::QueueEpoch));
        }
        if self.slot.slot_id.0 >= contract.model_config.queue_capacity {
            return Err(mismatch(BindingFieldV1::Slot));
        }
        if self.encoded_generation
            != self.slot.logical_generation % contract.model_config.generation_modulus
        {
            return Err(mismatch(BindingFieldV1::Generation));
        }
        Ok(())
    }
}

pub(crate) const fn mismatch(field: BindingFieldV1) -> ServiceHostErrorV1 {
    ServiceHostErrorV1::BindingMismatch { field }
}
