//! Side-by-side generated host contract for issue #272 W7.

use crate::{
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
    GeneratedArgumentLayoutError, GeneratedArgumentPackingError, GeneratedArgumentPackingPlanV1,
    generated_argument_plan::validate_worker_v3_argument_packing,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, Access, AddressSpace as AbiAddressSpace, AliasClass, BlockSize,
    LaunchContract, PointerWidth, ScalarType as AbiScalarType,
};
use fe2o3_compiler_ffi::InertProductionCapabilityResultV5;
use fe2o3_kernel_descriptor::{
    BlockSizeV1, CanonicalCodeObjectDigest, DeviceDescriptorTableDigest, DeviceDescriptorTableV1,
    KernelDescriptorDigest, KernelDescriptorV1,
};
use fe2o3_kernel_ir::{
    AccessMode as KirAccessMode, AddressSpace as KirAddressSpace, Function, FunctionRole, Module,
    OperationKind, ScalarType as KirScalarType, Type as KirType, VerifiedCanonicalKernelIrV13,
    decode_module_v13,
};
use fe2o3_verifier::ValidatedCompilerCapabilitySourceOwnerV1;
use sha2::{Digest, Sha256};
use std::{fmt, marker::PhantomData, sync::Arc};

/// Maximum tensor rank accepted by the V2 generated host boundary.
pub const MAX_GENERATED_HOST_TENSOR_RANK_V2: usize = 8;
/// Maximum canonical memory constraints accepted for one kernel.
pub const MAX_GENERATED_HOST_MEMORY_ARGUMENTS_V2: usize = 64;

/// Invalid const-generic tensor metadata attached to a generated slice capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedHostTensorLayoutErrorV2 {
    Rank,
}

impl fmt::Display for GeneratedHostTensorLayoutErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generated tensor rank must be between one and eight")
    }
}

impl std::error::Error for GeneratedHostTensorLayoutErrorV2 {}

/// Compiler-generated, authority-free ABI and launch facts for one kernel marker.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedHostContractV2 {
    arguments: CompilerGeneratedArgumentLayoutV1,
    launch: LaunchContract,
}

impl CompilerGeneratedHostContractV2 {
    #[doc(hidden)]
    pub fn new(arguments: CompilerGeneratedArgumentLayoutV1, launch: LaunchContract) -> Self {
        Self { arguments, launch }
    }

    pub const fn arguments(&self) -> &CompilerGeneratedArgumentLayoutV1 {
        &self.arguments
    }

    pub const fn launch(&self) -> &LaunchContract {
        &self.launch
    }
}

/// Side-by-side V2 expectation emitted from one parsed kernel signature.
///
/// # Safety
///
/// The implementation must be generated from the same canonical signature and launch
/// registration as `Self`. A false implementation can misdescribe retained argument borrows.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedKernelExpectationV2:
    CompilerGeneratedKernelExpectationV1
{
    fn generated_host_contract_v2()
    -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError>;
}

/// Semantic use of one memory argument in the canonical executable graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedHostMemoryRoleV2 {
    Input,
    Output,
    InputOutput,
    Workspace,
}

impl GeneratedHostMemoryRoleV2 {
    const fn expected_access(self) -> Access {
        match self {
            Self::Input => Access::ReadOnly,
            Self::Output => Access::WriteOnly,
            Self::InputOutput | Self::Workspace => Access::ReadWrite,
        }
    }

    const fn expected_alias(self) -> AliasClass {
        match self {
            Self::Input => AliasClass::SharedReadOnly,
            Self::Output | Self::InputOutput | Self::Workspace => AliasClass::Exclusive,
        }
    }
}

/// Inclusive range required for one dynamic extent or element stride.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedHostAxisConstraintV2 {
    minimum_extent: u64,
    maximum_extent: u64,
    minimum_stride: u64,
    maximum_stride: u64,
}

impl GeneratedHostAxisConstraintV2 {
    pub(crate) fn new(
        minimum_extent: u64,
        maximum_extent: u64,
        minimum_stride: u64,
        maximum_stride: u64,
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        if minimum_extent > maximum_extent || minimum_stride > maximum_stride {
            return Err(GeneratedHostContractErrorV2::InvalidCanonicalConstraint);
        }
        Ok(Self {
            minimum_extent,
            maximum_extent,
            minimum_stride,
            maximum_stride,
        })
    }

    pub const fn minimum_extent(self) -> u64 {
        self.minimum_extent
    }

    pub const fn maximum_extent(self) -> u64 {
        self.maximum_extent
    }

    pub const fn minimum_stride(self) -> u64 {
        self.minimum_stride
    }

    pub const fn maximum_stride(self) -> u64 {
        self.maximum_stride
    }
}

/// Canonical KIR-derived dynamic requirements for one memory argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedHostMemoryConstraintV2 {
    argument_index: usize,
    role: GeneratedHostMemoryRoleV2,
    element_bytes: u64,
    required_alignment: u64,
    axes: Box<[GeneratedHostAxisConstraintV2]>,
}

impl GeneratedHostMemoryConstraintV2 {
    pub(crate) fn new(
        argument_index: usize,
        role: GeneratedHostMemoryRoleV2,
        element_bytes: u64,
        required_alignment: u64,
        axes: impl Into<Vec<GeneratedHostAxisConstraintV2>>,
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        let axes = axes.into();
        if axes.is_empty()
            || axes.len() > MAX_GENERATED_HOST_TENSOR_RANK_V2
            || element_bytes == 0
            || required_alignment == 0
            || !required_alignment.is_power_of_two()
        {
            return Err(GeneratedHostContractErrorV2::InvalidCanonicalConstraint);
        }
        Ok(Self {
            argument_index,
            role,
            element_bytes,
            required_alignment,
            axes: axes.into_boxed_slice(),
        })
    }

    pub const fn argument_index(&self) -> usize {
        self.argument_index
    }

    pub const fn role(&self) -> GeneratedHostMemoryRoleV2 {
        self.role
    }

    pub const fn element_bytes(&self) -> u64 {
        self.element_bytes
    }

    pub const fn required_alignment(&self) -> u64 {
        self.required_alignment
    }

    pub fn axes(&self) -> &[GeneratedHostAxisConstraintV2] {
        &self.axes
    }
}

const GENERATED_HOST_V13_ASSOCIATION_DOMAIN_V2: &[u8] =
    b"FE2O3/HOST/PRODUCTION-V13-STATIC-ASSOCIATION/V2\0";

/// Exact V13 graph, target, and launch coordinates carried by one production owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedHostExecutionSubjectV2 {
    final_graph_identity: [u8; 32],
    final_graph_bytes: u64,
    final_epoch: u64,
    target_model_identity: [u8; 32],
    launch_contract_identity: [u8; 32],
}

impl GeneratedHostExecutionSubjectV2 {
    fn from_v13(
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target_model_identity: [u8; 32],
        launch_contract_identity: [u8; 32],
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        final_graph
            .revalidate()
            .map_err(|_| GeneratedHostContractErrorV2::InvalidCanonicalKirV13)?;
        if final_epoch == 0
            || target_model_identity == [0; 32]
            || launch_contract_identity == [0; 32]
        {
            return Err(GeneratedHostContractErrorV2::InvalidProductionEvidence);
        }
        Ok(Self {
            final_graph_identity: *final_graph.identity().digest(),
            final_graph_bytes: final_graph.identity().canonical_length(),
            final_epoch,
            target_model_identity,
            launch_contract_identity,
        })
    }

    pub const fn final_graph_identity(self) -> [u8; 32] {
        self.final_graph_identity
    }

    pub const fn final_graph_bytes(self) -> u64 {
        self.final_graph_bytes
    }

    pub const fn final_epoch(self) -> u64 {
        self.final_epoch
    }

    pub const fn target_model_identity(self) -> [u8; 32] {
        self.target_model_identity
    }

    pub const fn launch_contract_identity(self) -> [u8; 32] {
        self.launch_contract_identity
    }
}

#[derive(Debug)]
struct ProductionV13StaticAssociationV2 {
    final_graph: VerifiedCanonicalKernelIrV13,
    production_result: Option<InertProductionCapabilityResultV5>,
    subject: GeneratedHostExecutionSubjectV2,
    capability_closure_identity: [u8; 32],
    capability_association_identity: [u8; 32],
    capability_association_bytes: u64,
    dynamic_precondition_roster_identity: [u8; 32],
    compiler_policy_identity: [u8; 32],
    identity: [u8; 32],
}

#[derive(Debug)]
struct GeneratedHostV13AbiProofV2 {
    zero_byte_logical_capabilities: usize,
    physical_parameters: Box<[GeneratedHostV13PhysicalParameterV2]>,
    identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneratedHostV13PhysicalParameterV2 {
    kir_parameter: usize,
    physical_argument: usize,
    offset: u64,
    size: u64,
    alignment: u32,
}

impl ProductionV13StaticAssociationV2 {
    fn from_production_result(
        production_result: InertProductionCapabilityResultV5,
        kernel_ordinal: usize,
        dynamic_precondition_roster_identity: [u8; 32],
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        let handoff = production_result.handoff();
        let carried_subject = *handoff
            .subjects()
            .get(kernel_ordinal)
            .ok_or(GeneratedHostContractErrorV2::KernelOrdinal)?;
        let final_graph = VerifiedCanonicalKernelIrV13::from_canonical_bytes(
            handoff.executable_kir().canonical_preimage().to_vec(),
        )
        .map_err(|_| GeneratedHostContractErrorV2::InvalidCanonicalKirV13)?;
        let capability_association = production_result
            .capability_associations()
            .entries()
            .get(kernel_ordinal)
            .ok_or(GeneratedHostContractErrorV2::KernelOrdinal)?
            .identity();
        let result_identity = production_result.identity();
        let executable_kir_bytes = production_result
            .proof_owner()
            .inputs()
            .executable_kir_bytes();
        let subject = GeneratedHostExecutionSubjectV2::from_v13(
            &final_graph,
            carried_subject.executable_kir_epoch(),
            *carried_subject.target_model().digest().as_bytes(),
            *carried_subject.launch_contract().digest().as_bytes(),
        )?;
        let target_closure = handoff.target_closure();
        let final_graph_report = handoff.final_graph_report();
        if carried_subject.executable_kir().digest().as_bytes() != &subject.final_graph_identity
            || executable_kir_bytes != subject.final_graph_bytes
            || target_closure.neutral_graph() != subject.final_graph_identity
            || target_closure.neutral_graph_bytes() != subject.final_graph_bytes
            || target_closure.neutral_epoch() != subject.final_epoch
            || final_graph_report.final_graph() != subject.final_graph_identity
            || final_graph_report.final_graph_bytes() != subject.final_graph_bytes
            || final_graph_report.final_epoch() != subject.final_epoch
        {
            return Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch);
        }
        let capability_closure_identity = target_closure.closure_identity();
        let capability_association_identity = capability_association.sha256();
        let capability_association_bytes = capability_association.byte_len();
        let compiler_policy_identity = handoff.inputs().compiler_policy();
        Self::new(
            final_graph,
            Some(production_result),
            subject,
            capability_closure_identity,
            capability_association_identity,
            capability_association_bytes,
            dynamic_precondition_roster_identity,
            compiler_policy_identity,
            Some((result_identity.sha256(), result_identity.byte_len())),
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn for_test_only(
        final_graph: VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target_model_identity: [u8; 32],
        launch_contract_identity: [u8; 32],
        capability_closure_identity: [u8; 32],
        capability_association_identity: [u8; 32],
        capability_association_bytes: u64,
        dynamic_precondition_roster_identity: [u8; 32],
        compiler_policy_identity: [u8; 32],
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        let subject = GeneratedHostExecutionSubjectV2::from_v13(
            &final_graph,
            final_epoch,
            target_model_identity,
            launch_contract_identity,
        )?;
        Self::new(
            final_graph,
            None,
            subject,
            capability_closure_identity,
            capability_association_identity,
            capability_association_bytes,
            dynamic_precondition_roster_identity,
            compiler_policy_identity,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        final_graph: VerifiedCanonicalKernelIrV13,
        production_result: Option<InertProductionCapabilityResultV5>,
        subject: GeneratedHostExecutionSubjectV2,
        capability_closure_identity: [u8; 32],
        capability_association_identity: [u8; 32],
        capability_association_bytes: u64,
        dynamic_precondition_roster_identity: [u8; 32],
        compiler_policy_identity: [u8; 32],
        production_result_identity: Option<([u8; 32], u64)>,
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        if capability_closure_identity == [0; 32]
            || capability_association_identity == [0; 32]
            || capability_association_bytes == 0
            || dynamic_precondition_roster_identity == [0; 32]
            || compiler_policy_identity == [0; 32]
        {
            return Err(GeneratedHostContractErrorV2::InvalidProductionEvidence);
        }
        let mut digest = Sha256::new();
        digest.update(GENERATED_HOST_V13_ASSOCIATION_DOMAIN_V2);
        digest.update(subject.final_graph_identity);
        digest.update(subject.final_graph_bytes.to_le_bytes());
        digest.update(subject.final_epoch.to_le_bytes());
        digest.update(subject.target_model_identity);
        digest.update(subject.launch_contract_identity);
        digest.update(capability_closure_identity);
        digest.update(capability_association_identity);
        digest.update(capability_association_bytes.to_le_bytes());
        digest.update(dynamic_precondition_roster_identity);
        digest.update(compiler_policy_identity);
        if let Some((sha256, byte_len)) = production_result_identity {
            digest.update(sha256);
            digest.update(byte_len.to_le_bytes());
        }
        let identity = digest.finalize().into();
        Ok(Self {
            final_graph,
            production_result,
            subject,
            capability_closure_identity,
            capability_association_identity,
            capability_association_bytes,
            dynamic_precondition_roster_identity,
            compiler_policy_identity,
            identity,
        })
    }
}

/// Move-only production inputs consumed by V2 host-contract admission.
///
/// This owner is intentionally not safely constructible. The production artifact/evidence join
/// must build it from exact carried records, not from application-provided digests.
#[derive(Debug)]
#[must_use = "dropping production host facts abandons the W7 host join"]
pub struct ProductionGeneratedHostFactsV2 {
    kernel_binding_identity: [u8; 32],
    generated_host_contract_identity: [u8; 32],
    descriptor_target: Box<str>,
    artifact_identity: CanonicalCodeObjectDigest,
    descriptor_table_identity: DeviceDescriptorTableDigest,
    descriptor_identity: KernelDescriptorDigest,
    entry_name: Box<str>,
    kernel_ordinal: usize,
    static_association: ProductionV13StaticAssociationV2,
    memory: Option<Box<[GeneratedHostMemoryConstraintV2]>>,
}

impl ProductionGeneratedHostFactsV2 {
    /// Consumes the complete V13 worker result while retaining generated ABI and artifact facts.
    ///
    /// The result owns the exact graph, target closure, final-graph report, capability
    /// association, and machine-refinement receipt. These facts remain inert until joined to the
    /// independently inspected artifact and sealed Worker admission at dispatch preparation.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_production_capability_result_v5(
        kernel_binding_identity: [u8; 32],
        generated_host_contract_identity: [u8; 32],
        descriptor_target: impl Into<Box<str>>,
        artifact_identity: CanonicalCodeObjectDigest,
        descriptor_table_identity: DeviceDescriptorTableDigest,
        descriptor_identity: KernelDescriptorDigest,
        entry_name: impl Into<Box<str>>,
        kernel_ordinal: usize,
        dynamic_precondition_roster_identity: [u8; 32],
        production_result: InertProductionCapabilityResultV5,
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        let descriptor_target = descriptor_target.into();
        let entry_name = entry_name.into();
        if kernel_binding_identity == [0; 32]
            || generated_host_contract_identity == [0; 32]
            || descriptor_target.is_empty()
            || entry_name.is_empty()
        {
            return Err(GeneratedHostContractErrorV2::InvalidProductionEvidence);
        }
        Ok(Self {
            kernel_binding_identity,
            generated_host_contract_identity,
            descriptor_target,
            artifact_identity,
            descriptor_table_identity,
            descriptor_identity,
            entry_name,
            kernel_ordinal,
            static_association: ProductionV13StaticAssociationV2::from_production_result(
                production_result,
                kernel_ordinal,
                dynamic_precondition_roster_identity,
            )?,
            memory: None,
        })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn for_test_only(
        kernel_binding_identity: [u8; 32],
        generated_host_contract_identity: [u8; 32],
        descriptor_target: impl Into<Box<str>>,
        artifact_identity: CanonicalCodeObjectDigest,
        descriptor_table_identity: DeviceDescriptorTableDigest,
        descriptor_identity: KernelDescriptorDigest,
        entry_name: impl Into<Box<str>>,
        kernel_ordinal: usize,
        final_graph: VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target_model_identity: [u8; 32],
        launch_contract_identity: [u8; 32],
        capability_closure_identity: [u8; 32],
        capability_association_identity: [u8; 32],
        capability_association_bytes: u64,
        dynamic_precondition_roster_identity: [u8; 32],
        compiler_policy_identity: [u8; 32],
        memory: impl Into<Vec<GeneratedHostMemoryConstraintV2>>,
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        let descriptor_target = descriptor_target.into();
        let entry_name = entry_name.into();
        let mut memory = memory.into();
        if kernel_binding_identity == [0; 32]
            || generated_host_contract_identity == [0; 32]
            || descriptor_target.is_empty()
            || entry_name.is_empty()
            || memory.len() > MAX_GENERATED_HOST_MEMORY_ARGUMENTS_V2
        {
            return Err(GeneratedHostContractErrorV2::InvalidProductionEvidence);
        }
        memory.sort_unstable_by_key(GeneratedHostMemoryConstraintV2::argument_index);
        if memory
            .windows(2)
            .any(|pair| pair[0].argument_index == pair[1].argument_index)
        {
            return Err(GeneratedHostContractErrorV2::DuplicateMemoryConstraint);
        }
        Ok(Self {
            kernel_binding_identity,
            generated_host_contract_identity,
            descriptor_target,
            artifact_identity,
            descriptor_table_identity,
            descriptor_identity,
            entry_name,
            kernel_ordinal,
            static_association: ProductionV13StaticAssociationV2::for_test_only(
                final_graph,
                final_epoch,
                target_model_identity,
                launch_contract_identity,
                capability_closure_identity,
                capability_association_identity,
                capability_association_bytes,
                dynamic_precondition_roster_identity,
                compiler_policy_identity,
            )?,
            memory: Some(memory.into_boxed_slice()),
        })
    }
}

/// Authority-free result of matching generated, compiler-carried, and inspected facts.
#[derive(Debug)]
#[must_use = "host-contract agreement alone grants no load or launch authority"]
pub struct AdmittedGeneratedHostContractV2<K> {
    packing: GeneratedArgumentPackingPlanV1,
    kernel_ordinal: usize,
    artifact_identity: CanonicalCodeObjectDigest,
    descriptor_table_identity: DeviceDescriptorTableDigest,
    descriptor_identity: KernelDescriptorDigest,
    entry_name: Box<str>,
    static_association: Arc<ProductionV13StaticAssociationV2>,
    abi_proof: GeneratedHostV13AbiProofV2,
    memory: Box<[GeneratedHostMemoryConstraintV2]>,
    _marker: PhantomData<fn() -> K>,
}

impl<K> AdmittedGeneratedHostContractV2<K> {
    pub fn subject(&self) -> GeneratedHostExecutionSubjectV2 {
        self.static_association.subject
    }

    pub fn capability_closure_identity(&self) -> [u8; 32] {
        self.static_association.capability_closure_identity
    }

    pub fn capability_association_identity(&self) -> ([u8; 32], u64) {
        (
            self.static_association.capability_association_identity,
            self.static_association.capability_association_bytes,
        )
    }

    /// Returns the complete protected compiler-policy identity.
    ///
    /// Operation-level numerical requirements remain part of the exact canonical KIR subject and
    /// authenticated target-capability closure; they are not projected out into a second policy.
    pub fn compiler_policy_identity(&self) -> [u8; 32] {
        self.static_association.compiler_policy_identity
    }

    pub fn dynamic_precondition_roster_identity(&self) -> [u8; 32] {
        self.static_association
            .dynamic_precondition_roster_identity
    }

    pub const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        &self.packing
    }

    pub const fn kernel_ordinal(&self) -> usize {
        self.kernel_ordinal
    }

    pub const fn artifact_identity(&self) -> CanonicalCodeObjectDigest {
        self.artifact_identity
    }

    pub const fn descriptor_table_identity(&self) -> DeviceDescriptorTableDigest {
        self.descriptor_table_identity
    }

    pub const fn descriptor_identity(&self) -> KernelDescriptorDigest {
        self.descriptor_identity
    }

    pub fn entry_name(&self) -> &str {
        &self.entry_name
    }

    pub fn v13_static_association_identity(&self) -> [u8; 32] {
        self.static_association.identity
    }

    pub const fn zero_byte_logical_capability_count(&self) -> usize {
        self.abi_proof.zero_byte_logical_capabilities
    }

    pub fn physical_abi_parameter_count(&self) -> usize {
        self.abi_proof.physical_parameters.len()
    }

    pub const fn physical_abi_identity(&self) -> [u8; 32] {
        self.abi_proof.identity
    }

    /// Binds runtime coordinates to this exact admitted V13/artifact association.
    pub fn bind_dispatch_evidence_v2(
        &self,
        runtime: GeneratedHostRuntimeCoordinatesV2,
    ) -> GeneratedHostDispatchEvidenceV2 {
        GeneratedHostDispatchEvidenceV2 {
            runtime,
            subject: self.subject(),
            capability_closure_identity: self.capability_closure_identity(),
            dynamic_precondition_roster_identity: self.dynamic_precondition_roster_identity(),
            compiler_policy_identity: self.compiler_policy_identity(),
            static_association_identity: self.static_association.identity,
            artifact_identity: self.artifact_identity,
            descriptor_table_identity: self.descriptor_table_identity,
            descriptor_identity: self.descriptor_identity,
            kernel_ordinal: self.kernel_ordinal,
        }
    }
}

impl<K: CompilerGeneratedKernelExpectationV2> AdmittedGeneratedHostContractV2<K> {
    pub(crate) fn validate_sealed_v13_source_v2(
        &self,
        source: &ValidatedCompilerCapabilitySourceOwnerV1,
    ) -> Result<(), GeneratedHostContractErrorV2> {
        let result = self
            .static_association
            .production_result
            .as_ref()
            .ok_or(GeneratedHostContractErrorV2::MissingSealedProductionResult)?;
        let capability = source.capability();
        let source_graph = capability.kernel_ir();
        source_graph
            .revalidate()
            .map_err(|_| GeneratedHostContractErrorV2::InvalidCanonicalKirV13)?;
        let source_subject = source.proof_owner().association().inputs().subject();
        let result_subject = result.handoff().inputs().subject();
        let source_association = capability.association().identity();
        let result_association = result.capability_association().identity();
        let source_machine = capability
            .machine_refinement()
            .ok_or(GeneratedHostContractErrorV2::MissingMachineRefinementReceipt)?;
        let result_machine = result.machine_refinement();
        let source_result_identity = source.production_result_identity();
        let result_identity = result.identity();
        let graph_identity = source_graph.identity();
        let host_subject = self.subject();
        if source_subject != result_subject
            || source_graph.canonical_bytes()
                != self.static_association.final_graph.canonical_bytes()
            || graph_identity.digest() != &host_subject.final_graph_identity
            || graph_identity.canonical_length() != host_subject.final_graph_bytes
            || source_subject.executable_kir_epoch() != host_subject.final_epoch
            || source_subject.target_model().digest().as_bytes()
                != &host_subject.target_model_identity
            || source_subject.launch_contract().digest().as_bytes()
                != &host_subject.launch_contract_identity
            || source_association.sha256()
                != self.static_association.capability_association_identity
            || source_association.byte_len() != self.static_association.capability_association_bytes
            || result_association != source_association
            || result.proof_owner().identity() != source.proof_owner().association().identity()
            || result_machine.identity() != source_machine.identity()
            || result_machine.canonical_preimage() != source_machine.canonical_preimage()
            || result_identity != source_result_identity
        {
            return Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch);
        }

        let module = decode_module_v13(source_graph.canonical_bytes())
            .map_err(|_| GeneratedHostContractErrorV2::InvalidCanonicalKirV13)?;
        let kernel = module
            .kernels
            .get(self.kernel_ordinal)
            .ok_or(GeneratedHostContractErrorV2::KernelOrdinal)?;
        let entry = module
            .function(&kernel.entry)
            .ok_or(GeneratedHostContractErrorV2::V13KernelEntry)?;
        let source_root = entry
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                OperationKind::KernelContextIssue(issue) => Some(issue.source().function()),
                _ => None,
            })
            .ok_or(GeneratedHostContractErrorV2::MissingV13LogicalCapability)?;
        if source_subject.kernel().digest().as_bytes() != &K::KERNEL_BINDING_ID_V1
            || source_subject.root().digest().as_bytes() != &source_root
        {
            return Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch);
        }

        Ok(())
    }
}

/// Reconciles V2 generated facts with an independently inspected descriptor and exact production
/// evidence. The result remains inert and must be joined to the existing runtime authority.
#[doc(hidden)]
pub fn admit_generated_host_contract_v2<K: CompilerGeneratedKernelExpectationV2>(
    table: &DeviceDescriptorTableV1,
    descriptor: &KernelDescriptorV1,
    mut production: ProductionGeneratedHostFactsV2,
) -> Result<AdmittedGeneratedHostContractV2<K>, GeneratedHostContractErrorV2> {
    if production.kernel_binding_identity != K::KERNEL_BINDING_ID_V1
        || descriptor.kernel_id().as_bytes() != &K::KERNEL_BINDING_ID_V1
    {
        return Err(GeneratedHostContractErrorV2::KernelSubstitution);
    }
    if production.generated_host_contract_identity != K::PROFILE.generated_host_contract_identity()
    {
        return Err(GeneratedHostContractErrorV2::GeneratedContractSubstitution);
    }
    if production.descriptor_target.as_ref() != table.device_target().to_string() {
        return Err(GeneratedHostContractErrorV2::DescriptorTargetMismatch);
    }
    let Some(ordinal_descriptor) = table.kernels().get(production.kernel_ordinal) else {
        return Err(GeneratedHostContractErrorV2::KernelOrdinal);
    };
    if ordinal_descriptor != descriptor {
        return Err(GeneratedHostContractErrorV2::KernelOrdinal);
    }
    if table.canonical_code_object_digest() != production.artifact_identity
        || DeviceDescriptorTableDigest::calculate(table)
            .map_err(|_| GeneratedHostContractErrorV2::DescriptorIdentity)?
            != production.descriptor_table_identity
        || KernelDescriptorDigest::calculate(descriptor) != production.descriptor_identity
        || descriptor.entry_name().as_str() != production.entry_name.as_ref()
    {
        return Err(GeneratedHostContractErrorV2::DescriptorIdentity);
    }

    let generated =
        K::generated_host_contract_v2().map_err(GeneratedHostContractErrorV2::GeneratedLayout)?;
    let packing = validate_worker_v3_argument_packing(table, descriptor, generated.arguments())
        .map_err(GeneratedHostContractErrorV2::DescriptorAbi)?;
    validate_launch_contract(generated.launch(), descriptor.launch())?;
    let module = decode_module_v13(production.static_association.final_graph.canonical_bytes())
        .map_err(|_| GeneratedHostContractErrorV2::InvalidCanonicalKirV13)?;
    let memory = match production.memory.take() {
        Some(memory) => memory,
        None => {
            derive_canonical_memory_constraints_v2(&module, production.kernel_ordinal, &packing)?
        }
    };
    validate_memory_constraints(&packing, &memory)?;
    let abi_proof = validate_exact_v13_abi::<K>(
        &module,
        production.kernel_ordinal,
        &packing,
        production.static_association.subject,
    )?;

    Ok(AdmittedGeneratedHostContractV2 {
        packing,
        kernel_ordinal: production.kernel_ordinal,
        artifact_identity: production.artifact_identity,
        descriptor_table_identity: production.descriptor_table_identity,
        descriptor_identity: production.descriptor_identity,
        entry_name: production.entry_name,
        static_association: Arc::new(production.static_association),
        abi_proof,
        memory,
        _marker: PhantomData,
    })
}

fn derive_canonical_memory_constraints_v2(
    module: &Module,
    kernel_ordinal: usize,
    packing: &GeneratedArgumentPackingPlanV1,
) -> Result<Box<[GeneratedHostMemoryConstraintV2]>, GeneratedHostContractErrorV2> {
    let kernel = module
        .kernels
        .get(kernel_ordinal)
        .ok_or(GeneratedHostContractErrorV2::KernelOrdinal)?;
    let entry = module
        .function(&kernel.entry)
        .ok_or(GeneratedHostContractErrorV2::V13KernelEntry)?;
    let mut physical_argument = 0_usize;
    let mut constraints = Vec::new();
    for ty in &entry.signature.parameters {
        if matches!(
            ty,
            KirType::KernelContext(_) | KirType::ExecutionCapability(_)
        ) {
            continue;
        }
        let field = packing
            .argument(physical_argument)
            .ok_or(GeneratedHostContractErrorV2::V13PhysicalAbi)?;
        if let AbiKind::Slice {
            element_size,
            element_alignment,
        } = field.kind()
        {
            let role = canonical_kir_memory_role_v2(ty)
                .ok_or(GeneratedHostContractErrorV2::V13PhysicalAbi)?;
            let maximum_extent = u64::MAX / element_size;
            constraints.push(GeneratedHostMemoryConstraintV2::new(
                physical_argument,
                role,
                element_size,
                u64::from(element_alignment),
                [GeneratedHostAxisConstraintV2::new(0, maximum_extent, 1, 1)?],
            )?);
        }
        physical_argument = physical_argument
            .checked_add(1)
            .ok_or(GeneratedHostContractErrorV2::V13AbiOverflow)?;
    }
    if physical_argument != packing.argument_count()
        || constraints.len() > MAX_GENERATED_HOST_MEMORY_ARGUMENTS_V2
    {
        return Err(GeneratedHostContractErrorV2::V13PhysicalAbi);
    }
    Ok(constraints.into_boxed_slice())
}

fn canonical_kir_memory_role_v2(ty: &KirType) -> Option<GeneratedHostMemoryRoleV2> {
    let access = match ty {
        KirType::Slice(slice) => slice.access,
        KirType::GlobalCapability(capability) => capability.role().access(),
        _ => return None,
    };
    Some(match access {
        KirAccessMode::ReadOnly => GeneratedHostMemoryRoleV2::Input,
        KirAccessMode::WriteOnly => GeneratedHostMemoryRoleV2::Output,
        KirAccessMode::ReadWrite => GeneratedHostMemoryRoleV2::InputOutput,
    })
}

fn validate_exact_v13_abi<K: CompilerGeneratedKernelExpectationV2>(
    module: &Module,
    kernel_ordinal: usize,
    packing: &GeneratedArgumentPackingPlanV1,
    subject: GeneratedHostExecutionSubjectV2,
) -> Result<GeneratedHostV13AbiProofV2, GeneratedHostContractErrorV2> {
    let kernel = module
        .kernels
        .get(kernel_ordinal)
        .ok_or(GeneratedHostContractErrorV2::KernelOrdinal)?;
    let entry = module
        .function(&kernel.entry)
        .ok_or(GeneratedHostContractErrorV2::V13KernelEntry)?;
    if entry.role != FunctionRole::KernelEntry {
        return Err(GeneratedHostContractErrorV2::V13KernelEntry);
    }
    validate_v13_context::<K>(entry, subject)?;

    let mut physical = Vec::new();
    let mut zero_byte_logical_capabilities = 0_usize;
    for (kir_parameter, ty) in entry.signature.parameters.iter().enumerate() {
        if matches!(
            ty,
            KirType::KernelContext(_) | KirType::ExecutionCapability(_)
        ) {
            zero_byte_logical_capabilities = zero_byte_logical_capabilities
                .checked_add(1)
                .ok_or(GeneratedHostContractErrorV2::V13AbiOverflow)?;
            continue;
        }
        let physical_argument = physical.len();
        let field = packing
            .argument(physical_argument)
            .ok_or(GeneratedHostContractErrorV2::V13PhysicalAbi)?;
        validate_v13_physical_type(ty, field, packing.pointer_width())?;
        physical.push(GeneratedHostV13PhysicalParameterV2 {
            kir_parameter,
            physical_argument,
            offset: field.offset(),
            size: field.size(),
            alignment: field.alignment(),
        });
    }
    if physical.len() != packing.argument_count() {
        return Err(GeneratedHostContractErrorV2::V13PhysicalAbi);
    }

    let body = entry
        .body
        .as_ref()
        .ok_or(GeneratedHostContractErrorV2::V13KernelEntry)?;
    for ty in body.blocks.iter().flat_map(|block| {
        block.parameters.iter().map(|value| &value.ty).chain(
            block
                .operations
                .iter()
                .flat_map(|operation| operation.results.iter().map(|value| &value.ty)),
        )
    }) {
        if matches!(
            ty,
            KirType::KernelContext(_) | KirType::ExecutionCapability(_)
        ) {
            zero_byte_logical_capabilities = zero_byte_logical_capabilities
                .checked_add(1)
                .ok_or(GeneratedHostContractErrorV2::V13AbiOverflow)?;
        }
    }
    if zero_byte_logical_capabilities == 0 {
        return Err(GeneratedHostContractErrorV2::MissingV13LogicalCapability);
    }
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/HOST/V13-ZERO-BYTE-CAPABILITY-ABI/V2\0");
    digest.update(
        u64::try_from(zero_byte_logical_capabilities)
            .map_err(|_| GeneratedHostContractErrorV2::V13AbiOverflow)?
            .to_le_bytes(),
    );
    digest.update(
        u64::try_from(physical.len())
            .map_err(|_| GeneratedHostContractErrorV2::V13AbiOverflow)?
            .to_le_bytes(),
    );
    for parameter in &physical {
        digest.update(
            u64::try_from(parameter.kir_parameter)
                .map_err(|_| GeneratedHostContractErrorV2::V13AbiOverflow)?
                .to_le_bytes(),
        );
        digest.update(
            u64::try_from(parameter.physical_argument)
                .map_err(|_| GeneratedHostContractErrorV2::V13AbiOverflow)?
                .to_le_bytes(),
        );
        digest.update(parameter.offset.to_le_bytes());
        digest.update(parameter.size.to_le_bytes());
        digest.update(parameter.alignment.to_le_bytes());
    }
    Ok(GeneratedHostV13AbiProofV2 {
        zero_byte_logical_capabilities,
        physical_parameters: physical.into_boxed_slice(),
        identity: digest.finalize().into(),
    })
}

fn validate_v13_context<K: CompilerGeneratedKernelExpectationV2>(
    entry: &Function,
    subject: GeneratedHostExecutionSubjectV2,
) -> Result<(), GeneratedHostContractErrorV2> {
    let mut contexts = entry
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(
            |operation| match (&operation.kind, operation.results.as_slice()) {
                (OperationKind::KernelContextIssue(_), [result]) => match &result.ty {
                    KirType::KernelContext(context) => Some(context),
                    _ => None,
                },
                _ => None,
            },
        );
    let context = contexts
        .next()
        .ok_or(GeneratedHostContractErrorV2::MissingV13LogicalCapability)?;
    if contexts.next().is_some()
        || context.root() != &entry.id
        || context.kernel_marker() != &K::KERNEL_BINDING_ID_V1
        || context.target() != &subject.target_model_identity
        || context.launch() != &subject.launch_contract_identity
    {
        return Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch);
    }
    Ok(())
}

fn validate_v13_physical_type(
    ty: &KirType,
    field: &AbiField,
    pointer_width: PointerWidth,
) -> Result<(), GeneratedHostContractErrorV2> {
    let expected = match ty {
        KirType::Scalar(scalar) => AbiKind::Scalar(kir_scalar_abi(*scalar, pointer_width)?),
        KirType::Pointer(pointer) => AbiKind::Pointer {
            pointee_size: kir_type_bytes(&pointer.pointee, pointer_width)?,
            pointee_alignment: u32::try_from(kir_type_bytes(&pointer.pointee, pointer_width)?)
                .map_err(|_| GeneratedHostContractErrorV2::V13PhysicalAbi)?,
        },
        KirType::Slice(slice) => AbiKind::Slice {
            element_size: kir_type_bytes(&slice.element, pointer_width)?,
            element_alignment: u32::try_from(kir_type_bytes(&slice.element, pointer_width)?)
                .map_err(|_| GeneratedHostContractErrorV2::V13PhysicalAbi)?,
        },
        KirType::GlobalCapability(capability) => AbiKind::Slice {
            element_size: kir_type_bytes(capability.element(), pointer_width)?,
            element_alignment: u32::try_from(kir_type_bytes(capability.element(), pointer_width)?)
                .map_err(|_| GeneratedHostContractErrorV2::V13PhysicalAbi)?,
        },
        KirType::Unit | KirType::KernelContext(_) | KirType::ExecutionCapability(_) => {
            return Err(GeneratedHostContractErrorV2::V13PhysicalAbi);
        }
    };
    if field.kind() != expected {
        return Err(GeneratedHostContractErrorV2::V13PhysicalAbi);
    }
    let expected_access = match ty {
        KirType::Pointer(pointer) => kir_access(pointer.access),
        KirType::Slice(slice) => kir_access(slice.access),
        KirType::GlobalCapability(capability) => kir_access(capability.role().access()),
        KirType::Scalar(_) => Access::ByValue,
        _ => return Err(GeneratedHostContractErrorV2::V13PhysicalAbi),
    };
    let expected_space = match ty {
        KirType::Pointer(pointer) => kir_address_space(pointer.address_space),
        KirType::Slice(slice) => kir_address_space(slice.address_space),
        KirType::GlobalCapability(_) => AbiAddressSpace::Global,
        KirType::Scalar(_) => AbiAddressSpace::Value,
        _ => return Err(GeneratedHostContractErrorV2::V13PhysicalAbi),
    };
    if field.access() != expected_access || field.address_space() != expected_space {
        return Err(GeneratedHostContractErrorV2::V13PhysicalAbi);
    }
    Ok(())
}

fn kir_scalar_abi(
    scalar: KirScalarType,
    pointer_width: PointerWidth,
) -> Result<AbiScalarType, GeneratedHostContractErrorV2> {
    Ok(match scalar {
        KirScalarType::I8 => AbiScalarType::I8,
        KirScalarType::U8 => AbiScalarType::U8,
        KirScalarType::I16 => AbiScalarType::I16,
        KirScalarType::U16 => AbiScalarType::U16,
        KirScalarType::I32 => AbiScalarType::I32,
        KirScalarType::U32 => AbiScalarType::U32,
        KirScalarType::I64 => AbiScalarType::I64,
        KirScalarType::U64 => AbiScalarType::U64,
        KirScalarType::Index if pointer_width == PointerWidth::Bits32 => AbiScalarType::U32,
        KirScalarType::Index if pointer_width == PointerWidth::Bits64 => AbiScalarType::U64,
        KirScalarType::F16 => AbiScalarType::F16,
        KirScalarType::F32 => AbiScalarType::F32,
        KirScalarType::F64 => AbiScalarType::F64,
        KirScalarType::Bool
        | KirScalarType::I128
        | KirScalarType::U128
        | KirScalarType::Bf16
        | KirScalarType::Index => return Err(GeneratedHostContractErrorV2::V13PhysicalAbi),
    })
}

fn kir_type_bytes(
    ty: &KirType,
    pointer_width: PointerWidth,
) -> Result<u64, GeneratedHostContractErrorV2> {
    match ty {
        KirType::Scalar(scalar) => Ok(match kir_scalar_abi(*scalar, pointer_width)? {
            AbiScalarType::I8 | AbiScalarType::U8 => 1,
            AbiScalarType::I16 | AbiScalarType::U16 | AbiScalarType::F16 => 2,
            AbiScalarType::I32 | AbiScalarType::U32 | AbiScalarType::F32 => 4,
            AbiScalarType::I64 | AbiScalarType::U64 | AbiScalarType::F64 => 8,
        }),
        _ => Err(GeneratedHostContractErrorV2::V13PhysicalAbi),
    }
}

const fn kir_access(access: KirAccessMode) -> Access {
    match access {
        KirAccessMode::ReadOnly => Access::ReadOnly,
        KirAccessMode::WriteOnly => Access::WriteOnly,
        KirAccessMode::ReadWrite => Access::ReadWrite,
    }
}

const fn kir_address_space(address_space: KirAddressSpace) -> AbiAddressSpace {
    match address_space {
        KirAddressSpace::Private => AbiAddressSpace::Private,
        KirAddressSpace::Workgroup => AbiAddressSpace::Workgroup,
        KirAddressSpace::Global => AbiAddressSpace::Global,
        KirAddressSpace::Constant => AbiAddressSpace::Constant,
        KirAddressSpace::Generic => AbiAddressSpace::Generic,
    }
}

fn validate_launch_contract(
    generated: &LaunchContract,
    descriptor: &fe2o3_kernel_descriptor::LaunchConstraintsV1,
) -> Result<(), GeneratedHostContractErrorV2> {
    let generated_grid = generated.max_grid();
    let descriptor_grid = descriptor.max_grid();
    if generated.rank() != descriptor.rank()
        || [generated_grid.x(), generated_grid.y(), generated_grid.z()]
            != [
                descriptor_grid.x(),
                descriptor_grid.y(),
                descriptor_grid.z(),
            ]
        || generated.static_shared_memory_bytes() != descriptor.static_shared_memory_bytes()
        || generated.max_dynamic_shared_memory_bytes()
            != descriptor.max_dynamic_shared_memory_bytes()
    {
        return Err(GeneratedHostContractErrorV2::DescriptorLaunchMismatch);
    }
    let block_matches = match (generated.block_size(), descriptor.block_size()) {
        (BlockSize::Any, BlockSizeV1::Any) => true,
        (BlockSize::Exact(left), BlockSizeV1::Exact(right))
        | (BlockSize::AtMost(left), BlockSizeV1::AtMost(right)) => {
            [left.x(), left.y(), left.z()] == [right.x(), right.y(), right.z()]
        }
        _ => false,
    };
    if !block_matches {
        return Err(GeneratedHostContractErrorV2::DescriptorLaunchMismatch);
    }
    Ok(())
}

fn validate_memory_constraints(
    packing: &GeneratedArgumentPackingPlanV1,
    constraints: &[GeneratedHostMemoryConstraintV2],
) -> Result<(), GeneratedHostContractErrorV2> {
    let memory_count = (0..packing.argument_count())
        .filter(|index| {
            packing
                .argument(*index)
                .is_some_and(|field| matches!(field.kind(), AbiKind::Slice { .. }))
        })
        .count();
    if constraints.len() != memory_count {
        return Err(GeneratedHostContractErrorV2::MemoryConstraintCount);
    }
    for constraint in constraints {
        let Some(field) = packing.argument(constraint.argument_index) else {
            return Err(GeneratedHostContractErrorV2::MemoryConstraintArgument {
                argument_index: constraint.argument_index,
            });
        };
        let AbiKind::Slice {
            element_size,
            element_alignment,
        } = field.kind()
        else {
            return Err(GeneratedHostContractErrorV2::MemoryConstraintArgument {
                argument_index: constraint.argument_index,
            });
        };
        if field.access() != constraint.role.expected_access()
            || field.alias_class() != constraint.role.expected_alias()
        {
            return Err(GeneratedHostContractErrorV2::MemoryRoleMismatch {
                argument_index: constraint.argument_index,
            });
        }
        if element_size != constraint.element_bytes {
            return Err(GeneratedHostContractErrorV2::MemoryElementWidthMismatch {
                argument_index: constraint.argument_index,
                expected: element_size,
                observed: constraint.element_bytes,
            });
        }
        let element_alignment = u64::from(element_alignment);
        if constraint.required_alignment < element_alignment
            || !constraint
                .required_alignment
                .is_multiple_of(element_alignment)
        {
            return Err(GeneratedHostContractErrorV2::MemoryAlignmentMismatch {
                argument_index: constraint.argument_index,
            });
        }
    }
    Ok(())
}

/// Runtime-observed identity of one selected target, context, and stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedHostRuntimeCoordinatesV2 {
    target_model_identity: [u8; 32],
    context_identity: [u8; 32],
    stream_identity: [u8; 32],
}

impl GeneratedHostRuntimeCoordinatesV2 {
    pub fn new(
        target_model_identity: [u8; 32],
        context_identity: [u8; 32],
        stream_identity: [u8; 32],
    ) -> Result<Self, GeneratedHostContractErrorV2> {
        if target_model_identity == [0; 32]
            || context_identity == [0; 32]
            || stream_identity == [0; 32]
        {
            return Err(GeneratedHostContractErrorV2::InvalidRuntimeCoordinates);
        }
        Ok(Self {
            target_model_identity,
            context_identity,
            stream_identity,
        })
    }

    pub const fn target_model_identity(self) -> [u8; 32] {
        self.target_model_identity
    }

    pub const fn context_identity(self) -> [u8; 32] {
        self.context_identity
    }

    pub const fn stream_identity(self) -> [u8; 32] {
        self.stream_identity
    }
}

/// Per-dispatch evidence coordinates checked against static custody and runtime observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedHostDispatchEvidenceV2 {
    runtime: GeneratedHostRuntimeCoordinatesV2,
    subject: GeneratedHostExecutionSubjectV2,
    capability_closure_identity: [u8; 32],
    dynamic_precondition_roster_identity: [u8; 32],
    compiler_policy_identity: [u8; 32],
    static_association_identity: [u8; 32],
    artifact_identity: CanonicalCodeObjectDigest,
    descriptor_table_identity: DeviceDescriptorTableDigest,
    descriptor_identity: KernelDescriptorDigest,
    kernel_ordinal: usize,
}

impl GeneratedHostDispatchEvidenceV2 {
    pub const fn runtime(self) -> GeneratedHostRuntimeCoordinatesV2 {
        self.runtime
    }
}

/// Backend-neutral launch dimensions checked before reviewed submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedHostLaunchGeometryV2 {
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_lds_bytes: u32,
}

impl GeneratedHostLaunchGeometryV2 {
    pub const fn new(grid: [u32; 3], workgroup: [u32; 3], dynamic_lds_bytes: u32) -> Self {
        Self {
            grid,
            workgroup,
            dynamic_lds_bytes,
        }
    }

    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    pub const fn dynamic_lds_bytes(self) -> u32 {
        self.dynamic_lds_bytes
    }
}

/// Address-free dynamic view derived from an argument that retains its original borrow.
#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct GeneratedHostMemoryBindingV2 {
    argument_index: usize,
    address: usize,
    allocation_elements: u64,
    element_bytes: u64,
    role: GeneratedHostMemoryRoleV2,
    alias: AliasClass,
    extents: Box<[u64]>,
    strides: Box<[u64]>,
}

impl GeneratedHostMemoryBindingV2 {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_compiler_generated_argument_v2(
        argument_index: usize,
        address: usize,
        allocation_elements: u64,
        element_bytes: u64,
        role: GeneratedHostMemoryRoleV2,
        alias: AliasClass,
        extents: impl Into<Vec<u64>>,
        strides: impl Into<Vec<u64>>,
    ) -> Self {
        Self {
            argument_index,
            address,
            allocation_elements,
            element_bytes,
            role,
            alias,
            extents: extents.into().into_boxed_slice(),
            strides: strides.into().into_boxed_slice(),
        }
    }
}

/// Memory-argument observation supplied by a generated argument field.
///
/// # Safety
///
/// The observation must describe the exact allocation borrow retained by `self`, including its
/// address, element extent, role, alias class, dimensions, and strides.
#[doc(hidden)]
pub unsafe trait GeneratedHostMemoryArgumentV2<'allocation>: 'allocation {
    fn generated_host_memory_binding_v2(
        &self,
        argument_index: usize,
    ) -> GeneratedHostMemoryBindingV2;
}

/// Complete dynamic memory observations for one generated `Arguments` value.
///
/// # Safety
///
/// Implementations must be compiler-generated and must visit every memory field exactly once.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedHostArgumentsV2<
    'allocation,
    K: CompilerGeneratedKernelExpectationV2,
>
{
    fn generated_host_memory_bindings_v2(&self) -> Vec<GeneratedHostMemoryBindingV2>;
}

/// Reviewed asynchronous backend transition used after all V2 preparation checks succeed.
///
/// # Safety
///
/// `observe_runtime_v2` must be side-effect free and identify the exact supplied context/stream.
/// `submit_v2` may publish GPU work only for those objects and must return `Err` only before
/// publication or after quiescence. `poll_v2` must not unwind and may return a terminal status only
/// after all effects are quiescent. `quiesce_v2` must not return until it can report a terminal
/// status. Implementations must not retain references after return.
pub unsafe trait ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments> {
    type Submission;
    type SubmitError;

    fn observe_runtime_v2(
        &self,
        context: &Context,
        stream: &Stream,
    ) -> GeneratedHostRuntimeCoordinatesV2;

    fn submit_v2(
        &mut self,
        context: &mut Context,
        stream: &Stream,
        geometry: GeneratedHostLaunchGeometryV2,
        arguments: &mut Arguments,
    ) -> Result<Self::Submission, Self::SubmitError>;

    fn poll_v2(
        &mut self,
        context: &mut Context,
        stream: &Stream,
        submission: &mut Self::Submission,
    ) -> ReviewedGeneratedHostAsyncStatusV2;

    fn quiesce_v2(
        &mut self,
        context: &mut Context,
        stream: &Stream,
        submission: &mut Self::Submission,
    ) -> ReviewedGeneratedHostAsyncStatusV2;
}

/// Conclusive status contract returned by a reviewed asynchronous backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewedGeneratedHostAsyncStatusV2 {
    Pending,
    Succeeded,
    Failed(i64),
}

/// Authority-free, move-only result of the complete generated-host pre-dispatch join.
///
/// The exact observations remain private so this value cannot be edited into agreement. Runtime
/// backends may retain it alongside their existing prepared authority, but it never grants load or
/// dispatch authority itself.
#[derive(Debug)]
#[must_use = "checked generated-host facts grant no authority and must remain with the invocation"]
pub(crate) struct CheckedGeneratedHostDispatchV2<K> {
    packing: GeneratedArgumentPackingPlanV1,
    runtime: GeneratedHostRuntimeCoordinatesV2,
    kernel_ordinal: usize,
    artifact_identity: CanonicalCodeObjectDigest,
    descriptor_table_identity: DeviceDescriptorTableDigest,
    descriptor_identity: KernelDescriptorDigest,
    entry_name: Box<str>,
    static_association: Arc<ProductionV13StaticAssociationV2>,
    geometry: GeneratedHostLaunchGeometryV2,
    memory: Box<[GeneratedHostMemoryBindingV2]>,
    _marker: PhantomData<fn() -> K>,
}

impl<K> CheckedGeneratedHostDispatchV2<K> {
    pub(crate) const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        &self.packing
    }

    pub(crate) const fn runtime(&self) -> GeneratedHostRuntimeCoordinatesV2 {
        self.runtime
    }

    pub(crate) fn subject(&self) -> GeneratedHostExecutionSubjectV2 {
        self.static_association.subject
    }

    pub(crate) const fn geometry(&self) -> GeneratedHostLaunchGeometryV2 {
        self.geometry
    }

    pub(crate) fn capability_closure_identity(&self) -> [u8; 32] {
        self.static_association.capability_closure_identity
    }

    pub(crate) fn capability_association_identity(&self) -> ([u8; 32], u64) {
        (
            self.static_association.capability_association_identity,
            self.static_association.capability_association_bytes,
        )
    }

    pub(crate) fn dynamic_precondition_roster_identity(&self) -> [u8; 32] {
        self.static_association
            .dynamic_precondition_roster_identity
    }

    pub(crate) fn compiler_policy_identity(&self) -> [u8; 32] {
        self.static_association.compiler_policy_identity
    }

    pub(crate) fn memory_bindings(&self) -> &[GeneratedHostMemoryBindingV2] {
        &self.memory
    }

    pub(crate) const fn kernel_ordinal(&self) -> usize {
        self.kernel_ordinal
    }

    pub(crate) const fn artifact_identity(&self) -> CanonicalCodeObjectDigest {
        self.artifact_identity
    }

    pub(crate) const fn descriptor_table_identity(&self) -> DeviceDescriptorTableDigest {
        self.descriptor_table_identity
    }

    pub(crate) const fn descriptor_identity(&self) -> KernelDescriptorDigest {
        self.descriptor_identity
    }

    pub(crate) fn entry_name(&self) -> &str {
        &self.entry_name
    }

    pub(crate) fn static_association_identity(&self) -> [u8; 32] {
        self.static_association.identity
    }
}

/// Fully checked invocation before the first operation permitted to publish GPU work.
#[must_use = "a prepared generated invocation does no work until submitted"]
pub struct PreparedGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments> {
    backend: &'runtime mut Backend,
    context: &'runtime mut Context,
    stream: &'runtime Stream,
    arguments: Arguments,
    checked: CheckedGeneratedHostDispatchV2<K>,
}

impl<K> AdmittedGeneratedHostContractV2<K>
where
    K: CompilerGeneratedKernelExpectationV2,
{
    /// Rejects caller-supplied launch and memory values before a target provider is opened.
    ///
    /// Runtime identity is deliberately checked later, against coordinates minted by the opened
    /// provider. This first pass exists so malformed dimensions, extents, alignments, or aliases
    /// cannot trigger device discovery, allocation, module loading, or submission.
    pub(crate) fn preflight_dynamic_v2<'allocation, Arguments>(
        &self,
        geometry: GeneratedHostLaunchGeometryV2,
        arguments: &Arguments,
    ) -> Result<(), GeneratedHostPrepareErrorV2>
    where
        Arguments: CompilerGeneratedHostArgumentsV2<'allocation, K> + 'allocation,
    {
        validate_geometry(K::generated_host_contract_v2()?.launch(), geometry)?;
        let _ =
            validate_dynamic_memory(&self.memory, arguments.generated_host_memory_bindings_v2())?;
        Ok(())
    }

    /// Checks every static and dynamic W7 condition without entering the submission backend.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare<'runtime, 'allocation, Backend, Context, Stream, Arguments>(
        &self,
        backend: &'runtime mut Backend,
        context: &'runtime mut Context,
        stream: &'runtime Stream,
        evidence: GeneratedHostDispatchEvidenceV2,
        geometry: GeneratedHostLaunchGeometryV2,
        arguments: Arguments,
    ) -> Result<
        PreparedGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments>,
        GeneratedHostPrepareErrorV2,
    >
    where
        Backend: ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>,
        Arguments: CompilerGeneratedHostArgumentsV2<'allocation, K> + 'allocation,
    {
        let observed = backend.observe_runtime_v2(context, stream);
        let checked = self.check_dispatch_v2(evidence, observed, geometry, &arguments)?;
        Ok(PreparedGeneratedHostInvocationV2 {
            backend,
            context,
            stream,
            arguments,
            checked,
        })
    }

    pub(crate) fn check_dispatch_v2<'allocation, Arguments>(
        &self,
        evidence: GeneratedHostDispatchEvidenceV2,
        observed: GeneratedHostRuntimeCoordinatesV2,
        geometry: GeneratedHostLaunchGeometryV2,
        arguments: &Arguments,
    ) -> Result<CheckedGeneratedHostDispatchV2<K>, GeneratedHostPrepareErrorV2>
    where
        Arguments: CompilerGeneratedHostArgumentsV2<'allocation, K> + 'allocation,
    {
        validate_dispatch_coordinates(self, evidence, observed)?;
        validate_geometry(K::generated_host_contract_v2()?.launch(), geometry)?;
        let memory =
            validate_dynamic_memory(&self.memory, arguments.generated_host_memory_bindings_v2())?;
        Ok(CheckedGeneratedHostDispatchV2 {
            packing: self.packing.clone(),
            runtime: observed,
            kernel_ordinal: self.kernel_ordinal,
            artifact_identity: self.artifact_identity,
            descriptor_table_identity: self.descriptor_table_identity,
            descriptor_identity: self.descriptor_identity,
            entry_name: self.entry_name.clone(),
            static_association: Arc::clone(&self.static_association),
            geometry,
            memory,
            _marker: PhantomData,
        })
    }
}

fn validate_dispatch_coordinates<K>(
    admitted: &AdmittedGeneratedHostContractV2<K>,
    evidence: GeneratedHostDispatchEvidenceV2,
    observed: GeneratedHostRuntimeCoordinatesV2,
) -> Result<(), GeneratedHostPrepareErrorV2> {
    if evidence.runtime.target_model_identity != admitted.subject().target_model_identity {
        return Err(GeneratedHostPrepareErrorV2::TargetMismatch);
    }
    if evidence.runtime.target_model_identity != observed.target_model_identity {
        return Err(GeneratedHostPrepareErrorV2::TargetMismatch);
    }
    if evidence.runtime.context_identity != observed.context_identity {
        return Err(GeneratedHostPrepareErrorV2::ContextMismatch);
    }
    if evidence.runtime.stream_identity != observed.stream_identity {
        return Err(GeneratedHostPrepareErrorV2::StreamMismatch);
    }
    if evidence.subject != admitted.subject() {
        return Err(GeneratedHostPrepareErrorV2::SubjectMismatch);
    }
    if evidence.capability_closure_identity != admitted.capability_closure_identity() {
        return Err(GeneratedHostPrepareErrorV2::CapabilityClosureMismatch);
    }
    if evidence.dynamic_precondition_roster_identity
        != admitted.dynamic_precondition_roster_identity()
    {
        return Err(GeneratedHostPrepareErrorV2::DynamicPreconditionRosterMismatch);
    }
    if evidence.compiler_policy_identity != admitted.compiler_policy_identity() {
        return Err(GeneratedHostPrepareErrorV2::CompilerPolicyMismatch);
    }
    if evidence.static_association_identity != admitted.static_association.identity
        || evidence.artifact_identity != admitted.artifact_identity
        || evidence.descriptor_table_identity != admitted.descriptor_table_identity
        || evidence.descriptor_identity != admitted.descriptor_identity
        || evidence.kernel_ordinal != admitted.kernel_ordinal
    {
        return Err(GeneratedHostPrepareErrorV2::StaticAssociationMismatch);
    }
    Ok(())
}

fn validate_geometry(
    contract: &LaunchContract,
    geometry: GeneratedHostLaunchGeometryV2,
) -> Result<(), GeneratedHostPrepareErrorV2> {
    if geometry.grid.contains(&0) || geometry.workgroup.contains(&0) {
        return Err(GeneratedHostPrepareErrorV2::LaunchGeometry);
    }
    let rank = usize::from(contract.rank());
    if geometry.grid[rank..] != [1; 3][rank..] || geometry.workgroup[rank..] != [1; 3][rank..] {
        return Err(GeneratedHostPrepareErrorV2::LaunchGeometry);
    }
    let maximum = contract.max_grid();
    if geometry.grid[0] > maximum.x()
        || geometry.grid[1] > maximum.y()
        || geometry.grid[2] > maximum.z()
    {
        return Err(GeneratedHostPrepareErrorV2::LaunchGeometry);
    }
    let block_matches = match contract.block_size() {
        BlockSize::Any => true,
        BlockSize::Exact(expected) => {
            geometry.workgroup == [expected.x(), expected.y(), expected.z()]
        }
        BlockSize::AtMost(maximum) => {
            geometry.workgroup[0] <= maximum.x()
                && geometry.workgroup[1] <= maximum.y()
                && geometry.workgroup[2] <= maximum.z()
        }
    };
    if !block_matches
        || geometry.dynamic_lds_bytes > contract.max_dynamic_shared_memory_bytes()
        || contract
            .static_shared_memory_bytes()
            .checked_add(geometry.dynamic_lds_bytes)
            .is_none()
    {
        return Err(GeneratedHostPrepareErrorV2::LaunchGeometry);
    }
    Ok(())
}

fn validate_dynamic_memory(
    constraints: &[GeneratedHostMemoryConstraintV2],
    mut bindings: Vec<GeneratedHostMemoryBindingV2>,
) -> Result<Box<[GeneratedHostMemoryBindingV2]>, GeneratedHostPrepareErrorV2> {
    bindings.sort_unstable_by_key(|binding| binding.argument_index);
    if bindings.len() != constraints.len()
        || bindings
            .windows(2)
            .any(|pair| pair[0].argument_index == pair[1].argument_index)
    {
        return Err(GeneratedHostPrepareErrorV2::MemoryBindingCount);
    }
    for (constraint, binding) in constraints.iter().zip(&bindings) {
        let index = constraint.argument_index;
        if binding.argument_index != index {
            return Err(GeneratedHostPrepareErrorV2::MemoryBindingIndex {
                argument_index: index,
            });
        }
        if binding.role != constraint.role || binding.alias != constraint.role.expected_alias() {
            return Err(GeneratedHostPrepareErrorV2::MemoryRole {
                argument_index: index,
            });
        }
        if binding.element_bytes != constraint.element_bytes {
            return Err(GeneratedHostPrepareErrorV2::ElementWidth {
                argument_index: index,
                expected: constraint.element_bytes,
                observed: binding.element_bytes,
            });
        }
        if binding.extents.len() != constraint.axes.len()
            || binding.strides.len() != constraint.axes.len()
        {
            return Err(GeneratedHostPrepareErrorV2::Dimensions {
                argument_index: index,
            });
        }
        let mut required_elements = u64::from(!binding.extents.contains(&0));
        for (axis, ((extent, stride), expected)) in binding
            .extents
            .iter()
            .zip(&binding.strides)
            .zip(&constraint.axes)
            .enumerate()
        {
            if !(expected.minimum_extent..=expected.maximum_extent).contains(extent) {
                return Err(GeneratedHostPrepareErrorV2::Extent {
                    argument_index: index,
                    axis,
                });
            }
            if !(expected.minimum_stride..=expected.maximum_stride).contains(stride) {
                return Err(GeneratedHostPrepareErrorV2::Stride {
                    argument_index: index,
                    axis,
                });
            }
            if required_elements != 0 {
                let trailing_elements =
                    extent
                        .checked_sub(1)
                        .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                            argument_index: index,
                        })?;
                required_elements = required_elements
                    .checked_add(trailing_elements.checked_mul(*stride).ok_or(
                        GeneratedHostPrepareErrorV2::AllocationExtent {
                            argument_index: index,
                        },
                    )?)
                    .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                        argument_index: index,
                    })?;
            }
        }
        let required_bytes = required_elements
            .checked_mul(constraint.element_bytes)
            .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                argument_index: index,
            })?;
        let allocation_bytes = binding
            .allocation_elements
            .checked_mul(constraint.element_bytes)
            .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                argument_index: index,
            })?;
        if required_elements > binding.allocation_elements || required_bytes > allocation_bytes {
            return Err(GeneratedHostPrepareErrorV2::AllocationExtent {
                argument_index: index,
            });
        }
        if !binding
            .address
            .is_multiple_of(usize::try_from(constraint.required_alignment).unwrap_or(usize::MAX))
        {
            return Err(GeneratedHostPrepareErrorV2::Alignment {
                argument_index: index,
            });
        }
    }
    validate_aliases(&bindings)?;
    Ok(bindings.into_boxed_slice())
}

fn validate_aliases(
    bindings: &[GeneratedHostMemoryBindingV2],
) -> Result<(), GeneratedHostPrepareErrorV2> {
    for (left_index, left) in bindings.iter().enumerate() {
        let left_bytes = left
            .allocation_elements
            .checked_mul(left.element_bytes)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .and_then(|bytes| left.address.checked_add(bytes))
            .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                argument_index: left.argument_index,
            })?;
        for right in &bindings[left_index + 1..] {
            let right_bytes = right
                .allocation_elements
                .checked_mul(right.element_bytes)
                .and_then(|bytes| usize::try_from(bytes).ok())
                .and_then(|bytes| right.address.checked_add(bytes))
                .ok_or(GeneratedHostPrepareErrorV2::AllocationExtent {
                    argument_index: right.argument_index,
                })?;
            let overlaps = left.address < right_bytes && right.address < left_bytes;
            if overlaps
                && (left.alias == AliasClass::Exclusive || right.alias == AliasClass::Exclusive)
            {
                return Err(GeneratedHostPrepareErrorV2::AliasConflict {
                    left: left.argument_index,
                    right: right.argument_index,
                });
            }
        }
    }
    Ok(())
}

impl<'runtime, K, Backend, Context, Stream, Arguments>
    PreparedGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments>
where
    Backend: ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>,
{
    pub const fn geometry(&self) -> GeneratedHostLaunchGeometryV2 {
        self.checked.geometry()
    }

    pub const fn runtime_coordinates(&self) -> GeneratedHostRuntimeCoordinatesV2 {
        self.checked.runtime()
    }

    pub fn subject(&self) -> GeneratedHostExecutionSubjectV2 {
        self.checked.subject()
    }

    pub fn capability_association_identity(&self) -> ([u8; 32], u64) {
        self.checked.capability_association_identity()
    }

    pub fn dynamic_precondition_roster_identity(&self) -> [u8; 32] {
        self.checked.dynamic_precondition_roster_identity()
    }

    pub fn v13_static_association_identity(&self) -> [u8; 32] {
        self.checked.static_association_identity()
    }

    pub fn artifact_binding(&self) -> (CanonicalCodeObjectDigest, usize, &str) {
        (
            self.checked.artifact_identity(),
            self.checked.kernel_ordinal(),
            self.checked.entry_name(),
        )
    }

    pub const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        self.checked.packing_plan()
    }

    pub fn memory_binding_count(&self) -> usize {
        self.checked.memory_bindings().len()
    }

    /// Performs the first backend call permitted to publish GPU work.
    pub fn submit(
        mut self,
    ) -> Result<
        PendingGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments>,
        Backend::SubmitError,
    > {
        let submission = self.backend.submit_v2(
            self.context,
            self.stream,
            self.checked.geometry(),
            &mut self.arguments,
        )?;
        Ok(PendingGeneratedHostInvocationV2 {
            backend: self.backend,
            context: self.context,
            stream: self.stream,
            arguments: Some(self.arguments),
            submission: Some(submission),
            checked: Some(self.checked),
        })
    }
}

/// In-flight typed invocation retaining every argument, context, and stream borrow.
#[must_use = "dropping an in-flight invocation violates the reviewed backend contract"]
pub struct PendingGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments>
where
    Backend: ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>,
{
    backend: &'runtime mut Backend,
    context: &'runtime mut Context,
    stream: &'runtime Stream,
    arguments: Option<Arguments>,
    submission: Option<Backend::Submission>,
    checked: Option<CheckedGeneratedHostDispatchV2<K>>,
}

/// One nonblocking progress transition that preserves ownership while pending.
pub enum GeneratedHostAsyncProgressV2<Pending, Completion> {
    Pending(Pending),
    Complete(Completion),
}

impl<'runtime, K, Backend, Context, Stream, Arguments>
    PendingGeneratedHostInvocationV2<'runtime, K, Backend, Context, Stream, Arguments>
where
    Backend: ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>,
{
    pub const fn runtime_coordinates(&self) -> GeneratedHostRuntimeCoordinatesV2 {
        self.checked
            .as_ref()
            .expect("pending invocation retains checked host facts")
            .runtime()
    }

    pub fn capability_association_identity(&self) -> ([u8; 32], u64) {
        self.checked
            .as_ref()
            .expect("pending invocation retains checked host facts")
            .capability_association_identity()
    }

    pub fn dynamic_precondition_roster_identity(&self) -> [u8; 32] {
        self.checked
            .as_ref()
            .expect("pending invocation retains checked host facts")
            .dynamic_precondition_roster_identity()
    }

    pub fn v13_static_association_identity(&self) -> [u8; 32] {
        self.checked
            .as_ref()
            .expect("pending invocation retains checked host facts")
            .static_association_identity()
    }

    pub fn artifact_binding(&self) -> (CanonicalCodeObjectDigest, usize, &str) {
        let checked = self
            .checked
            .as_ref()
            .expect("pending invocation retains checked host facts");
        (
            checked.artifact_identity(),
            checked.kernel_ordinal(),
            checked.entry_name(),
        )
    }

    pub const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        self.checked
            .as_ref()
            .expect("pending invocation retains checked host facts")
            .packing_plan()
    }

    pub fn poll(
        mut self,
    ) -> GeneratedHostAsyncProgressV2<Self, CompletedGeneratedHostInvocationV2<K, Arguments>> {
        let status = self.backend.poll_v2(
            self.context,
            self.stream,
            self.submission
                .as_mut()
                .expect("pending invocation retains its submission"),
        );
        match status {
            ReviewedGeneratedHostAsyncStatusV2::Pending => {
                GeneratedHostAsyncProgressV2::Pending(self)
            }
            status @ (ReviewedGeneratedHostAsyncStatusV2::Succeeded
            | ReviewedGeneratedHostAsyncStatusV2::Failed(_)) => {
                drop(self.submission.take());
                GeneratedHostAsyncProgressV2::Complete(CompletedGeneratedHostInvocationV2 {
                    arguments: self
                        .arguments
                        .take()
                        .expect("terminal invocation retains its arguments"),
                    checked: self
                        .checked
                        .take()
                        .expect("terminal invocation retains checked host facts"),
                    status,
                })
            }
        }
    }
}

impl<K, Backend, Context, Stream, Arguments> Drop
    for PendingGeneratedHostInvocationV2<'_, K, Backend, Context, Stream, Arguments>
where
    Backend: ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>,
{
    fn drop(&mut self) {
        let Some(submission) = self.submission.as_mut() else {
            return;
        };
        let status = self
            .backend
            .quiesce_v2(self.context, self.stream, submission);
        if status == ReviewedGeneratedHostAsyncStatusV2::Pending {
            // A reviewed backend contract violation must not release live GPU borrows.
            std::process::abort();
        }
        drop(self.submission.take());
    }
}

/// Quiescent typed completion. Dropping it releases all retained argument borrows.
pub struct CompletedGeneratedHostInvocationV2<K, Arguments> {
    arguments: Arguments,
    checked: CheckedGeneratedHostDispatchV2<K>,
    status: ReviewedGeneratedHostAsyncStatusV2,
}

impl<K, Arguments> CompletedGeneratedHostInvocationV2<K, Arguments> {
    pub const fn runtime_coordinates(&self) -> GeneratedHostRuntimeCoordinatesV2 {
        self.checked.runtime()
    }

    pub fn subject(&self) -> GeneratedHostExecutionSubjectV2 {
        self.checked.subject()
    }

    pub const fn geometry(&self) -> GeneratedHostLaunchGeometryV2 {
        self.checked.geometry()
    }

    pub fn capability_closure_identity(&self) -> [u8; 32] {
        self.checked.capability_closure_identity()
    }

    pub fn capability_association_identity(&self) -> ([u8; 32], u64) {
        self.checked.capability_association_identity()
    }

    pub fn dynamic_precondition_roster_identity(&self) -> [u8; 32] {
        self.checked.dynamic_precondition_roster_identity()
    }

    pub fn compiler_policy_identity(&self) -> [u8; 32] {
        self.checked.compiler_policy_identity()
    }

    pub fn v13_static_association_identity(&self) -> [u8; 32] {
        self.checked.static_association_identity()
    }

    pub fn artifact_binding(&self) -> (CanonicalCodeObjectDigest, usize, &str) {
        (
            self.checked.artifact_identity(),
            self.checked.kernel_ordinal(),
            self.checked.entry_name(),
        )
    }

    pub const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        self.checked.packing_plan()
    }

    pub fn memory_binding_count(&self) -> usize {
        self.checked.memory_bindings().len()
    }

    pub const fn status(&self) -> ReviewedGeneratedHostAsyncStatusV2 {
        self.status
    }

    pub fn into_arguments(self) -> Arguments {
        self.arguments
    }
}

/// Failure to reconcile generated, inspected, and production-carried static facts.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedHostContractErrorV2 {
    InvalidCanonicalConstraint,
    InvalidProductionEvidence,
    InvalidCanonicalKirV13,
    DuplicateMemoryConstraint,
    KernelSubstitution,
    KernelOrdinal,
    GeneratedContractSubstitution,
    DescriptorTargetMismatch,
    DescriptorIdentity,
    V13KernelEntry,
    V13StaticAssociationMismatch,
    MissingSealedProductionResult,
    MissingMachineRefinementReceipt,
    V13PhysicalAbi,
    V13AbiOverflow,
    MissingV13LogicalCapability,
    StaleCapabilityClosure,
    CrossTargetCapabilityClosure,
    CrossLaunchCapabilityClosure,
    CompilerPolicyMismatch,
    GeneratedLayout(GeneratedArgumentLayoutError),
    DescriptorAbi(GeneratedArgumentPackingError),
    DescriptorLaunchMismatch,
    MemoryConstraintCount,
    MemoryConstraintArgument {
        argument_index: usize,
    },
    MemoryRoleMismatch {
        argument_index: usize,
    },
    MemoryElementWidthMismatch {
        argument_index: usize,
        expected: u64,
        observed: u64,
    },
    MemoryAlignmentMismatch {
        argument_index: usize,
    },
    InvalidRuntimeCoordinates,
}

impl fmt::Display for GeneratedHostContractErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemoryElementWidthMismatch {
                argument_index,
                expected,
                observed,
            } => write!(
                formatter,
                "error[FE2O3-CAP-STATIC012]: argument {argument_index} canonical element width mismatch: descriptor requires {expected} bytes, capability facts require {observed} bytes"
            ),
            _ => write!(formatter, "generated host contract rejected: {self:?}"),
        }
    }
}

impl std::error::Error for GeneratedHostContractErrorV2 {}

/// Dynamic W7 rejection. Every variant is produced before `submit_v2` is called.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedHostPrepareErrorV2 {
    GeneratedLayout(GeneratedArgumentLayoutError),
    TargetMismatch,
    ContextMismatch,
    StreamMismatch,
    SubjectMismatch,
    CapabilityClosureMismatch,
    DynamicPreconditionRosterMismatch,
    CompilerPolicyMismatch,
    StaticAssociationMismatch,
    LaunchGeometry,
    MemoryBindingCount,
    MemoryBindingIndex {
        argument_index: usize,
    },
    MemoryRole {
        argument_index: usize,
    },
    ElementWidth {
        argument_index: usize,
        expected: u64,
        observed: u64,
    },
    Dimensions {
        argument_index: usize,
    },
    Extent {
        argument_index: usize,
        axis: usize,
    },
    Stride {
        argument_index: usize,
        axis: usize,
    },
    AllocationExtent {
        argument_index: usize,
    },
    Alignment {
        argument_index: usize,
    },
    AliasConflict {
        left: usize,
        right: usize,
    },
}

impl From<GeneratedArgumentLayoutError> for GeneratedHostPrepareErrorV2 {
    fn from(error: GeneratedArgumentLayoutError) -> Self {
        Self::GeneratedLayout(error)
    }
}

impl fmt::Display for GeneratedHostPrepareErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementWidth {
                argument_index,
                expected,
                observed,
            } => write!(
                formatter,
                "error[FE2O3-CAP-DYNAMIC012]: argument {argument_index} element width mismatch: expected {expected} bytes, observed {observed} bytes"
            ),
            _ => write!(formatter, "error[FE2O3-CAP-DYNAMIC001]: {self:?}"),
        }
    }
}

impl std::error::Error for GeneratedHostPrepareErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompilerGeneratedKernelProfileV1, GeneratedDeviceScalarV1, GeneratedKfdReadSlice,
        GeneratedKfdReadWriteSlice, GeneratedKfdWriteSlice,
    };
    use fe2o3_amd_target::AmdTargetId;
    use fe2o3_artifacts::{
        AbiField, AddressSpace, ArgumentOwnership, Dimensions, Mutability, Name, PointerWidth,
    };
    use fe2o3_device::KernelMarkerV1;
    use fe2o3_kernel_descriptor::{
        AccessMode, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion,
        CompilerIdentityV1, DeviceDescriptorTableDigest, DeviceLayoutDescriptorV1,
        DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
        KernelAbiLayoutV1, KernelDescriptorDigest, KernelId, LaunchConstraintsV1,
        LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1,
        SourceTypeRecordV1, Text, ValidName,
    };
    use fe2o3_kernel_ir::{
        AccessMode as KirAccessMode, AddressSpace as KirAddressSpace, BasicBlock, BlockId,
        Function, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
        LaunchExtent, Module, Operation, ScalarType as KirScalarType, Signature, Terminator,
        Type as KirType, ValueId, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrV12,
        VerifiedCanonicalKernelIrV13,
    };
    use std::collections::VecDeque;

    const FIRST_KERNEL: [u8; 32] = [9; 32];
    const SECOND_KERNEL: [u8; 32] = [10; 32];
    const HOST_CONTRACT: [u8; 32] = [11; 32];
    const SECOND_HOST_CONTRACT: [u8; 32] = [12; 32];

    struct Marker;
    struct SecondMarker;
    struct WrongAbiMarker;
    struct WrongLaunchMarker;

    fn marker_function() {}

    macro_rules! marker {
        ($marker:ty, $binding:expr, $contract:expr) => {
            unsafe impl KernelMarkerV1 for $marker {
                type Function = fn();
                type Registration = ();

                const LOGICAL_NAME: &'static str = "not_a_selector";
                const EXPORT_NAME: &'static str = "not_a_selector";
                const FUNCTION: Self::Function = marker_function;
                const REGISTRATION: &'static Self::Registration = &();
            }

            unsafe impl CompilerGeneratedKernelExpectationV1 for $marker {
                const PROFILE: CompilerGeneratedKernelProfileV1 =
                    CompilerGeneratedKernelProfileV1::new($contract);
                const KERNEL_BINDING_ID_V1: [u8; 32] = $binding;
            }
        };
    }

    marker!(Marker, FIRST_KERNEL, HOST_CONTRACT);
    marker!(SecondMarker, SECOND_KERNEL, SECOND_HOST_CONTRACT);
    marker!(WrongAbiMarker, FIRST_KERNEL, HOST_CONTRACT);
    marker!(WrongLaunchMarker, FIRST_KERNEL, HOST_CONTRACT);

    unsafe impl CompilerGeneratedKernelExpectationV2 for Marker {
        fn generated_host_contract_v2()
        -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError> {
            Ok(CompilerGeneratedHostContractV2::new(
                generated_layout(),
                generated_launch(64),
            ))
        }
    }

    unsafe impl CompilerGeneratedKernelExpectationV2 for SecondMarker {
        fn generated_host_contract_v2()
        -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError> {
            Marker::generated_host_contract_v2()
        }
    }

    unsafe impl CompilerGeneratedKernelExpectationV2 for WrongAbiMarker {
        fn generated_host_contract_v2()
        -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError> {
            let mut fields = generated_fields();
            fields.pop();
            Ok(CompilerGeneratedHostContractV2::new(
                CompilerGeneratedArgumentLayoutV1::new(32, 8, PointerWidth::Bits64, fields)?,
                generated_launch(64),
            ))
        }
    }

    unsafe impl CompilerGeneratedKernelExpectationV2 for WrongLaunchMarker {
        fn generated_host_contract_v2()
        -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError> {
            Ok(CompilerGeneratedHostContractV2::new(
                generated_layout(),
                generated_launch(32),
            ))
        }
    }

    fn generated_launch(block: u32) -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(block, 1, 1).unwrap()),
            Dimensions::new(1024, 1, 1).unwrap(),
            128,
            256,
        )
        .unwrap()
    }

    fn generated_fields() -> Vec<AbiField> {
        [
            ("input", 0, Access::ReadOnly),
            ("output", 16, Access::WriteOnly),
            ("workspace", 32, Access::ReadWrite),
        ]
        .into_iter()
        .map(|(name, offset, access)| {
            let shared = access == Access::ReadOnly;
            AbiField::new(
                Name::new(name).unwrap(),
                offset,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                if shared {
                    Mutability::Immutable
                } else {
                    Mutability::Mutable
                },
                access,
                AddressSpace::Global,
                if shared {
                    f32::shared_slice_type_identity_v1(PointerWidth::Bits64)
                } else {
                    f32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
                },
                if shared {
                    ArgumentOwnership::SharedBorrow
                } else {
                    ArgumentOwnership::UniqueBorrow
                },
                if shared {
                    AliasClass::SharedReadOnly
                } else {
                    AliasClass::Exclusive
                },
            )
            .unwrap()
        })
        .collect()
    }

    fn generated_layout() -> CompilerGeneratedArgumentLayoutV1 {
        CompilerGeneratedArgumentLayoutV1::new(48, 8, PointerWidth::Bits64, generated_fields())
            .unwrap()
    }

    fn descriptor_table() -> DeviceDescriptorTableV1 {
        let shared =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let disjoint =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
        let shared_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let disjoint_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
        let kernel = |binding, name: &str| {
            let arguments = vec![
                LogicalArgumentV1::shared_slice(
                    0,
                    ValidName::new("input").unwrap(),
                    &shared,
                    &shared_layout,
                    0,
                )
                .unwrap(),
                LogicalArgumentV1::disjoint_slice(
                    1,
                    ValidName::new("output").unwrap(),
                    &disjoint,
                    &disjoint_layout,
                    AccessMode::WriteOnly,
                    16,
                )
                .unwrap(),
                LogicalArgumentV1::disjoint_slice(
                    2,
                    ValidName::new("workspace").unwrap(),
                    &disjoint,
                    &disjoint_layout,
                    AccessMode::ReadWrite,
                    32,
                )
                .unwrap(),
            ];
            KernelDescriptorV1::new(
                KernelId::from_bytes(binding),
                ValidName::new(name).unwrap(),
                ValidName::new(name).unwrap(),
                ValidName::new(format!("{name}.kd")).unwrap(),
                BuildEvidenceV1::new(
                    EvidenceIdentity::from_opaque_bytes([1; 32]),
                    EvidenceDigest::from_sha256_bytes([2; 32]),
                ),
                BuildEvidenceV1::new(
                    EvidenceIdentity::from_opaque_bytes([3; 32]),
                    EvidenceDigest::from_sha256_bytes([4; 32]),
                ),
                vec![],
                KernelAbiLayoutV1::new(48, 48, 8).unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                    DimensionsV1::new(1024, 1, 1).unwrap(),
                    1024,
                    128,
                    256,
                )
                .unwrap(),
                arguments,
            )
            .unwrap()
        };
        let kernels = vec![
            kernel(FIRST_KERNEL, "first_physical"),
            kernel(SECOND_KERNEL, "second_physical"),
        ];
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([5; 32]),
            CodeObjectVersion::V6,
            CompilerIdentityV1::new(
                Text::new("rustc").unwrap(),
                Text::new("test").unwrap(),
                [6; 20],
            ),
            ProducerIdentityV1::new(
                Text::new("cargo-fe2o3").unwrap(),
                Text::new("test").unwrap(),
            ),
            DeviceTargetV1::new(AmdTargetId::parse("gfx942").unwrap()),
            vec![shared, disjoint],
            vec![shared_layout, disjoint_layout],
            kernels,
        )
        .unwrap()
    }

    fn v13_graph() -> VerifiedCanonicalKernelIrV13 {
        let mut module = Module::new("generated-host-v13");
        for (ordinal, (name, marker)) in [
            ("first_physical", FIRST_KERNEL),
            ("second_physical", SECOND_KERNEL),
        ]
        .into_iter()
        .enumerate()
        {
            let entry_name = format!("entry_{ordinal}");
            let context = KernelContextTypeV1::new(entry_name.as_str(), marker, [22; 32], [23; 32]);
            let slice = |access| {
                KirType::slice(
                    KirType::Scalar(KirScalarType::F32),
                    KirAddressSpace::Global,
                    access,
                )
            };
            let mut block = BasicBlock::new(BlockId(0));
            block.operations.push(Operation::kernel_context_issue(
                ValueId(3),
                context,
                KernelContextSourceIdentityV1::new(
                    [31 + ordinal as u8; 32],
                    [41 + ordinal as u8; 32],
                    [51 + ordinal as u8; 32],
                    [61 + ordinal as u8; 32],
                ),
            ));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::kernel_entry(
                entry_name.as_str(),
                Signature::new(
                    vec![
                        slice(KirAccessMode::ReadOnly),
                        slice(KirAccessMode::WriteOnly),
                        slice(KirAccessMode::ReadWrite),
                    ],
                    vec![],
                ),
                vec![ValueId(0), ValueId(1), ValueId(2)],
                vec![block],
            ));
            module.kernels.push(Kernel::new(
                name,
                entry_name,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            ));
        }
        VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
    }

    fn subject() -> GeneratedHostExecutionSubjectV2 {
        GeneratedHostExecutionSubjectV2::from_v13(&v13_graph(), 7, [22; 32], [23; 32]).unwrap()
    }

    fn constraints() -> Vec<GeneratedHostMemoryConstraintV2> {
        [
            GeneratedHostMemoryRoleV2::Input,
            GeneratedHostMemoryRoleV2::Output,
            GeneratedHostMemoryRoleV2::Workspace,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            GeneratedHostMemoryConstraintV2::new(
                index,
                role,
                4,
                4,
                vec![GeneratedHostAxisConstraintV2::new(4, 4, 1, 1).unwrap()],
            )
            .unwrap()
        })
        .collect()
    }

    fn production_facts<K: CompilerGeneratedKernelExpectationV1>(
        epoch: u64,
        target: [u8; 32],
        launch: [u8; 32],
    ) -> ProductionGeneratedHostFactsV2 {
        let table = descriptor_table();
        let ordinal = usize::from(K::KERNEL_BINDING_ID_V1 == SECOND_KERNEL);
        let descriptor = &table.kernels()[ordinal];
        ProductionGeneratedHostFactsV2::for_test_only(
            K::KERNEL_BINDING_ID_V1,
            K::PROFILE.generated_host_contract_identity(),
            "gfx942",
            table.canonical_code_object_digest(),
            DeviceDescriptorTableDigest::calculate(&table).unwrap(),
            KernelDescriptorDigest::calculate(descriptor),
            descriptor.entry_name().as_str(),
            ordinal,
            v13_graph(),
            epoch,
            target,
            launch,
            [24; 32],
            [25; 32],
            512,
            [27; 32],
            [26; 32],
            constraints(),
        )
        .unwrap()
    }

    fn admission() -> AdmittedGeneratedHostContractV2<Marker> {
        let table = descriptor_table();
        admit_generated_host_contract_v2::<Marker>(
            &table,
            &table.kernels()[0],
            production_facts::<Marker>(7, [22; 32], [23; 32]),
        )
        .unwrap()
    }

    #[test]
    fn production_memory_contract_is_derived_from_kir_and_inspected_abi() {
        let table = descriptor_table();
        let descriptor = &table.kernels()[0];
        let packing =
            validate_worker_v3_argument_packing(&table, descriptor, &generated_layout()).unwrap();
        let graph = v13_graph();
        let module = decode_module_v13(graph.canonical_bytes()).unwrap();
        let memory = derive_canonical_memory_constraints_v2(&module, 0, &packing).unwrap();
        assert_eq!(memory.len(), 3);
        assert_eq!(memory[0].role(), GeneratedHostMemoryRoleV2::Input);
        assert_eq!(memory[1].role(), GeneratedHostMemoryRoleV2::Output);
        assert_eq!(memory[2].role(), GeneratedHostMemoryRoleV2::InputOutput);
        for (index, constraint) in memory.iter().enumerate() {
            assert_eq!(constraint.argument_index(), index);
            assert_eq!(constraint.element_bytes(), 4);
            assert_eq!(constraint.required_alignment(), 4);
            assert_eq!(constraint.axes().len(), 1);
            assert_eq!(constraint.axes()[0].minimum_extent(), 0);
            assert_eq!(constraint.axes()[0].maximum_extent(), u64::MAX / 4);
            assert_eq!(constraint.axes()[0].minimum_stride(), 1);
            assert_eq!(constraint.axes()[0].maximum_stride(), 1);
        }
    }

    #[test]
    fn static_association_commits_dynamic_roster_and_compiler_policy() {
        let association = |dynamic_preconditions, compiler_policy| {
            ProductionV13StaticAssociationV2::for_test_only(
                v13_graph(),
                7,
                [22; 32],
                [23; 32],
                [24; 32],
                [25; 32],
                512,
                dynamic_preconditions,
                compiler_policy,
            )
            .unwrap()
            .identity
        };
        let baseline = association([27; 32], [26; 32]);
        assert_ne!(baseline, association([47; 32], [26; 32]));
        assert_ne!(baseline, association([27; 32], [46; 32]));
    }

    fn runtime() -> GeneratedHostRuntimeCoordinatesV2 {
        GeneratedHostRuntimeCoordinatesV2::new([22; 32], [31; 32], [32; 32]).unwrap()
    }

    fn dispatch_evidence() -> GeneratedHostDispatchEvidenceV2 {
        admission().bind_dispatch_evidence_v2(runtime())
    }

    fn geometry() -> GeneratedHostLaunchGeometryV2 {
        GeneratedHostLaunchGeometryV2::new([256, 1, 1], [64, 1, 1], 128)
    }

    struct TestArguments(Vec<GeneratedHostMemoryBindingV2>);

    unsafe impl<'allocation> CompilerGeneratedHostArgumentsV2<'allocation, Marker> for TestArguments {
        fn generated_host_memory_bindings_v2(&self) -> Vec<GeneratedHostMemoryBindingV2> {
            self.0.clone()
        }
    }

    fn bindings() -> Vec<GeneratedHostMemoryBindingV2> {
        [
            (0, 0x1000, GeneratedHostMemoryRoleV2::Input),
            (1, 0x2000, GeneratedHostMemoryRoleV2::Output),
            (2, 0x3000, GeneratedHostMemoryRoleV2::Workspace),
        ]
        .into_iter()
        .map(|(index, address, role)| {
            GeneratedHostMemoryBindingV2::from_compiler_generated_argument_v2(
                index,
                address,
                4,
                4,
                role,
                role.expected_alias(),
                vec![4],
                vec![1],
            )
        })
        .collect()
    }

    struct Context;
    struct Stream;

    struct Backend {
        runtime: GeneratedHostRuntimeCoordinatesV2,
        submissions: usize,
        quiescences: usize,
        statuses: VecDeque<ReviewedGeneratedHostAsyncStatusV2>,
    }

    impl Backend {
        fn new() -> Self {
            Self {
                runtime: runtime(),
                submissions: 0,
                quiescences: 0,
                statuses: VecDeque::from([
                    ReviewedGeneratedHostAsyncStatusV2::Pending,
                    ReviewedGeneratedHostAsyncStatusV2::Succeeded,
                ]),
            }
        }
    }

    // SAFETY: this unit backend has no GPU side effects, observes its fixed coordinates, and
    // reports terminal completion only after a deterministic pending transition.
    unsafe impl<K, Arguments> ReviewedGeneratedHostAsyncBackendV2<K, Context, Stream, Arguments>
        for Backend
    {
        type Submission = ();
        type SubmitError = ();

        fn observe_runtime_v2(
            &self,
            _context: &Context,
            _stream: &Stream,
        ) -> GeneratedHostRuntimeCoordinatesV2 {
            self.runtime
        }

        fn submit_v2(
            &mut self,
            _context: &mut Context,
            _stream: &Stream,
            _geometry: GeneratedHostLaunchGeometryV2,
            _arguments: &mut Arguments,
        ) -> Result<Self::Submission, Self::SubmitError> {
            self.submissions += 1;
            Ok(())
        }

        fn poll_v2(
            &mut self,
            _context: &mut Context,
            _stream: &Stream,
            _submission: &mut Self::Submission,
        ) -> ReviewedGeneratedHostAsyncStatusV2 {
            self.statuses.pop_front().unwrap()
        }

        fn quiesce_v2(
            &mut self,
            _context: &mut Context,
            _stream: &Stream,
            _submission: &mut Self::Submission,
        ) -> ReviewedGeneratedHostAsyncStatusV2 {
            self.quiescences += 1;
            ReviewedGeneratedHostAsyncStatusV2::Succeeded
        }
    }

    fn rejected_before_submission(
        bindings: Vec<GeneratedHostMemoryBindingV2>,
        evidence: GeneratedHostDispatchEvidenceV2,
        geometry: GeneratedHostLaunchGeometryV2,
        expected: &str,
    ) {
        let admission = admission();
        rejected_with_admission(&admission, bindings, evidence, geometry, expected);
    }

    fn rejected_with_admission(
        admission: &AdmittedGeneratedHostContractV2<Marker>,
        bindings: Vec<GeneratedHostMemoryBindingV2>,
        evidence: GeneratedHostDispatchEvidenceV2,
        geometry: GeneratedHostLaunchGeometryV2,
        expected: &str,
    ) {
        let mut backend = Backend::new();
        let mut context = Context;
        let stream = Stream;
        let error = match admission.prepare(
            &mut backend,
            &mut context,
            &stream,
            evidence,
            geometry,
            TestArguments(bindings),
        ) {
            Ok(_) => panic!("hostile preparation unexpectedly succeeded"),
            Err(error) => error,
        };
        assert!(format!("{error:?}").contains(expected), "{error:?}");
        assert_eq!(backend.submissions, 0);
    }

    #[test]
    fn shape_stride_and_byte_arithmetic_overflow_before_submission() {
        let mut broad = admission();
        broad.memory[0].axes[0] =
            GeneratedHostAxisConstraintV2::new(0, u64::MAX, 0, u64::MAX).unwrap();
        let mut changed = bindings();
        changed[0].extents[0] = u64::MAX;
        changed[0].strides[0] = u64::MAX;
        changed[0].allocation_elements = u64::MAX;
        rejected_with_admission(
            &broad,
            changed,
            dispatch_evidence(),
            geometry(),
            "AllocationExtent",
        );

        let mut changed = bindings();
        changed[0].allocation_elements = u64::MAX;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "AllocationExtent");

        let mut changed = bindings();
        changed[0].address = usize::MAX - 3;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "AllocationExtent");
    }

    #[test]
    fn preparation_checks_every_dynamic_axis_before_submission() {
        let mut changed = bindings();
        changed[0].extents = vec![2, 2].into_boxed_slice();
        changed[0].strides = vec![2, 1].into_boxed_slice();
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "Dimensions");

        let mut changed = bindings();
        changed[0].extents[0] = 3;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "Extent");

        let mut changed = bindings();
        changed[0].strides[0] = 2;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "Stride");

        let mut changed = bindings();
        changed[0].element_bytes = 8;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "ElementWidth");

        let mut changed = bindings();
        changed[0].allocation_elements = 3;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "AllocationExtent");

        let mut changed = bindings();
        changed[0].address += 1;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "Alignment");

        let mut changed = bindings();
        changed[1].role = GeneratedHostMemoryRoleV2::Input;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "MemoryRole");

        let mut changed = bindings();
        changed[1].alias = AliasClass::SharedReadOnly;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "MemoryRole");

        let mut changed = bindings();
        changed[2].address = changed[1].address;
        rejected_before_submission(changed, dispatch_evidence(), geometry(), "AliasConflict");

        let mut evidence = dispatch_evidence();
        evidence.runtime.target_model_identity = [41; 32];
        rejected_before_submission(bindings(), evidence, geometry(), "TargetMismatch");

        let mut evidence = dispatch_evidence();
        evidence.runtime.context_identity = [42; 32];
        rejected_before_submission(bindings(), evidence, geometry(), "ContextMismatch");

        let mut evidence = dispatch_evidence();
        evidence.runtime.stream_identity = [43; 32];
        rejected_before_submission(bindings(), evidence, geometry(), "StreamMismatch");

        let mut evidence = dispatch_evidence();
        evidence.capability_closure_identity = [44; 32];
        rejected_before_submission(
            bindings(),
            evidence,
            geometry(),
            "CapabilityClosureMismatch",
        );

        let mut evidence = dispatch_evidence();
        evidence.dynamic_precondition_roster_identity = [47; 32];
        rejected_before_submission(
            bindings(),
            evidence,
            geometry(),
            "DynamicPreconditionRosterMismatch",
        );

        let mut evidence = dispatch_evidence();
        evidence.compiler_policy_identity = [45; 32];
        rejected_before_submission(bindings(), evidence, geometry(), "CompilerPolicyMismatch");

        let mut evidence = dispatch_evidence();
        evidence.subject.final_epoch += 1;
        rejected_before_submission(bindings(), evidence, geometry(), "SubjectMismatch");

        let mut evidence = dispatch_evidence();
        evidence.subject.final_graph_identity[0] ^= 1;
        rejected_before_submission(bindings(), evidence, geometry(), "SubjectMismatch");

        let mut evidence = dispatch_evidence();
        evidence.static_association_identity[0] ^= 1;
        rejected_before_submission(
            bindings(),
            evidence,
            geometry(),
            "StaticAssociationMismatch",
        );

        let mut evidence = dispatch_evidence();
        evidence.artifact_identity = CanonicalCodeObjectDigest::from_bytes([46; 32]);
        rejected_before_submission(
            bindings(),
            evidence,
            geometry(),
            "StaticAssociationMismatch",
        );

        let mut evidence = dispatch_evidence();
        evidence.kernel_ordinal = 1;
        rejected_before_submission(
            bindings(),
            evidence,
            geometry(),
            "StaticAssociationMismatch",
        );

        let mut changed_geometry = geometry();
        changed_geometry.workgroup[0] = 32;
        rejected_before_submission(
            bindings(),
            dispatch_evidence(),
            changed_geometry,
            "LaunchGeometry",
        );

        let mut changed_geometry = geometry();
        changed_geometry.dynamic_lds_bytes = 257;
        rejected_before_submission(
            bindings(),
            dispatch_evidence(),
            changed_geometry,
            "LaunchGeometry",
        );
    }

    struct BorrowedArguments<'allocation> {
        input: GeneratedKfdReadSlice<'allocation, f32>,
        output: GeneratedKfdWriteSlice<'allocation, f32>,
        workspace: GeneratedKfdReadWriteSlice<'allocation, f32>,
    }

    unsafe impl<'allocation> CompilerGeneratedHostArgumentsV2<'allocation, Marker>
        for BorrowedArguments<'allocation>
    {
        fn generated_host_memory_bindings_v2(&self) -> Vec<GeneratedHostMemoryBindingV2> {
            vec![
                self.input.generated_host_memory_binding_v2(0),
                self.output.generated_host_memory_binding_v2(1),
                self.workspace.generated_host_memory_binding_v2(2),
            ]
        }
    }

    #[test]
    fn pending_completion_retains_arguments_context_and_stream() {
        let input = [1.0_f32; 4];
        let mut output = [0.0_f32; 4];
        let mut workspace = [0.0_f32; 4];
        let arguments = BorrowedArguments {
            input: GeneratedKfdReadSlice::new(&input),
            output: GeneratedKfdWriteSlice::new(&mut output),
            workspace: GeneratedKfdReadWriteSlice::workspace(&mut workspace),
        };
        let admission = admission();
        let static_association = admission.v13_static_association_identity();
        let artifact_binding = (
            CanonicalCodeObjectDigest::from_bytes([5; 32]),
            0,
            "first_physical",
        );
        let mut backend = Backend::new();
        let mut context = Context;
        let stream = Stream;
        let prepared = admission
            .prepare(
                &mut backend,
                &mut context,
                &stream,
                dispatch_evidence(),
                geometry(),
                arguments,
            )
            .unwrap();
        assert_eq!(prepared.runtime_coordinates(), runtime());
        assert_eq!(prepared.subject(), subject());
        assert_eq!(prepared.geometry(), geometry());
        assert_eq!(prepared.capability_association_identity(), ([25; 32], 512));
        assert_eq!(prepared.dynamic_precondition_roster_identity(), [27; 32]);
        assert_eq!(
            prepared.v13_static_association_identity(),
            static_association
        );
        assert_eq!(prepared.artifact_binding(), artifact_binding);
        assert_eq!(prepared.memory_binding_count(), 3);
        assert_eq!(
            prepared.packing_plan().kernel_id().as_bytes(),
            &FIRST_KERNEL
        );
        let pending = prepared.submit().unwrap();
        assert_eq!(pending.runtime_coordinates(), runtime());
        assert_eq!(pending.capability_association_identity(), ([25; 32], 512));
        assert_eq!(pending.dynamic_precondition_roster_identity(), [27; 32]);
        assert_eq!(
            pending.v13_static_association_identity(),
            static_association
        );
        assert_eq!(pending.artifact_binding(), artifact_binding);
        assert_eq!(pending.packing_plan().kernel_id().as_bytes(), &FIRST_KERNEL);
        let pending = match pending.poll() {
            GeneratedHostAsyncProgressV2::Pending(pending) => pending,
            GeneratedHostAsyncProgressV2::Complete(_) => panic!("completed too early"),
        };
        let completion = match pending.poll() {
            GeneratedHostAsyncProgressV2::Complete(completion) => completion,
            GeneratedHostAsyncProgressV2::Pending(_) => panic!("remained pending"),
        };
        assert_eq!(
            completion.status(),
            ReviewedGeneratedHostAsyncStatusV2::Succeeded
        );
        assert_eq!(completion.runtime_coordinates(), runtime());
        assert_eq!(completion.subject(), subject());
        assert_eq!(completion.geometry(), geometry());
        assert_eq!(completion.capability_closure_identity(), [24; 32]);
        assert_eq!(
            completion.dynamic_precondition_roster_identity(),
            [27; 32]
        );
        assert_eq!(
            completion.capability_association_identity(),
            ([25; 32], 512)
        );
        assert_eq!(completion.compiler_policy_identity(), [26; 32]);
        assert_eq!(
            completion.v13_static_association_identity(),
            static_association
        );
        assert_eq!(completion.artifact_binding(), artifact_binding);
        assert_eq!(completion.memory_binding_count(), 3);
        assert_eq!(
            completion.packing_plan().kernel_id().as_bytes(),
            &FIRST_KERNEL
        );
        drop(completion.into_arguments());
        output[0] = 7.0;
        workspace[0] = 8.0;
        assert_eq!((output[0], workspace[0]), (7.0, 8.0));
        assert_eq!(backend.submissions, 1);
        assert_eq!(backend.quiescences, 0);
    }

    #[test]
    fn dropping_pending_quiesces_before_releasing_borrows() {
        let input = [1.0_f32; 4];
        let mut output = [0.0_f32; 4];
        let mut workspace = [0.0_f32; 4];
        let arguments = BorrowedArguments {
            input: GeneratedKfdReadSlice::new(&input),
            output: GeneratedKfdWriteSlice::new(&mut output),
            workspace: GeneratedKfdReadWriteSlice::workspace(&mut workspace),
        };
        let admission = admission();
        let mut backend = Backend::new();
        let mut context = Context;
        let stream = Stream;
        let pending = admission
            .prepare(
                &mut backend,
                &mut context,
                &stream,
                dispatch_evidence(),
                geometry(),
                arguments,
            )
            .unwrap()
            .submit()
            .unwrap();
        drop(pending);
        output[0] = 7.0;
        workspace[0] = 8.0;
        assert_eq!((output[0], workspace[0]), (7.0, 8.0));
        assert_eq!(backend.quiescences, 1);
    }

    #[test]
    fn descriptor_and_static_custody_substitutions_fail_closed() {
        let table = descriptor_table();
        assert!(matches!(
            admit_generated_host_contract_v2::<WrongAbiMarker>(
                &table,
                &table.kernels()[0],
                production_facts::<WrongAbiMarker>(7, [22; 32], [23; 32]),
            ),
            Err(GeneratedHostContractErrorV2::DescriptorAbi(_))
        ));
        assert!(matches!(
            admit_generated_host_contract_v2::<WrongLaunchMarker>(
                &table,
                &table.kernels()[0],
                production_facts::<WrongLaunchMarker>(7, [22; 32], [23; 32]),
            ),
            Err(GeneratedHostContractErrorV2::DescriptorLaunchMismatch)
        ));

        let mut wrong_width = production_facts::<Marker>(7, [22; 32], [23; 32]);
        wrong_width.memory.as_mut().unwrap()[0].element_bytes = 8;
        assert!(matches!(
            admit_generated_host_contract_v2::<Marker>(&table, &table.kernels()[0], wrong_width,),
            Err(GeneratedHostContractErrorV2::MemoryElementWidthMismatch {
                argument_index: 0,
                expected: 4,
                observed: 8,
            })
        ));

        let mut wrong_descriptor = production_facts::<Marker>(7, [22; 32], [23; 32]);
        wrong_descriptor.descriptor_identity =
            KernelDescriptorDigest::calculate(&table.kernels()[1]);
        assert!(matches!(
            admit_generated_host_contract_v2::<Marker>(
                &table,
                &table.kernels()[0],
                wrong_descriptor,
            ),
            Err(GeneratedHostContractErrorV2::DescriptorIdentity)
        ));

        assert!(matches!(
            admit_generated_host_contract_v2::<Marker>(
                &table,
                &table.kernels()[0],
                production_facts::<Marker>(7, [51; 32], [23; 32]),
            ),
            Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch)
        ));

        assert!(matches!(
            admit_generated_host_contract_v2::<Marker>(
                &table,
                &table.kernels()[0],
                production_facts::<Marker>(7, [22; 32], [52; 32]),
            ),
            Err(GeneratedHostContractErrorV2::V13StaticAssociationMismatch)
        ));

        let v12 =
            VerifiedCanonicalKernelIrV12::from_module(Module::new("v12-substitution")).unwrap();
        assert!(matches!(
            VerifiedCanonicalKernelIrV13::from_canonical_bytes(v12.into_canonical_bytes()),
            Err(VerifiedCanonicalKernelIrErrorV13::NotExactV13 { version: 12 })
        ));
    }

    #[test]
    fn multiple_kernels_bind_by_identity_without_name_selectors() {
        let table = descriptor_table();
        let first = admit_generated_host_contract_v2::<Marker>(
            &table,
            &table.kernels()[0],
            production_facts::<Marker>(7, [22; 32], [23; 32]),
        )
        .unwrap();
        assert_eq!(first.kernel_ordinal(), 0);
        assert_eq!(first.zero_byte_logical_capability_count(), 1);
        assert_eq!(first.physical_abi_parameter_count(), 3);
        assert_ne!(first.physical_abi_identity(), [0; 32]);
        let second = admit_generated_host_contract_v2::<SecondMarker>(
            &table,
            &table.kernels()[1],
            production_facts::<SecondMarker>(7, [22; 32], [23; 32]),
        )
        .unwrap();
        assert_eq!(second.kernel_ordinal(), 1);
        assert!(matches!(
            admit_generated_host_contract_v2::<Marker>(
                &table,
                &table.kernels()[1],
                production_facts::<Marker>(7, [22; 32], [23; 32]),
            ),
            Err(GeneratedHostContractErrorV2::KernelSubstitution)
        ));
    }

    #[test]
    fn element_width_diagnostic_is_stable() {
        let mut changed = bindings();
        changed[0].element_bytes = 8;
        let admission = admission();
        let mut backend = Backend::new();
        let mut context = Context;
        let stream = Stream;
        let error = match admission.prepare(
            &mut backend,
            &mut context,
            &stream,
            dispatch_evidence(),
            geometry(),
            TestArguments(changed),
        ) {
            Ok(_) => panic!("element-width substitution unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "error[FE2O3-CAP-DYNAMIC012]: argument 0 element width mismatch: expected 4 bytes, observed 8 bytes"
        );
        assert_eq!(backend.submissions, 0);
    }
}
