#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

//! Exact application of a checked machine-structure receipt to a prepared dispatch.

use core::fmt;

use fe2o3_kernel_analysis::CheckedGfx942AtomicCollectiveMachineStructureV1;
use fe2o3_kfd::CheckedGfx942XnackMinusDevice;

use fe2o3_runtime::{
    Gfx942AuthorizedRuntimeDispatchResultV1, Gfx942AuthorizedRuntimeExecutionErrorV1,
    PreparedGfx942RuntimeDispatchV1, WorkerV3Gfx942ExecutionAuthorityV1,
    execute_authorized_gfx942_runtime_dispatch_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942MachineStructureApplicationErrorV1 {
    ArtifactDigestMismatch,
    ArtifactLengthMismatch,
    KernelSymbolMismatch,
    DescriptorDigestMismatch,
    EntryDigestMismatch,
}

impl fmt::Display for Gfx942MachineStructureApplicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 machine-structure receipt does not match prepared dispatch: {self:?}"
        )
    }
}

impl std::error::Error for Gfx942MachineStructureApplicationErrorV1 {}

/// Move-only exact binding of structural machine evidence to one prepared dispatch.
///
/// This value retains both owners. It neither implements Worker V3 authority nor
/// permits native execution. Concrete device generation, queue occurrence, and
/// dispatch generation remain the responsibility of the Worker V3/native-owner
/// transition.
#[must_use = "machine-structure application is not load or launch authority"]
pub struct AppliedGfx942AtomicCollectiveMachineStructureV1 {
    structure: CheckedGfx942AtomicCollectiveMachineStructureV1,
    prepared: PreparedGfx942RuntimeDispatchV1,
}

impl fmt::Debug for AppliedGfx942AtomicCollectiveMachineStructureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppliedGfx942AtomicCollectiveMachineStructureV1")
            .field("structure", &self.structure)
            .field("prepared", &self.prepared)
            .finish_non_exhaustive()
    }
}

impl AppliedGfx942AtomicCollectiveMachineStructureV1 {
    pub const fn establishes_machine_instruction_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn binds_device_queue_and_dispatch_generation(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942AtomicCollectiveMachineStructureV1,
        PreparedGfx942RuntimeDispatchV1,
    ) {
        (self.structure, self.prepared)
    }
}

/// Completed Worker V3 execution retaining the structure required at admission.
#[must_use]
pub struct Gfx942MachineStructureAppliedDispatchResultV1 {
    structure: CheckedGfx942AtomicCollectiveMachineStructureV1,
    execution: Gfx942AuthorizedRuntimeDispatchResultV1,
}

impl fmt::Debug for Gfx942MachineStructureAppliedDispatchResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942MachineStructureAppliedDispatchResultV1")
            .field("structure", &self.structure)
            .field("execution", &self.execution)
            .finish()
    }
}

impl Gfx942MachineStructureAppliedDispatchResultV1 {
    pub fn structure(&self) -> &CheckedGfx942AtomicCollectiveMachineStructureV1 {
        &self.structure
    }

    pub fn execution(&self) -> &Gfx942AuthorizedRuntimeDispatchResultV1 {
        &self.execution
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942AtomicCollectiveMachineStructureV1,
        Gfx942AuthorizedRuntimeDispatchResultV1,
    ) {
        (self.structure, self.execution)
    }

    pub const fn establishes_machine_instruction_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
}

/// Executes through the existing Worker V3 gate while requiring the exact
/// prepared-dispatch machine-structure application on this path.
///
/// Worker V3 remains the semantic and launch authority. The applied structure
/// is an additional exact-artifact structural prerequisite and is retained in
/// the successful result.
pub fn execute_machine_structure_applied_gfx942_runtime_dispatch_v1<A>(
    authority: A,
    device: CheckedGfx942XnackMinusDevice,
    applied: AppliedGfx942AtomicCollectiveMachineStructureV1,
) -> Result<
    Gfx942MachineStructureAppliedDispatchResultV1,
    Gfx942AuthorizedRuntimeExecutionErrorV1<A::CurrentnessError>,
>
where
    A: WorkerV3Gfx942ExecutionAuthorityV1,
{
    let (structure, prepared) = applied.into_parts();
    execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared).map(|execution| {
        Gfx942MachineStructureAppliedDispatchResultV1 {
            structure,
            execution,
        }
    })
}

/// A failed application returns custody of both move-only inputs.
pub struct Gfx942MachineStructureApplicationFailureV1 {
    structure: Box<CheckedGfx942AtomicCollectiveMachineStructureV1>,
    prepared: Box<PreparedGfx942RuntimeDispatchV1>,
    error: Gfx942MachineStructureApplicationErrorV1,
}

impl fmt::Debug for Gfx942MachineStructureApplicationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942MachineStructureApplicationFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942MachineStructureApplicationFailureV1 {
    pub const fn error(&self) -> Gfx942MachineStructureApplicationErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942AtomicCollectiveMachineStructureV1,
        PreparedGfx942RuntimeDispatchV1,
        Gfx942MachineStructureApplicationErrorV1,
    ) {
        (*self.structure, *self.prepared, self.error)
    }
}

/// Applies exact authenticated machine structure to the same prepared artifact and entry.
pub fn apply_gfx942_atomic_collective_machine_structure_v1(
    structure: CheckedGfx942AtomicCollectiveMachineStructureV1,
    prepared: PreparedGfx942RuntimeDispatchV1,
) -> Result<
    AppliedGfx942AtomicCollectiveMachineStructureV1,
    Gfx942MachineStructureApplicationFailureV1,
> {
    let prepared_identity = prepared.identity();
    let result = check_exact_application(
        structure.artifact_identity().sha256(),
        structure.artifact_identity().byte_len(),
        structure.kernel_symbol(),
        structure.descriptor_identity().as_bytes(),
        structure.entry_sha256(),
        prepared_identity.object_sha256(),
        prepared.finalized_hsaco_length(),
        prepared.kernel_name(),
        prepared_identity.descriptor_sha256(),
        prepared_identity.entry_sha256(),
    );
    match result {
        Ok(()) => Ok(AppliedGfx942AtomicCollectiveMachineStructureV1 {
            structure,
            prepared,
        }),
        Err(error) => Err(Gfx942MachineStructureApplicationFailureV1 {
            structure: Box::new(structure),
            prepared: Box::new(prepared),
            error,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn check_exact_application(
    structure_artifact_digest: [u8; 32],
    structure_artifact_length: u64,
    structure_kernel_symbol: &str,
    structure_descriptor_digest: [u8; 32],
    structure_entry_digest: [u8; 32],
    prepared_artifact_digest: [u8; 32],
    prepared_artifact_length: u64,
    prepared_kernel_symbol: &str,
    prepared_descriptor_digest: [u8; 32],
    prepared_entry_digest: [u8; 32],
) -> Result<(), Gfx942MachineStructureApplicationErrorV1> {
    if structure_artifact_digest != prepared_artifact_digest {
        return Err(Gfx942MachineStructureApplicationErrorV1::ArtifactDigestMismatch);
    }
    if structure_artifact_length != prepared_artifact_length {
        return Err(Gfx942MachineStructureApplicationErrorV1::ArtifactLengthMismatch);
    }
    if structure_kernel_symbol != prepared_kernel_symbol {
        return Err(Gfx942MachineStructureApplicationErrorV1::KernelSymbolMismatch);
    }
    if structure_descriptor_digest != prepared_descriptor_digest {
        return Err(Gfx942MachineStructureApplicationErrorV1::DescriptorDigestMismatch);
    }
    if structure_entry_digest != prepared_entry_digest {
        return Err(Gfx942MachineStructureApplicationErrorV1::EntryDigestMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_with_change(field: usize) -> Result<(), Gfx942MachineStructureApplicationErrorV1> {
        let mut artifact = [1; 32];
        let mut artifact_length = 64;
        let mut kernel = "alpha";
        let mut descriptor = [2; 32];
        let mut entry = [3; 32];
        match field {
            0 => artifact[0] ^= 1,
            1 => artifact_length += 1,
            2 => kernel = "zeta",
            3 => descriptor[0] ^= 1,
            4 => entry[0] ^= 1,
            _ => {}
        }
        check_exact_application(
            [1; 32],
            64,
            "alpha",
            [2; 32],
            [3; 32],
            artifact,
            artifact_length,
            kernel,
            descriptor,
            entry,
        )
    }

    #[test]
    fn exact_application_accepts_only_all_equal_identity_fields() {
        assert_eq!(check_with_change(usize::MAX), Ok(()));
        assert_eq!(
            check_with_change(0),
            Err(Gfx942MachineStructureApplicationErrorV1::ArtifactDigestMismatch)
        );
        assert_eq!(
            check_with_change(1),
            Err(Gfx942MachineStructureApplicationErrorV1::ArtifactLengthMismatch)
        );
        assert_eq!(
            check_with_change(2),
            Err(Gfx942MachineStructureApplicationErrorV1::KernelSymbolMismatch)
        );
        assert_eq!(
            check_with_change(3),
            Err(Gfx942MachineStructureApplicationErrorV1::DescriptorDigestMismatch)
        );
        assert_eq!(
            check_with_change(4),
            Err(Gfx942MachineStructureApplicationErrorV1::EntryDigestMismatch)
        );
    }
}
