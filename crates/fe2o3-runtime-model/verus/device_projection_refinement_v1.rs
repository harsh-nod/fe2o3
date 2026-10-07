use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct InventoryIdentityV1 {
    pub topology_node: nat,
    pub kfd_gpu_id: nat,
    pub gpu_unique: nat,
    pub render_minor: nat,
    pub pci: nat,
    pub vendor: nat,
    pub device: nat,
    pub pci_revision: nat,
    pub target: nat,
}

#[derive(PartialEq, Eq)]
pub struct DigestV1 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
}

#[derive(PartialEq, Eq)]
pub enum DeviceAdmissionTargetProfileV1 {
    Gfx942XnackMinusSpxNps1Kfd1_18Drm3_64_0,
    Gfx950XnackMinusSpxNps1Kfd1_18Drm3_64_0,
}

#[derive(PartialEq, Eq)]
pub enum KernelReleaseObservationV1 {
    Linux6_8_0_124Generic,
    Other,
}

#[derive(PartialEq, Eq)]
pub enum AmdgpuModuleObservationV1 {
    Version6_16_13SourceA6f143bec60c0afc3263226,
    Version6_16_13Source703b1127e578bc5d4bd6615,
    Other,
}

// These commitments are caller-supplied model inputs, not authenticated provenance.
pub struct DeviceAdmissionProfileV1 {
    pub target_profile: DeviceAdmissionTargetProfileV1,
    pub identity: DigestV1,
    pub kfd_schema: DigestV1,
    pub drm_schema: DigestV1,
}

pub struct TargetProjectionTupleV1 {
    pub target: nat,
    pub pci_device: nat,
    pub firmware: nat,
    pub sdma_firmware: nat,
    pub simd_count: nat,
    pub chip_revision: nat,
    pub external_revision: nat,
    pub module: AmdgpuModuleObservationV1,
}

pub open spec fn profile_tuple_v1(profile: DeviceAdmissionTargetProfileV1)
    -> TargetProjectionTupleV1
{
    match profile {
        DeviceAdmissionTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18Drm3_64_0 =>
            TargetProjectionTupleV1 {
                target: 942, pci_device: 0x74a1, firmware: 192, sdma_firmware: 25,
                simd_count: 1216, chip_revision: 1, external_revision: 71,
                module: AmdgpuModuleObservationV1::Version6_16_13SourceA6f143bec60c0afc3263226,
            },
        DeviceAdmissionTargetProfileV1::Gfx950XnackMinusSpxNps1Kfd1_18Drm3_64_0 =>
            TargetProjectionTupleV1 {
                target: 950, pci_device: 0x75a0, firmware: 41, sdma_firmware: 12,
                simd_count: 1024, chip_revision: 0, external_revision: 80,
                module: AmdgpuModuleObservationV1::Version6_16_13Source703b1127e578bc5d4bd6615,
            },
    }
}

pub open spec fn v1_profile_identity() -> DigestV1 {
    DigestV1 {
        word0: 0xe12ea33b259666e7,
        word1: 0x928612403109640b,
        word2: 0x03b0d637b893a2c1,
        word3: 0x5b87d17a4211c8de,
    }
}

pub open spec fn v1_kfd_schema_identity() -> DigestV1 {
    DigestV1 {
        word0: 0xe4aad5d8e3177ea6,
        word1: 0xd70298adab7741c3,
        word2: 0x77cb091373553ce6,
        word3: 0x89f3525e7514d9b4,
    }
}

pub open spec fn v1_drm_schema_identity() -> DigestV1 {
    DigestV1 {
        word0: 0x800569fe9b467b38,
        word1: 0x9bcfc6e5d65b23d6,
        word2: 0x6a0386a90fc2a669,
        word3: 0xfac8c83800e76d8b,
    }
}

pub open spec fn gfx942_profile_v1() -> DeviceAdmissionProfileV1 {
    DeviceAdmissionProfileV1 {
        target_profile: DeviceAdmissionTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18Drm3_64_0,
        identity: v1_profile_identity(),
        kfd_schema: v1_kfd_schema_identity(),
        drm_schema: v1_drm_schema_identity(),
    }
}

pub struct CanonicalObservationV1 {
    pub schema_version: nat,
    pub domain: nat,
    pub profile: DigestV1,
    pub kfd_schema: DigestV1,
    pub drm_schema: DigestV1,
    pub source_commitment: nat,
    pub module_file_system_device: nat,
    pub module_inode: nat,
    pub kernel_release: KernelReleaseObservationV1,
    pub amdgpu_module: AmdgpuModuleObservationV1,
    pub kfd_descriptor_commitment: nat,
    pub render_descriptor_commitment: nat,
    pub aperture_inventory_commitment: nat,
    pub inventory: Seq<InventoryIdentityV1>,
    pub topology_generation: nat,
    pub topology_node: nat,
    pub physical: nat,
    pub topology_gpu_unique: nat,
    pub render_gpu_unique: nat,
    pub pci: nat,
    pub render_pci: nat,
    pub kfd_gpu_id: nat,
    pub topology_render_minor: nat,
    pub render_minor: nat,
    pub vendor: nat,
    pub device: nat,
    pub pci_revision: nat,
    pub target: nat,
    pub compute_partition: nat,
    pub memory_partition: nat,
    pub kfd_major: nat,
    pub kfd_minor: nat,
    pub kfd_uapi_major: nat,
    pub kfd_uapi_minor: nat,
    pub xnack: nat,
    pub drm_major: nat,
    pub drm_minor: nat,
    pub drm_patch: nat,
    pub acceleration_working: bool,
    pub drm_family: nat,
    pub drm_chip_revision: nat,
    pub drm_external_revision: nat,
    pub firmware: nat,
    pub sdma_firmware: nat,
    pub wavefront_size: nat,
    pub simd_count: nat,
    pub xcc_count: nat,
    pub initial_vram_lost_counter: nat,
    pub commit_fence_complete: bool,
    pub reset_subscription_established: bool,
    pub reset_event_mask_enabled: bool,
    pub reset_event_descriptor_cloexec: bool,
    pub reset_fence_initially_clear: bool,
    pub drm_reobserved_after_subscription_equal: bool,
    pub reset_fence_clear_before_commit: bool,
}

pub open spec fn canonical_equal_v1(
    left: CanonicalObservationV1,
    right: CanonicalObservationV1,
) -> bool {
    &&& left.schema_version == right.schema_version
    &&& left.domain == right.domain
    &&& left.profile == right.profile
    &&& left.kfd_schema == right.kfd_schema
    &&& left.drm_schema == right.drm_schema
    &&& left.source_commitment == right.source_commitment
    &&& left.module_file_system_device == right.module_file_system_device
    &&& left.module_inode == right.module_inode
    &&& left.kernel_release == right.kernel_release
    &&& left.amdgpu_module == right.amdgpu_module
    &&& left.kfd_descriptor_commitment == right.kfd_descriptor_commitment
    &&& left.render_descriptor_commitment == right.render_descriptor_commitment
    &&& left.aperture_inventory_commitment == right.aperture_inventory_commitment
    &&& left.inventory =~= right.inventory
    &&& left.topology_generation == right.topology_generation
    &&& left.topology_node == right.topology_node
    &&& left.physical == right.physical
    &&& left.topology_gpu_unique == right.topology_gpu_unique
    &&& left.render_gpu_unique == right.render_gpu_unique
    &&& left.pci == right.pci
    &&& left.render_pci == right.render_pci
    &&& left.kfd_gpu_id == right.kfd_gpu_id
    &&& left.topology_render_minor == right.topology_render_minor
    &&& left.render_minor == right.render_minor
    &&& left.vendor == right.vendor
    &&& left.device == right.device
    &&& left.pci_revision == right.pci_revision
    &&& left.target == right.target
    &&& left.compute_partition == right.compute_partition
    &&& left.memory_partition == right.memory_partition
    &&& left.kfd_major == right.kfd_major
    &&& left.kfd_minor == right.kfd_minor
    &&& left.kfd_uapi_major == right.kfd_uapi_major
    &&& left.kfd_uapi_minor == right.kfd_uapi_minor
    &&& left.xnack == right.xnack
    &&& left.drm_major == right.drm_major
    &&& left.drm_minor == right.drm_minor
    &&& left.drm_patch == right.drm_patch
    &&& left.acceleration_working == right.acceleration_working
    &&& left.drm_family == right.drm_family
    &&& left.drm_chip_revision == right.drm_chip_revision
    &&& left.drm_external_revision == right.drm_external_revision
    &&& left.firmware == right.firmware
    &&& left.sdma_firmware == right.sdma_firmware
    &&& left.wavefront_size == right.wavefront_size
    &&& left.simd_count == right.simd_count
    &&& left.xcc_count == right.xcc_count
    &&& left.initial_vram_lost_counter == right.initial_vram_lost_counter
    &&& left.commit_fence_complete == right.commit_fence_complete
    &&& left.reset_subscription_established == right.reset_subscription_established
    &&& left.reset_event_mask_enabled == right.reset_event_mask_enabled
    &&& left.reset_event_descriptor_cloexec == right.reset_event_descriptor_cloexec
    &&& left.reset_fence_initially_clear == right.reset_fence_initially_clear
    &&& left.drm_reobserved_after_subscription_equal
        == right.drm_reobserved_after_subscription_equal
    &&& left.reset_fence_clear_before_commit == right.reset_fence_clear_before_commit
}

pub open spec fn inventory_globally_unique_v1(
    observation: CanonicalObservationV1,
) -> bool {
    forall |left: int, right: int|
        0 <= left < observation.inventory.len()
        && 0 <= right < observation.inventory.len()
        && left != right
        ==> {
            let left_identity = #[trigger] observation.inventory[left];
            let right_identity = #[trigger] observation.inventory[right];
            &&& left_identity.topology_node != right_identity.topology_node
            &&& left_identity.gpu_unique != right_identity.gpu_unique
            &&& left_identity.kfd_gpu_id != right_identity.kfd_gpu_id
            &&& left_identity.render_minor != right_identity.render_minor
            &&& left_identity.pci != right_identity.pci
        }
}

pub open spec fn selected_inventory_identity_matches_v1(
    observation: CanonicalObservationV1,
    selected: int,
) -> bool {
    &&& 0 <= selected < observation.inventory.len()
    &&& observation.inventory[selected].topology_node == observation.topology_node
    &&& observation.inventory[selected].gpu_unique == observation.physical
    &&& observation.inventory[selected].kfd_gpu_id == observation.kfd_gpu_id
    &&& observation.inventory[selected].render_minor == observation.render_minor
    &&& observation.inventory[selected].pci == observation.pci
    &&& observation.inventory[selected].vendor == observation.vendor
    &&& observation.inventory[selected].device == observation.device
    &&& observation.inventory[selected].pci_revision == observation.pci_revision
    &&& observation.inventory[selected].target == observation.target
}

pub open spec fn exactly_one_selected_inventory_v1(
    observation: CanonicalObservationV1,
) -> bool {
    exists |selected: int| selected_inventory_identity_matches_v1(observation, selected)
        && forall |other: int| 0 <= other < observation.inventory.len()
            && #[trigger] observation.inventory[other].gpu_unique == observation.physical
            ==> other == selected
}

pub struct ModelProjectionV1 {
    pub canonical: CanonicalObservationV1,
    pub target_profile: DeviceAdmissionTargetProfileV1,
    pub domain: nat,
    pub profile: DigestV1,
    pub physical: nat,
    pub pci: nat,
    pub kfd_gpu_id: nat,
    pub render_minor: nat,
    pub kfd_schema: DigestV1,
    pub drm_schema: DigestV1,
}

pub open spec fn project_v1(observation: CanonicalObservationV1) -> ModelProjectionV1 {
    project_for_profile_v1(observation, gfx942_profile_v1())
}

pub open spec fn project_for_profile_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
) -> ModelProjectionV1 {
    ModelProjectionV1 {
        canonical: observation,
        target_profile: profile.target_profile,
        domain: observation.domain,
        profile: observation.profile,
        physical: observation.physical,
        pci: observation.pci,
        kfd_gpu_id: observation.kfd_gpu_id,
        render_minor: observation.render_minor,
        kfd_schema: observation.kfd_schema,
        drm_schema: observation.drm_schema,
    }
}

pub open spec fn projection_refines_v1(
    observation: CanonicalObservationV1,
    projection: ModelProjectionV1,
) -> bool {
    projection_refines_for_profile_v1(observation, gfx942_profile_v1(), projection)
}

pub open spec fn projection_refines_for_profile_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
    projection: ModelProjectionV1,
) -> bool {
    &&& canonical_equal_v1(projection.canonical, observation)
    &&& projection.target_profile == profile.target_profile
    &&& projection.domain == observation.domain
    &&& projection.profile == observation.profile
    &&& projection.physical == observation.physical
    &&& projection.pci == observation.pci
    &&& projection.kfd_gpu_id == observation.kfd_gpu_id
    &&& projection.render_minor == observation.render_minor
    &&& projection.kfd_schema == observation.kfd_schema
    &&& projection.drm_schema == observation.drm_schema
}

pub open spec fn inventory_matches_target_profile_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionTargetProfileV1,
) -> bool {
    forall |i: int| 0 <= i < observation.inventory.len() ==> {
        let identity = #[trigger] observation.inventory[i];
        &&& identity.vendor == 0x1002
        &&& identity.device > 0
        &&& identity.target == profile_tuple_v1(profile).target
    }
}

pub open spec fn canonical_observation_admitted_v1(observation: CanonicalObservationV1) -> bool {
    canonical_observation_admitted_for_profile_v1(observation, gfx942_profile_v1())
}

pub open spec fn canonical_observation_admitted_for_profile_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
) -> bool {
    &&& observation.schema_version == 1
    &&& observation.domain > 0
    &&& observation.profile == profile.identity
    &&& observation.kfd_schema == profile.kfd_schema
    &&& observation.drm_schema == profile.drm_schema
    &&& observation.source_commitment > 0
    &&& observation.module_file_system_device > 0
    &&& observation.module_inode > 0
    &&& observation.kernel_release == KernelReleaseObservationV1::Linux6_8_0_124Generic
    &&& observation.amdgpu_module == profile_tuple_v1(profile.target_profile).module
    &&& observation.kfd_descriptor_commitment > 0
    &&& observation.render_descriptor_commitment > 0
    &&& observation.aperture_inventory_commitment > 0
    &&& observation.inventory.len() > 0
    &&& observation.inventory.len() <= 16
    &&& inventory_globally_unique_v1(observation)
    &&& exactly_one_selected_inventory_v1(observation)
    &&& inventory_matches_target_profile_v1(observation, profile.target_profile)
    &&& observation.topology_generation > 0
    &&& observation.physical > 0
    &&& observation.topology_gpu_unique == observation.physical
    &&& observation.render_gpu_unique == observation.physical
    &&& observation.pci == observation.render_pci
    &&& observation.kfd_gpu_id > 0
    &&& observation.topology_render_minor == observation.render_minor
    &&& observation.vendor == 0x1002
    &&& observation.device == profile_tuple_v1(profile.target_profile).pci_device
    &&& observation.pci_revision == 0
    &&& observation.target == profile_tuple_v1(profile.target_profile).target
    &&& observation.compute_partition == 1
    &&& observation.memory_partition == 1
    &&& observation.kfd_major > 0
    &&& observation.kfd_minor == 0
    &&& observation.kfd_uapi_major == 1
    &&& observation.kfd_uapi_minor == 18
    &&& observation.xnack == 0
    &&& observation.drm_major == 3
    &&& observation.drm_minor == 64
    &&& observation.drm_patch == 0
    &&& observation.acceleration_working
    &&& observation.drm_family == 141
    &&& observation.drm_chip_revision == profile_tuple_v1(profile.target_profile).chip_revision
    &&& observation.drm_external_revision == profile_tuple_v1(profile.target_profile).external_revision
    &&& observation.firmware == profile_tuple_v1(profile.target_profile).firmware
    &&& observation.sdma_firmware == profile_tuple_v1(profile.target_profile).sdma_firmware
    &&& observation.wavefront_size == 64
    &&& observation.simd_count == profile_tuple_v1(profile.target_profile).simd_count
    &&& observation.xcc_count == 8
    &&& observation.initial_vram_lost_counter <= 0xffff_ffff
    &&& observation.commit_fence_complete
    &&& observation.reset_subscription_established
    &&& observation.reset_event_mask_enabled
    &&& observation.reset_event_descriptor_cloexec
    &&& observation.reset_fence_initially_clear
    &&& observation.drm_reobserved_after_subscription_equal
    &&& observation.reset_fence_clear_before_commit
}

pub open spec fn model_projection_admitted_v1(projection: ModelProjectionV1) -> bool {
    model_projection_admitted_for_profile_v1(projection, gfx942_profile_v1())
}

pub open spec fn model_projection_admitted_for_profile_v1(
    projection: ModelProjectionV1,
    profile: DeviceAdmissionProfileV1,
) -> bool {
    &&& canonical_observation_admitted_for_profile_v1(projection.canonical, profile)
    &&& projection_refines_for_profile_v1(projection.canonical, profile, projection)
    &&& projection.render_minor >= 128
}

pub proof fn profile_parameterized_projection_retains_target_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
)
    ensures
        projection_refines_for_profile_v1(
            observation, profile, project_for_profile_v1(observation, profile),
        ),
        project_for_profile_v1(observation, profile).target_profile == profile.target_profile,
{
}

pub proof fn admitted_projection_preserves_exact_target_profile_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
)
    requires
        canonical_observation_admitted_for_profile_v1(observation, profile),
        observation.render_minor >= 128,
    ensures
        model_projection_admitted_for_profile_v1(
            project_for_profile_v1(observation, profile), profile,
        ),
        project_for_profile_v1(observation, profile).canonical.target
            == profile_tuple_v1(profile.target_profile).target,
{
}

pub proof fn equal_profile_digest_does_not_relabel_projection_target_v1(
    observation: CanonicalObservationV1,
    original: DeviceAdmissionProfileV1,
    substituted: DeviceAdmissionProfileV1,
)
    requires
        original.identity == substituted.identity,
        original.kfd_schema == substituted.kfd_schema,
        original.drm_schema == substituted.drm_schema,
        original.target_profile != substituted.target_profile,
        canonical_observation_admitted_for_profile_v1(observation, original),
    ensures
        !canonical_observation_admitted_for_profile_v1(observation, substituted),
        !model_projection_admitted_for_profile_v1(
            project_for_profile_v1(observation, original), substituted,
        ),
{
}

pub proof fn admitted_inventory_retains_selected_target_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
    selected: int,
)
    requires
        canonical_observation_admitted_for_profile_v1(observation, profile),
        0 <= selected < observation.inventory.len(),
        observation.inventory[selected].gpu_unique == observation.physical,
    ensures
        observation.inventory[selected].target == observation.target,
        observation.inventory[selected].device == observation.device,
        observation.inventory[selected].pci_revision == observation.pci_revision,
{
}

pub proof fn substituted_inventory_target_is_rejected_v1(
    observation: CanonicalObservationV1,
    profile: DeviceAdmissionProfileV1,
    index: int,
)
    requires
        0 <= index < observation.inventory.len(),
        observation.inventory[index].target != profile_tuple_v1(profile.target_profile).target,
    ensures
        !canonical_observation_admitted_for_profile_v1(observation, profile),
{
}

// A model witness, not an observation of any running host or XNACK state.
pub open spec fn example_observation_for_profile_v1(profile: DeviceAdmissionProfileV1)
    -> CanonicalObservationV1
{
    let tuple = profile_tuple_v1(profile.target_profile);
    CanonicalObservationV1 {
        schema_version: 1, domain: 1, profile: profile.identity,
        kfd_schema: profile.kfd_schema, drm_schema: profile.drm_schema,
        source_commitment: 1, module_file_system_device: 1, module_inode: 1,
        kernel_release: KernelReleaseObservationV1::Linux6_8_0_124Generic,
        amdgpu_module: tuple.module,
        kfd_descriptor_commitment: 1, render_descriptor_commitment: 2,
        aperture_inventory_commitment: 1,
        inventory: Seq::empty().push(InventoryIdentityV1 {
            topology_node: 1, kfd_gpu_id: 1, gpu_unique: 1, render_minor: 128,
            pci: 1, vendor: 0x1002, device: tuple.pci_device, pci_revision: 0,
            target: tuple.target,
        }),
        topology_generation: 1, topology_node: 1, physical: 1,
        topology_gpu_unique: 1, render_gpu_unique: 1, pci: 1, render_pci: 1,
        kfd_gpu_id: 1, topology_render_minor: 128, render_minor: 128,
        vendor: 0x1002, device: tuple.pci_device, pci_revision: 0, target: tuple.target,
        compute_partition: 1, memory_partition: 1, kfd_major: 1, kfd_minor: 0,
        kfd_uapi_major: 1, kfd_uapi_minor: 18, xnack: 0,
        drm_major: 3, drm_minor: 64, drm_patch: 0, acceleration_working: true,
        drm_family: 141, drm_chip_revision: tuple.chip_revision,
        drm_external_revision: tuple.external_revision, firmware: tuple.firmware,
        sdma_firmware: tuple.sdma_firmware, wavefront_size: 64,
        simd_count: tuple.simd_count, xcc_count: 8, initial_vram_lost_counter: 0,
        commit_fence_complete: true, reset_subscription_established: true,
        reset_event_mask_enabled: true, reset_event_descriptor_cloexec: true,
        reset_fence_initially_clear: true, drm_reobserved_after_subscription_equal: true,
        reset_fence_clear_before_commit: true,
    }
}

pub proof fn both_target_projection_profiles_are_inhabited_v1(profile: DeviceAdmissionProfileV1)
    ensures
        canonical_observation_admitted_for_profile_v1(
            example_observation_for_profile_v1(profile), profile,
        ),
        model_projection_admitted_for_profile_v1(
            project_for_profile_v1(example_observation_for_profile_v1(profile), profile), profile,
        ),
{
    let observation = example_observation_for_profile_v1(profile);
    assert(selected_inventory_identity_matches_v1(observation, 0));
    assert(exactly_one_selected_inventory_v1(observation));
}

pub proof fn canonical_projection_refines_every_retained_field_v1(
    observation: CanonicalObservationV1,
)
    ensures
        projection_refines_v1(observation, project_v1(observation)),
{
}

pub proof fn admitted_canonical_projection_satisfies_model_profile_v1(
    observation: CanonicalObservationV1,
)
    requires
        canonical_observation_admitted_v1(observation),
        observation.render_minor >= 128,
    ensures
        model_projection_admitted_v1(project_v1(observation)),
{
}

pub proof fn canonical_projection_retains_exact_inventory_v1(
    observation: CanonicalObservationV1,
)
    requires
        observation.inventory.len() <= 16,
        inventory_globally_unique_v1(observation),
        exactly_one_selected_inventory_v1(observation),
    ensures
        project_v1(observation).canonical.inventory =~= observation.inventory,
        project_v1(observation).canonical.inventory.len() <= 16,
        inventory_globally_unique_v1(project_v1(observation).canonical),
        exactly_one_selected_inventory_v1(project_v1(observation).canonical),
{
}

pub struct ProjectionHistoryEntryV1 {
    pub physical: nat,
    pub generation: nat,
    pub predecessor_generation: nat,
    pub projection: ModelProjectionV1,
}

pub struct ProjectionHistoryV1 {
    pub physical: nat,
    pub entries: Seq<ProjectionHistoryEntryV1>,
}

// Explicit entry instantiation avoids recursively triggering every predecessor access.
pub open spec fn projection_history_entry_valid_v1(
    history: ProjectionHistoryV1,
    i: int,
) -> bool {
    let entry = history.entries[i];
    &&& entry.physical == history.physical
    &&& entry.projection.physical == history.physical
    &&& entry.generation > 0
    &&& if i == 0 {
        entry.predecessor_generation == 0
    } else {
        &&& entry.predecessor_generation == history.entries[i - 1].generation
        &&& history.entries[i - 1].generation < entry.generation
    }
}

pub open spec fn projection_history_valid_v1(history: ProjectionHistoryV1) -> bool {
    &&& history.physical > 0
    &&& forall |i: int| 0 <= i < history.entries.len()
        ==> #[trigger] projection_history_entry_valid_v1(history, i)
}

pub open spec fn append_projection_history_v1(
    history: ProjectionHistoryV1,
    projection: ModelProjectionV1,
    generation: nat,
) -> ProjectionHistoryV1 {
    let predecessor = if history.entries.len() == 0 {
        0
    } else {
        history.entries.last().generation
    };
    ProjectionHistoryV1 {
        physical: history.physical,
        entries: history.entries.push(ProjectionHistoryEntryV1 {
            physical: history.physical,
            generation,
            predecessor_generation: predecessor,
            projection,
        }),
    }
}

pub proof fn append_preserves_projection_history_generations_v1(
    history: ProjectionHistoryV1,
    projection: ModelProjectionV1,
    generation: nat,
)
    requires
        projection_history_valid_v1(history),
        projection.physical == history.physical,
        generation > 0,
        history.entries.len() == 0 || history.entries.last().generation < generation,
    ensures
        projection_history_valid_v1(
            append_projection_history_v1(history, projection, generation),
        ),
{
    let next = append_projection_history_v1(history, projection, generation);
    assert forall |i: int| 0 <= i < next.entries.len()
        implies #[trigger] projection_history_entry_valid_v1(next, i) by {
        let old_len = history.entries.len() as int;
        if i < old_len {
            assert(projection_history_entry_valid_v1(history, i));
            assert(next.entries[i].physical == history.entries[i].physical);
            assert(next.entries[i].projection.physical == history.entries[i].projection.physical);
            assert(next.entries[i].generation == history.entries[i].generation);
            assert(next.entries[i].predecessor_generation
                == history.entries[i].predecessor_generation);
            if i > 0 {
                assert(next.entries[i - 1].generation == history.entries[i - 1].generation);
            }
        } else {
            assert(i == old_len);
            if i == 0 {
                assert(history.entries.len() == 0);
            } else {
                assert(next.entries[i - 1].generation == history.entries.last().generation);
            }
        }
    }
}

pub proof fn append_preserves_exact_projection_history_link_v1(
    history: ProjectionHistoryV1,
    projection: ModelProjectionV1,
    generation: nat,
)
    requires
        projection_history_valid_v1(history),
        projection.physical == history.physical,
        generation > 0,
        history.entries.len() == 0 || history.entries.last().generation < generation,
    ensures
        projection_history_valid_v1(
            append_projection_history_v1(history, projection, generation),
        ),
        canonical_equal_v1(
            append_projection_history_v1(history, projection, generation)
                .entries.last().projection.canonical,
            projection.canonical,
        ),
        append_projection_history_v1(history, projection, generation)
            .entries.last().projection.physical == projection.physical,
{
    append_preserves_projection_history_generations_v1(history, projection, generation);
}

} // verus!
