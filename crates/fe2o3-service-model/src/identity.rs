use alloc::vec::Vec;
use core::fmt;

pub const SERVICE_IDENTITY_SCHEMA_VERSION_V1: u16 = 1;
pub const IDENTITY_DIGEST_BYTES_V1: usize = 32;
pub const MAX_TASK_VARIANTS_V1: usize = 32;
pub const MAX_FUSION_NODES_V1: usize = 64;
pub const MAX_FUSION_EDGES_V1: usize = 128;
pub const MAX_FUSION_PHASES_V1: usize = 32;
pub const MAX_MATERIALIZED_VALUES_V1: usize = 64;
pub const MAX_WORKER_ROLES_V1: usize = 16;
pub const MAX_SERVICE_WORKERS_V1: u16 = 4096;
pub const MAX_RUN_ALLOCATIONS_V1: usize = 128;
pub const MAX_QUEUE_CAPACITY_V1: u16 = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityInputErrorV1 {
    EmptyCollection(&'static str),
    TooManyItems {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    DuplicateTaskTag(u32),
    NonCanonicalTaskTagOrder,
    InvalidQueueCapacity(u16),
    InvalidGenerationDomain,
    DuplicateFusionNode,
    InvalidFusionEdge,
    DuplicateFusionEdge,
    NonCanonicalFusionEdgeOrder,
    InvalidPhasePartition,
    DuplicateMaterializedValue,
    InvalidWorkerRange,
    DuplicateWorkerRole,
    NonCanonicalWorkerRoleOrder,
    InvalidResidentWorkerRequirement,
    DuplicateHandlerTag(u32),
    NonCanonicalHandlerOrder,
    MissingHandlerTag(u32),
    HandlerIdentityMismatch(u32),
    DuplicateAllocation,
    NonCanonicalAllocationOrder,
}

impl fmt::Display for IdentityInputErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid service identity input: {self:?}")
    }
}

/// Opaque output of the #134-selected canonical digest operation.
///
/// Construction does not authenticate the bytes or grant any authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct IdentityDigestV1([u8; IDENTITY_DIGEST_BYTES_V1]);

impl IdentityDigestV1 {
    pub const fn from_untrusted_bytes(bytes: [u8; IDENTITY_DIGEST_BYTES_V1]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; IDENTITY_DIGEST_BYTES_V1] {
        &self.0
    }
}

macro_rules! typed_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(IdentityDigestV1);

        impl $name {
            /// Wraps a caller-supplied canonical commitment without authenticating it.
            pub const fn from_untrusted_digest(digest: IdentityDigestV1) -> Self {
                Self(digest)
            }

            pub const fn digest(self) -> IdentityDigestV1 {
                self.0
            }
        }
    };
}

typed_identity!(
    /// Closed task-family identity.
    TaskSchemaIdV1
);
typed_identity!(
    /// Abstract scheduler semantics identity.
    SchedulerModelIdV1
);
typed_identity!(
    /// Finite graph-fusion identity.
    FusionPlanIdV1
);
typed_identity!(
    /// Persistent service-plan identity.
    PersistentPlanIdV1
);
typed_identity!(
    /// Target artifact-bound service identity, reserved for P4.
    ServiceExecutableIdV1
);
typed_identity!(
    /// One runtime service-instance identity, reserved for P4.
    ServiceRunIdV1
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UnknownTaskTagPolicyV1 {
    Reject = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskVariantInputV1 {
    pub canonical_tag: u32,
    pub variant_name_identity: IdentityDigestV1,
    pub payload_abi_and_layout_identity: IdentityDigestV1,
    pub payload_lifetime_and_region_contract_id: IdentityDigestV1,
    pub handler_algorithm_id: IdentityDigestV1,
    pub handler_numerical_contract_id: IdentityDigestV1,
    pub handler_contract_id: IdentityDigestV1,
    pub handler_effect_and_capability_closure_id: IdentityDigestV1,
    pub cancellation_contract_id: IdentityDigestV1,
    pub unsafe_or_external_obligations_id: IdentityDigestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSchemaInputV1 {
    variants: Vec<TaskVariantInputV1>,
    schema_failure_policy_id: IdentityDigestV1,
}

impl TaskSchemaInputV1 {
    pub fn new(
        variants: Vec<TaskVariantInputV1>,
        schema_failure_policy_id: IdentityDigestV1,
    ) -> Result<Self, IdentityInputErrorV1> {
        require_nonempty_bounded("task variants", variants.len(), MAX_TASK_VARIANTS_V1)?;
        for pair in variants.windows(2) {
            if pair[0].canonical_tag == pair[1].canonical_tag {
                return Err(IdentityInputErrorV1::DuplicateTaskTag(
                    pair[0].canonical_tag,
                ));
            }
            if pair[0].canonical_tag > pair[1].canonical_tag {
                return Err(IdentityInputErrorV1::NonCanonicalTaskTagOrder);
            }
        }
        Ok(Self {
            variants,
            schema_failure_policy_id,
        })
    }

    pub fn variants(&self) -> &[TaskVariantInputV1] {
        &self.variants
    }

    pub const fn schema_failure_policy_id(&self) -> IdentityDigestV1 {
        self.schema_failure_policy_id
    }

    pub const fn unknown_tag_policy(&self) -> UnknownTaskTagPolicyV1 {
        UnknownTaskTagPolicyV1::Reject
    }

    /// Returns the bounded canonical identity preimage; it is not a digest.
    pub fn encode_canonical_preimage(&self) -> Vec<u8> {
        let mut encoder = Encoder::new(*b"F2TASKS1");
        encoder.sequence(1, &self.variants, |entry, variant| {
            entry.u32(1, variant.canonical_tag);
            entry.digest(2, variant.variant_name_identity);
            entry.digest(3, variant.payload_abi_and_layout_identity);
            entry.digest(4, variant.payload_lifetime_and_region_contract_id);
            entry.digest(5, variant.handler_algorithm_id);
            entry.digest(6, variant.handler_numerical_contract_id);
            entry.digest(7, variant.handler_contract_id);
            entry.digest(8, variant.handler_effect_and_capability_closure_id);
            entry.digest(9, variant.cancellation_contract_id);
            entry.digest(10, variant.unsafe_or_external_obligations_id);
        });
        encoder.u8(2, self.unknown_tag_policy() as u8);
        encoder.digest(3, self.schema_failure_policy_id);
        encoder.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DeliveryPolicyV1 {
    AtMostOnce = 1,
    ExactlyOnce = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueDisciplineV1 {
    Fifo,
    FifoBatch { maximum_batch: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LifecyclePolicyV1 {
    DrainThenStop = 1,
    ClassifiedDirectStop = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerModelInputV1 {
    pub queue_model_id: IdentityDigestV1,
    pub queue_capacity: u16,
    pub generation_modulus: u64,
    pub maximum_live_generation_span: u64,
    pub queue_discipline: QueueDisciplineV1,
    pub delivery_policy: DeliveryPolicyV1,
    pub dependency_epoch_model_id: IdentityDigestV1,
    pub lifecycle_policy: LifecyclePolicyV1,
    pub cancellation_policy_id: IdentityDigestV1,
    pub failure_model_id: IdentityDigestV1,
    pub synchronization_contract_id: IdentityDigestV1,
    pub progress_contract_id: Option<IdentityDigestV1>,
}

impl SchedulerModelInputV1 {
    pub fn validate(&self) -> Result<(), IdentityInputErrorV1> {
        if self.queue_capacity == 0 || self.queue_capacity > MAX_QUEUE_CAPACITY_V1 {
            return Err(IdentityInputErrorV1::InvalidQueueCapacity(
                self.queue_capacity,
            ));
        }
        if self.generation_modulus < 2
            || self.maximum_live_generation_span >= self.generation_modulus
        {
            return Err(IdentityInputErrorV1::InvalidGenerationDomain);
        }
        if let QueueDisciplineV1::FifoBatch { maximum_batch } = self.queue_discipline
            && (maximum_batch == 0 || maximum_batch > self.queue_capacity)
        {
            return Err(IdentityInputErrorV1::InvalidQueueCapacity(maximum_batch));
        }
        Ok(())
    }

    pub fn encode_canonical_preimage(&self) -> Result<Vec<u8>, IdentityInputErrorV1> {
        self.validate()?;
        let mut encoder = Encoder::new(*b"F2SCHED1");
        encoder.digest(1, self.queue_model_id);
        encoder.u16(2, self.queue_capacity);
        encoder.u64(3, self.generation_modulus);
        encoder.u64(4, self.maximum_live_generation_span);
        match self.queue_discipline {
            QueueDisciplineV1::Fifo => encoder.u8(5, 1),
            QueueDisciplineV1::FifoBatch { maximum_batch } => {
                encoder.u8(5, 2);
                encoder.u16(6, maximum_batch);
            }
        }
        encoder.u8(7, self.delivery_policy as u8);
        encoder.digest(8, self.dependency_epoch_model_id);
        encoder.u8(9, self.lifecycle_policy as u8);
        encoder.digest(10, self.cancellation_policy_id);
        encoder.digest(11, self.failure_model_id);
        encoder.digest(12, self.synchronization_contract_id);
        encoder.optional_digest(13, self.progress_contract_id);
        Ok(encoder.finish())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FusionEdgeInputV1 {
    pub source_node_index: u16,
    pub target_node_index: u16,
    pub edge_identity: IdentityDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FusionPhaseInputV1 {
    pub phase_identity: IdentityDigestV1,
    pub first_node_index: u16,
    pub node_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FusionPlanInputV1 {
    pub authoritative_dispatch_graph_id: IdentityDigestV1,
    pub nodes_in_graph_order: Vec<IdentityDigestV1>,
    pub edges: Vec<FusionEdgeInputV1>,
    pub phases: Vec<FusionPhaseInputV1>,
    pub materialized_values: Vec<IdentityDigestV1>,
    pub effect_dependency_origin_map_id: IdentityDigestV1,
    pub layout_and_region_choices_id: IdentityDigestV1,
    pub barrier_and_convergence_contract_id: IdentityDigestV1,
    pub numerical_order_contract_id: IdentityDigestV1,
    pub schedule_parameters_id: IdentityDigestV1,
    pub legality_rule_set_id: IdentityDigestV1,
    pub transformation_receipt_schema_id: IdentityDigestV1,
}

impl FusionPlanInputV1 {
    pub fn validate(&self) -> Result<(), IdentityInputErrorV1> {
        require_nonempty_bounded(
            "fusion nodes",
            self.nodes_in_graph_order.len(),
            MAX_FUSION_NODES_V1,
        )?;
        require_nonempty_bounded("fusion phases", self.phases.len(), MAX_FUSION_PHASES_V1)?;
        require_bounded("fusion edges", self.edges.len(), MAX_FUSION_EDGES_V1)?;
        require_bounded(
            "materialized values",
            self.materialized_values.len(),
            MAX_MATERIALIZED_VALUES_V1,
        )?;
        if has_duplicate(&self.nodes_in_graph_order) {
            return Err(IdentityInputErrorV1::DuplicateFusionNode);
        }
        if has_duplicate(&self.materialized_values) {
            return Err(IdentityInputErrorV1::DuplicateMaterializedValue);
        }
        for edge in &self.edges {
            if usize::from(edge.source_node_index) >= self.nodes_in_graph_order.len()
                || usize::from(edge.target_node_index) >= self.nodes_in_graph_order.len()
                || edge.source_node_index == edge.target_node_index
            {
                return Err(IdentityInputErrorV1::InvalidFusionEdge);
            }
        }
        for pair in self.edges.windows(2) {
            let left = (
                pair[0].source_node_index,
                pair[0].target_node_index,
                pair[0].edge_identity,
            );
            let right = (
                pair[1].source_node_index,
                pair[1].target_node_index,
                pair[1].edge_identity,
            );
            if left == right {
                return Err(IdentityInputErrorV1::DuplicateFusionEdge);
            }
            if left > right {
                return Err(IdentityInputErrorV1::NonCanonicalFusionEdgeOrder);
            }
        }
        let mut expected_first = 0usize;
        for phase in &self.phases {
            if phase.node_count == 0
                || usize::from(phase.first_node_index) != expected_first
                || expected_first + usize::from(phase.node_count) > self.nodes_in_graph_order.len()
            {
                return Err(IdentityInputErrorV1::InvalidPhasePartition);
            }
            expected_first += usize::from(phase.node_count);
        }
        if expected_first != self.nodes_in_graph_order.len() {
            return Err(IdentityInputErrorV1::InvalidPhasePartition);
        }
        Ok(())
    }

    pub fn encode_canonical_preimage(&self) -> Result<Vec<u8>, IdentityInputErrorV1> {
        self.validate()?;
        let mut encoder = Encoder::new(*b"F2FUSEP1");
        encoder.digest(1, self.authoritative_dispatch_graph_id);
        encoder.digest_sequence(2, &self.nodes_in_graph_order);
        encoder.sequence(3, &self.edges, |entry, edge| {
            entry.u16(1, edge.source_node_index);
            entry.u16(2, edge.target_node_index);
            entry.digest(3, edge.edge_identity);
        });
        encoder.sequence(4, &self.phases, |entry, phase| {
            entry.digest(1, phase.phase_identity);
            entry.u16(2, phase.first_node_index);
            entry.u16(3, phase.node_count);
        });
        encoder.digest_sequence(5, &self.materialized_values);
        encoder.digest(6, self.effect_dependency_origin_map_id);
        encoder.digest(7, self.layout_and_region_choices_id);
        encoder.digest(8, self.barrier_and_convergence_contract_id);
        encoder.digest(9, self.numerical_order_contract_id);
        encoder.digest(10, self.schedule_parameters_id);
        encoder.digest(11, self.legality_rule_set_id);
        encoder.digest(12, self.transformation_receipt_schema_id);
        Ok(encoder.finish())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerRoleInputV1 {
    pub role_identity: IdentityDigestV1,
    pub minimum_workers: u16,
    pub maximum_workers: u16,
    pub state_partition_id: IdentityDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlerPlanReferenceV1 {
    pub task_tag: u32,
    pub handler_algorithm_id: IdentityDigestV1,
    pub fusion_plan_id: Option<FusionPlanIdV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentPlanInputV1 {
    pub task_schema_id: TaskSchemaIdV1,
    pub scheduler_model_id: SchedulerModelIdV1,
    pub worker_roles: Vec<WorkerRoleInputV1>,
    pub resident_workgroups: u16,
    pub resident_waves: u16,
    pub queue_and_state_resource_plan_id: IdentityDigestV1,
    pub launch_and_cooperation_contract_id: IdentityDigestV1,
    pub handler_plan_references: Vec<HandlerPlanReferenceV1>,
    pub drain_stop_failure_policy_id: IdentityDigestV1,
    pub resource_contract_id: IdentityDigestV1,
}

impl PersistentPlanInputV1 {
    pub fn validate(&self) -> Result<(), IdentityInputErrorV1> {
        require_nonempty_bounded("worker roles", self.worker_roles.len(), MAX_WORKER_ROLES_V1)?;
        require_nonempty_bounded(
            "handler plan references",
            self.handler_plan_references.len(),
            MAX_TASK_VARIANTS_V1,
        )?;
        let mut maximum_workers = 0u32;
        for role in &self.worker_roles {
            if role.minimum_workers == 0 || role.minimum_workers > role.maximum_workers {
                return Err(IdentityInputErrorV1::InvalidWorkerRange);
            }
            maximum_workers += u32::from(role.maximum_workers);
        }
        for pair in self.worker_roles.windows(2) {
            if pair[0].role_identity == pair[1].role_identity {
                return Err(IdentityInputErrorV1::DuplicateWorkerRole);
            }
            if pair[0].role_identity > pair[1].role_identity {
                return Err(IdentityInputErrorV1::NonCanonicalWorkerRoleOrder);
            }
        }
        if maximum_workers > u32::from(MAX_SERVICE_WORKERS_V1)
            || self.resident_workgroups == 0
            || self.resident_waves == 0
            || self.resident_workgroups > MAX_SERVICE_WORKERS_V1
        {
            return Err(IdentityInputErrorV1::InvalidResidentWorkerRequirement);
        }
        for pair in self.handler_plan_references.windows(2) {
            if pair[0].task_tag == pair[1].task_tag {
                return Err(IdentityInputErrorV1::DuplicateHandlerTag(pair[0].task_tag));
            }
            if pair[0].task_tag > pair[1].task_tag {
                return Err(IdentityInputErrorV1::NonCanonicalHandlerOrder);
            }
        }
        Ok(())
    }

    pub fn validate_against_schema(
        &self,
        schema: &TaskSchemaInputV1,
    ) -> Result<(), IdentityInputErrorV1> {
        self.validate()?;
        if self.handler_plan_references.len() != schema.variants().len() {
            return Err(IdentityInputErrorV1::MissingHandlerTag(u32::MAX));
        }
        for variant in schema.variants() {
            let Some(handler) = self
                .handler_plan_references
                .iter()
                .find(|handler| handler.task_tag == variant.canonical_tag)
            else {
                return Err(IdentityInputErrorV1::MissingHandlerTag(
                    variant.canonical_tag,
                ));
            };
            if handler.handler_algorithm_id != variant.handler_algorithm_id {
                return Err(IdentityInputErrorV1::HandlerIdentityMismatch(
                    variant.canonical_tag,
                ));
            }
        }
        Ok(())
    }

    pub fn encode_canonical_preimage(&self) -> Result<Vec<u8>, IdentityInputErrorV1> {
        self.validate()?;
        let mut encoder = Encoder::new(*b"F2PERST1");
        encoder.digest(1, self.task_schema_id.digest());
        encoder.digest(2, self.scheduler_model_id.digest());
        encoder.sequence(3, &self.worker_roles, |entry, role| {
            entry.digest(1, role.role_identity);
            entry.u16(2, role.minimum_workers);
            entry.u16(3, role.maximum_workers);
            entry.digest(4, role.state_partition_id);
        });
        encoder.u16(4, self.resident_workgroups);
        encoder.u16(5, self.resident_waves);
        encoder.digest(6, self.queue_and_state_resource_plan_id);
        encoder.digest(7, self.launch_and_cooperation_contract_id);
        encoder.sequence(8, &self.handler_plan_references, |entry, handler| {
            entry.u32(1, handler.task_tag);
            entry.digest(2, handler.handler_algorithm_id);
            entry.optional_digest(3, handler.fusion_plan_id.map(FusionPlanIdV1::digest));
        });
        encoder.digest(9, self.drain_stop_failure_policy_id);
        encoder.digest(10, self.resource_contract_id);
        Ok(encoder.finish())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceExecutableInputV1 {
    pub persistent_plan_id: PersistentPlanIdV1,
    pub target_plan_id: IdentityDigestV1,
    pub launch_contract_id: IdentityDigestV1,
    pub compiler_and_toolchain_id: IdentityDigestV1,
    pub llvm_module_id: IdentityDigestV1,
    pub object_id: IdentityDigestV1,
    pub hsaco_id: IdentityDigestV1,
    pub resource_and_origin_map_id: IdentityDigestV1,
}

impl ServiceExecutableInputV1 {
    /// Encodes reserved P4 identity inputs. It does not establish an artifact.
    pub fn encode_canonical_preimage(&self) -> Vec<u8> {
        let mut encoder = Encoder::new(*b"F2SVEXE1");
        encoder.digest(1, self.persistent_plan_id.digest());
        encoder.digest(2, self.target_plan_id);
        encoder.digest(3, self.launch_contract_id);
        encoder.digest(4, self.compiler_and_toolchain_id);
        encoder.digest(5, self.llvm_module_id);
        encoder.digest(6, self.object_id);
        encoder.digest(7, self.hsaco_id);
        encoder.digest(8, self.resource_and_origin_map_id);
        encoder.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AllocationRoleV1 {
    Queue = 1,
    State = 2,
    Input = 3,
    Output = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationBindingInputV1 {
    pub role: AllocationRoleV1,
    pub ordinal: u16,
    pub allocation_identity: IdentityDigestV1,
    pub allocation_epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceRunInputV1 {
    pub executable_id: ServiceExecutableIdV1,
    pub physical_device_id: IdentityDigestV1,
    pub runtime_context_id: IdentityDigestV1,
    pub stream_or_queue_id: IdentityDigestV1,
    pub service_epoch: u64,
    pub allocations: Vec<AllocationBindingInputV1>,
    pub launch_instance_id: IdentityDigestV1,
}

impl ServiceRunInputV1 {
    pub fn validate(&self) -> Result<(), IdentityInputErrorV1> {
        require_nonempty_bounded(
            "run allocations",
            self.allocations.len(),
            MAX_RUN_ALLOCATIONS_V1,
        )?;
        for pair in self.allocations.windows(2) {
            let left = (pair[0].role, pair[0].ordinal);
            let right = (pair[1].role, pair[1].ordinal);
            if left == right {
                return Err(IdentityInputErrorV1::DuplicateAllocation);
            }
            if left > right {
                return Err(IdentityInputErrorV1::NonCanonicalAllocationOrder);
            }
        }
        let queue_count = self
            .allocations
            .iter()
            .filter(|binding| binding.role == AllocationRoleV1::Queue)
            .count();
        let state_count = self
            .allocations
            .iter()
            .filter(|binding| binding.role == AllocationRoleV1::State)
            .count();
        if queue_count != 1 || state_count != 1 {
            return Err(IdentityInputErrorV1::DuplicateAllocation);
        }
        Ok(())
    }

    pub fn encode_canonical_preimage(&self) -> Result<Vec<u8>, IdentityInputErrorV1> {
        self.validate()?;
        let mut encoder = Encoder::new(*b"F2SVRUN1");
        encoder.digest(1, self.executable_id.digest());
        encoder.digest(2, self.physical_device_id);
        encoder.digest(3, self.runtime_context_id);
        encoder.digest(4, self.stream_or_queue_id);
        encoder.u64(5, self.service_epoch);
        encoder.sequence(6, &self.allocations, |entry, binding| {
            entry.u8(1, binding.role as u8);
            entry.u16(2, binding.ordinal);
            entry.digest(3, binding.allocation_identity);
            entry.u64(4, binding.allocation_epoch);
        });
        encoder.digest(7, self.launch_instance_id);
        Ok(encoder.finish())
    }
}

fn require_nonempty_bounded(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), IdentityInputErrorV1> {
    if actual == 0 {
        return Err(IdentityInputErrorV1::EmptyCollection(field));
    }
    require_bounded(field, actual, maximum)
}

fn require_bounded(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), IdentityInputErrorV1> {
    if actual > maximum {
        return Err(IdentityInputErrorV1::TooManyItems {
            field,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn has_duplicate<T: Eq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new(domain: [u8; 8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&domain);
        bytes.extend_from_slice(&SERVICE_IDENTITY_SCHEMA_VERSION_V1.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        Self { bytes }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn field(&mut self, tag: u16, value: &[u8]) {
        self.bytes.extend_from_slice(&tag.to_be_bytes());
        self.bytes
            .extend_from_slice(&(value.len() as u32).to_be_bytes());
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, tag: u16, value: u8) {
        self.field(tag, &[value]);
    }

    fn u16(&mut self, tag: u16, value: u16) {
        self.field(tag, &value.to_be_bytes());
    }

    fn u32(&mut self, tag: u16, value: u32) {
        self.field(tag, &value.to_be_bytes());
    }

    fn u64(&mut self, tag: u16, value: u64) {
        self.field(tag, &value.to_be_bytes());
    }

    fn digest(&mut self, tag: u16, value: IdentityDigestV1) {
        self.field(tag, value.as_bytes());
    }

    fn optional_digest(&mut self, tag: u16, value: Option<IdentityDigestV1>) {
        match value {
            None => self.field(tag, &[0]),
            Some(value) => {
                let mut encoded = [0u8; IDENTITY_DIGEST_BYTES_V1 + 1];
                encoded[0] = 1;
                encoded[1..].copy_from_slice(value.as_bytes());
                self.field(tag, &encoded);
            }
        }
    }

    fn digest_sequence(&mut self, tag: u16, values: &[IdentityDigestV1]) {
        self.sequence(tag, values, |entry, value| entry.digest(1, *value));
    }

    fn sequence<T>(&mut self, tag: u16, values: &[T], encode: impl Fn(&mut Encoder, &T)) {
        let mut sequence = Encoder { bytes: Vec::new() };
        sequence
            .bytes
            .extend_from_slice(&(values.len() as u16).to_be_bytes());
        for value in values {
            let mut entry = Encoder { bytes: Vec::new() };
            encode(&mut entry, value);
            sequence
                .bytes
                .extend_from_slice(&(entry.bytes.len() as u32).to_be_bytes());
            sequence.bytes.extend_from_slice(&entry.bytes);
        }
        self.field(tag, &sequence.bytes);
    }
}
