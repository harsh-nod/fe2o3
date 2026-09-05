//! Authenticated Worker V3 program custody for the persistent service queue.

use std::error::Error;
use std::fmt;

use fe2o3_amdhsa_loader::{
    AdmittedProfile, KernelClosureError, KernelDispatchAbiErrorV1, KernelGlobalBufferAbiV1,
    PlanError, ValidatedKernelEnvelope, validate,
};
use fe2o3_hsaco::{ArgumentAccess, InspectedKernel, KernelDescriptorBinding};
use fe2o3_kernel_descriptor::{
    AccessMode, DeviceDescriptorTableV1, KernelDescriptorV1, PhysicalAbiComponentKind,
};
use fe2o3_service_host::{
    DeviceAllocationRoleMarkerV1, DeviceLocalAllocationV1, QuarantinedServiceQueueV1,
    ServiceAllocationSessionV1, ServiceAllocationSubleaseSetV1, ServiceCompletedQueueSessionV1,
    ServiceCompletedReadRequestV1, ServiceCompletedReadbackV1, ServiceCompletedSnapshotRequestV1,
    ServiceDeviceDispatchRangeV1, ServiceFixedBatchV1, ServiceFixedDispatchPacketV1,
    ServiceHostDispatchRangeV1, ServiceHostDispatchSnapshotRangeV1, ServicePublishedQueueSessionV1,
    ServiceQueueBindFailureV1, ServiceQueueCreateFailureV1, ServiceQueueDataUpdateFailureV1,
    ServiceQueueErrorV1, ServiceQueueHostDataUpdateV1, ServiceQueueOperationFailureV1,
    ServiceQueuePartitionedDataUpdateV1, ServiceQueuePollV1, ServiceQueuePollWithProgressV1,
    ServiceQueueProgressV1, ServiceQueueReleaseFailureV1, ServiceQueueReleaseObservationV1,
    ServiceQueueRolloverFailureV1, ServiceQueueRolloverSuccessV1, ServiceQueueSessionV1,
    ServiceQueueUnboundSessionV1, ServiceRecycledQueueSessionV1,
};

use crate::{
    AuthenticatedWorkerV3RosterV1, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV1, RecoveredWorkerV3AdmissionErrorV1,
};

const MAX_AUTHENTICATED_SERVICE_PROGRAMS_V1: usize =
    fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1;

trait ErasedAuthenticatedWorkerV3RosterV1: fmt::Debug {
    fn entry_count(&self) -> usize;
    fn generated_host_contract(&self, ordinal: usize) -> [u8; 32];
    fn exact_current_hsaco_bytes(&self) -> &[u8];
    fn descriptor_table(&self) -> &DeviceDescriptorTableV1;
    fn descriptor(&self, ordinal: usize) -> &KernelDescriptorV1;
    fn physical_kernel(&self, ordinal: usize) -> &InspectedKernel;
    fn descriptor_binding(&self, ordinal: usize) -> KernelDescriptorBinding;
    fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1>;
}

impl<R> ErasedAuthenticatedWorkerV3RosterV1 for AuthenticatedWorkerV3RosterV1<R>
where
    R: CompilerGeneratedKernelExpectationRosterV1 + 'static,
{
    fn entry_count(&self) -> usize {
        self.entry_count()
    }

    fn generated_host_contract(&self, ordinal: usize) -> [u8; 32] {
        self.verification().entries()[ordinal].generated_host_contract_identity()
    }

    fn exact_current_hsaco_bytes(&self) -> &[u8] {
        self.exact_current_hsaco_bytes()
    }

    fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.admitted_roster().descriptor_table()
    }

    fn descriptor(&self, ordinal: usize) -> &KernelDescriptorV1 {
        self.admitted_roster()
            .descriptor(ordinal)
            .expect("authenticated roster retains every descriptor")
    }

    fn physical_kernel(&self, ordinal: usize) -> &InspectedKernel {
        self.admitted_roster()
            .physical_kernel(ordinal)
            .expect("authenticated roster retains every physical kernel")
    }

    fn descriptor_binding(&self, ordinal: usize) -> KernelDescriptorBinding {
        self.admitted_roster()
            .descriptor_binding(ordinal)
            .expect("authenticated roster retains every descriptor binding")
    }

    fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.revalidate_currentness()
    }
}

/// Why an authenticated roster could not enter one heterogeneous service program set.
#[derive(Debug)]
#[non_exhaustive]
pub enum AuthenticatedWorkerV3ProgramSetAdmissionErrorV1 {
    /// The roster contains no programs.
    EmptyRoster,
    /// The aggregate exceeds the native fixed-dispatch program bound.
    TooManyPrograms {
        /// Aggregate requested program count.
        requested: usize,
        /// Maximum admitted program count.
        maximum: usize,
    },
    /// Every roster in one set must name the same concrete device target.
    TargetMismatch,
    /// A kernel binding occurred in more than one roster.
    DuplicateKernelBinding,
    /// The recovered publication is no longer exactly current.
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
}

impl fmt::Display for AuthenticatedWorkerV3ProgramSetAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "authenticated Worker V3 program-set admission failed: {self:?}"
        )
    }
}

impl Error for AuthenticatedWorkerV3ProgramSetAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::EmptyRoster
            | Self::TooManyPrograms { .. }
            | Self::TargetMismatch
            | Self::DuplicateKernelBinding => None,
        }
    }
}

/// Initial program-set rejection retaining the exact authenticated roster owner.
#[must_use = "a rejected authenticated roster owner must remain classified"]
pub struct AuthenticatedWorkerV3ProgramSetInitialFailureV1<R> {
    error: Box<AuthenticatedWorkerV3ProgramSetAdmissionErrorV1>,
    roster: Box<AuthenticatedWorkerV3RosterV1<R>>,
}

impl<R> AuthenticatedWorkerV3ProgramSetInitialFailureV1<R> {
    /// Returns the exact admission error.
    pub const fn error(&self) -> &AuthenticatedWorkerV3ProgramSetAdmissionErrorV1 {
        &self.error
    }

    /// Returns the error and unchanged roster owner.
    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedWorkerV3ProgramSetAdmissionErrorV1,
        AuthenticatedWorkerV3RosterV1<R>,
    ) {
        (*self.error, *self.roster)
    }
}

impl<R> fmt::Debug for AuthenticatedWorkerV3ProgramSetInitialFailureV1<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3ProgramSetInitialFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Append rejection retaining both the existing set and new roster owner.
#[must_use = "rejected authenticated program custody must remain classified"]
pub struct AuthenticatedWorkerV3ProgramSetAppendFailureV1<R> {
    error: Box<AuthenticatedWorkerV3ProgramSetAdmissionErrorV1>,
    programs: AuthenticatedWorkerV3ProgramSetV1,
    roster: Box<AuthenticatedWorkerV3RosterV1<R>>,
}

impl<R> AuthenticatedWorkerV3ProgramSetAppendFailureV1<R> {
    /// Returns the exact admission error.
    pub const fn error(&self) -> &AuthenticatedWorkerV3ProgramSetAdmissionErrorV1 {
        &self.error
    }

    /// Returns the error, unchanged set, and unchanged roster owner.
    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedWorkerV3ProgramSetAdmissionErrorV1,
        AuthenticatedWorkerV3ProgramSetV1,
        AuthenticatedWorkerV3RosterV1<R>,
    ) {
        (*self.error, self.programs, *self.roster)
    }
}

impl<R> fmt::Debug for AuthenticatedWorkerV3ProgramSetAppendFailureV1<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3ProgramSetAppendFailureV1")
            .field("error", &self.error)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Lookup failure for a generated marker in one authenticated program set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticatedWorkerV3ProgramLookupErrorV1 {
    /// The generated marker is absent from the composed set.
    MarkerNotFound,
}

impl fmt::Display for AuthenticatedWorkerV3ProgramLookupErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generated marker is absent from the authenticated program set")
    }
}

impl Error for AuthenticatedWorkerV3ProgramLookupErrorV1 {}

/// Failure while regenerating structural service programs from authenticated custody.
#[derive(Debug)]
#[non_exhaustive]
pub enum AuthenticatedWorkerV3ProgramMaterializationErrorV1 {
    /// A publication changed after protected roster admission.
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    /// The exact current bytes did not satisfy the closed COV6 loader profile.
    LoadPlan(PlanError),
    /// An expected descriptor entry did not bind to the exact current object.
    KernelClosure(KernelClosureError),
    /// The independently regenerated physical program differed from retained admission.
    PhysicalProgramMismatch,
    /// The descriptor could not supply a complete pointer ABI contract.
    DescriptorContract(&'static str),
    /// The exact descriptor contract contradicted the physical kernel ABI.
    DispatchAbi(KernelDispatchAbiErrorV1),
}

impl fmt::Display for AuthenticatedWorkerV3ProgramMaterializationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "authenticated Worker V3 program materialization failed: {self:?}"
        )
    }
}

impl Error for AuthenticatedWorkerV3ProgramMaterializationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::LoadPlan(_)
            | Self::KernelClosure(_)
            | Self::PhysicalProgramMismatch
            | Self::DescriptorContract(_)
            | Self::DispatchAbi(_) => None,
        }
    }
}

/// Move-only heterogeneous custody for authenticated Worker V3 service programs.
///
/// Program order is roster append order followed by canonical descriptor order
/// within each roster. The owner exposes only counts, target identity, and
/// generated-marker program lookup. Exact bytes, currentness tokens, loader
/// envelopes, descriptors, and roster owners remain private.
#[must_use = "authenticated Worker V3 program custody must remain retained"]
pub struct AuthenticatedWorkerV3ProgramSetV1 {
    rosters: Vec<Box<dyn ErasedAuthenticatedWorkerV3RosterV1>>,
    target: fe2o3_amd_target::AmdTargetId,
    marker_bindings: Vec<[u8; 32]>,
}

impl fmt::Debug for AuthenticatedWorkerV3ProgramSetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3ProgramSetV1")
            .field("roster_count", &self.rosters.len())
            .field("program_count", &self.marker_bindings.len())
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedWorkerV3ProgramSetV1 {
    /// Begins a heterogeneous set with one authenticated roster owner.
    pub fn from_roster<R>(
        roster: AuthenticatedWorkerV3RosterV1<R>,
    ) -> Result<Self, AuthenticatedWorkerV3ProgramSetInitialFailureV1<R>>
    where
        R: CompilerGeneratedKernelExpectationRosterV1 + 'static,
    {
        if let Err(error) = roster.revalidate_currentness() {
            return Err(AuthenticatedWorkerV3ProgramSetInitialFailureV1 {
                error: Box::new(
                    AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::CurrentPublication(error),
                ),
                roster: Box::new(roster),
            });
        }
        let markers = roster
            .verification()
            .entries()
            .iter()
            .map(|entry| entry.marker_binding_identity())
            .collect::<Vec<_>>();
        if let Err(error) = validate_program_append(None, &[], roster.target(), &markers) {
            return Err(AuthenticatedWorkerV3ProgramSetInitialFailureV1 {
                error: Box::new(error),
                roster: Box::new(roster),
            });
        }
        Ok(Self {
            target: roster.target(),
            rosters: vec![Box::new(roster)],
            marker_bindings: markers,
        })
    }

    /// Appends a differently typed authenticated roster in canonical order.
    pub fn append_roster<R>(
        mut self,
        roster: AuthenticatedWorkerV3RosterV1<R>,
    ) -> Result<Self, AuthenticatedWorkerV3ProgramSetAppendFailureV1<R>>
    where
        R: CompilerGeneratedKernelExpectationRosterV1 + 'static,
    {
        if let Err(error) = self.revalidate_rosters() {
            return Err(AuthenticatedWorkerV3ProgramSetAppendFailureV1 {
                error: Box::new(
                    AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::CurrentPublication(error),
                ),
                programs: self,
                roster: Box::new(roster),
            });
        }
        if let Err(error) = roster.revalidate_currentness() {
            return Err(AuthenticatedWorkerV3ProgramSetAppendFailureV1 {
                error: Box::new(
                    AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::CurrentPublication(error),
                ),
                programs: self,
                roster: Box::new(roster),
            });
        }
        let markers = roster
            .verification()
            .entries()
            .iter()
            .map(|entry| entry.marker_binding_identity())
            .collect::<Vec<_>>();
        if let Err(error) = validate_program_append(
            Some(self.target),
            &self.marker_bindings,
            roster.target(),
            &markers,
        ) {
            return Err(AuthenticatedWorkerV3ProgramSetAppendFailureV1 {
                error: Box::new(error),
                programs: self,
                roster: Box::new(roster),
            });
        }
        self.marker_bindings.extend(markers);
        self.rosters.push(Box::new(roster));
        Ok(self)
    }

    /// Returns the number of independently authenticated roster owners.
    pub fn roster_count(&self) -> usize {
        self.rosters.len()
    }

    /// Returns the flattened service-program count.
    pub fn program_count(&self) -> usize {
        self.marker_bindings.len()
    }

    /// Returns the common exact device target.
    pub const fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.target
    }

    /// Resolves one generated marker to its stable flattened program ordinal.
    pub fn program_index<K: CompilerGeneratedKernelExpectationV1>(
        &self,
    ) -> Result<usize, AuthenticatedWorkerV3ProgramLookupErrorV1> {
        self.marker_bindings
            .iter()
            .position(|binding| *binding == K::KERNEL_BINDING_ID_V1)
            .ok_or(AuthenticatedWorkerV3ProgramLookupErrorV1::MarkerNotFound)
    }

    fn revalidate_rosters(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        for roster in &self.rosters {
            roster.revalidate_currentness()?;
        }
        Ok(())
    }

    fn revalidate_currentness(
        &self,
    ) -> Result<(), AuthenticatedWorkerV3ProgramMaterializationErrorV1> {
        self.revalidate_rosters()
            .map_err(AuthenticatedWorkerV3ProgramMaterializationErrorV1::CurrentPublication)
    }

    fn derive_programs(
        &self,
    ) -> Result<Vec<ValidatedKernelEnvelope<'_>>, AuthenticatedWorkerV3ProgramMaterializationErrorV1>
    {
        self.revalidate_currentness()?;
        let mut programs = Vec::with_capacity(self.program_count());
        for roster in &self.rosters {
            let bytes = roster.exact_current_hsaco_bytes();
            for ordinal in 0..roster.entry_count() {
                let descriptor = roster.descriptor(ordinal);
                let physical = roster.physical_kernel(ordinal);
                let binding = roster.descriptor_binding(ordinal);
                let envelope = validate(bytes, AdmittedProfile::Gfx942XnackOffCov6)
                    .map_err(AuthenticatedWorkerV3ProgramMaterializationErrorV1::LoadPlan)?
                    .bind_kernel(descriptor.entry_name().as_str())
                    .map_err(AuthenticatedWorkerV3ProgramMaterializationErrorV1::KernelClosure)?;
                if envelope.selected_kernel() != physical || envelope.selected_binding() != binding
                {
                    return Err(
                        AuthenticatedWorkerV3ProgramMaterializationErrorV1::PhysicalProgramMismatch,
                    );
                }
                let rows =
                    descriptor_dispatch_abi(roster.descriptor_table(), descriptor, physical)?;
                let envelope = envelope
                    .reconcile_dispatch_abi(roster.generated_host_contract(ordinal), &rows)
                    .map_err(AuthenticatedWorkerV3ProgramMaterializationErrorV1::DispatchAbi)?;
                programs.push(envelope);
            }
        }
        self.revalidate_currentness()?;
        Ok(programs)
    }
}

fn validate_program_append(
    existing_target: Option<fe2o3_amd_target::AmdTargetId>,
    existing_markers: &[[u8; 32]],
    incoming_target: fe2o3_amd_target::AmdTargetId,
    incoming_markers: &[[u8; 32]],
) -> Result<(), AuthenticatedWorkerV3ProgramSetAdmissionErrorV1> {
    if incoming_markers.is_empty() {
        return Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::EmptyRoster);
    }
    let requested = existing_markers
        .len()
        .checked_add(incoming_markers.len())
        .ok_or(
            AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::TooManyPrograms {
                requested: usize::MAX,
                maximum: MAX_AUTHENTICATED_SERVICE_PROGRAMS_V1,
            },
        )?;
    if requested > MAX_AUTHENTICATED_SERVICE_PROGRAMS_V1 {
        return Err(
            AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::TooManyPrograms {
                requested,
                maximum: MAX_AUTHENTICATED_SERVICE_PROGRAMS_V1,
            },
        );
    }
    if existing_target.is_some_and(|target| target != incoming_target) {
        return Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::TargetMismatch);
    }
    for (index, marker) in incoming_markers.iter().enumerate() {
        if existing_markers.contains(marker) || incoming_markers[..index].contains(marker) {
            return Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::DuplicateKernelBinding);
        }
    }
    Ok(())
}

fn descriptor_dispatch_abi<'a>(
    table: &'a DeviceDescriptorTableV1,
    descriptor: &'a KernelDescriptorV1,
    physical: &'a InspectedKernel,
) -> Result<Vec<KernelGlobalBufferAbiV1<'a>>, AuthenticatedWorkerV3ProgramMaterializationErrorV1> {
    let mut rows = Vec::new();
    let mut explicit_index = 0usize;
    for argument in descriptor.arguments() {
        let source = table
            .type_records()
            .binary_search_by_key(&argument.source_type(), |record| record.identity())
            .ok()
            .and_then(|index| table.type_records().get(index))
            .ok_or(
                AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                    "source type record",
                ),
            )?;
        let pointee_alignment = u64::from(source.descriptor().scalar_type().alignment_bytes());
        for (kind, offset, _size, _alignment) in argument.physical_components() {
            if kind == PhysicalAbiComponentKind::GlobalPointer {
                let physical_argument = physical.explicit_arguments().get(explicit_index).ok_or(
                    AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                        "physical argument cardinality",
                    ),
                )?;
                let name = physical_argument.name().ok_or(
                    AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                        "physical pointer argument name",
                    ),
                )?;
                let access = match argument.access() {
                    AccessMode::ReadOnly => ArgumentAccess::ReadOnly,
                    AccessMode::WriteOnly => ArgumentAccess::WriteOnly,
                    AccessMode::ReadWrite => ArgumentAccess::ReadWrite,
                    AccessMode::ByValue => {
                        return Err(
                            AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                                "pointer access",
                            ),
                        );
                    }
                };
                rows.push(KernelGlobalBufferAbiV1::new(
                    explicit_index,
                    name,
                    u64::from(offset),
                    pointee_alignment,
                    access,
                ));
            }
            explicit_index = explicit_index.checked_add(1).ok_or(
                AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                    "physical argument index",
                ),
            )?;
        }
    }
    if explicit_index != physical.explicit_arguments().len() {
        return Err(
            AuthenticatedWorkerV3ProgramMaterializationErrorV1::DescriptorContract(
                "physical argument cardinality",
            ),
        );
    }
    Ok(rows)
}

struct AuthenticatedProgramCustodyV1 {
    active: Option<AuthenticatedWorkerV3ProgramSetV1>,
    retired: Vec<AuthenticatedWorkerV3ProgramSetV1>,
}

impl AuthenticatedProgramCustodyV1 {
    fn active(programs: AuthenticatedWorkerV3ProgramSetV1) -> Self {
        Self {
            active: Some(programs),
            retired: Vec::new(),
        }
    }

    fn revalidate_active(&self) -> Result<(), AuthenticatedWorkerV3ProgramMaterializationErrorV1> {
        if let Some(programs) = &self.active {
            programs.revalidate_currentness()?;
        }
        Ok(())
    }

    fn retire_active(&mut self) {
        let active = self
            .active
            .take()
            .expect("attached authenticated queue retains one active program set");
        self.retired.push(active);
    }

    fn install_active(&mut self, programs: AuthenticatedWorkerV3ProgramSetV1) {
        debug_assert!(self.active.is_none());
        self.active = Some(programs);
    }

    fn take_most_recent_retired(&mut self) -> AuthenticatedWorkerV3ProgramSetV1 {
        debug_assert!(self.active.is_none());
        self.retired
            .pop()
            .expect("unbound authenticated queue retains a detached program set")
    }

    fn restore_most_recent_retired(&mut self, programs: AuthenticatedWorkerV3ProgramSetV1) {
        debug_assert!(self.active.is_none());
        self.retired.push(programs);
    }

    fn into_program_sets(mut self) -> Vec<AuthenticatedWorkerV3ProgramSetV1> {
        if let Some(active) = self.active.take() {
            self.retired.push(active);
        }
        self.retired
    }
}

impl fmt::Debug for AuthenticatedProgramCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedProgramCustodyV1")
            .field("has_active", &self.active.is_some())
            .field("retired_set_count", &self.retired.len())
            .finish_non_exhaustive()
    }
}

/// Program owners retained after a terminal native rollover consumed the queue.
///
/// This owner is intentionally opaque: it carries no executable, descriptor,
/// token, address, or queue authority. Consuming it only releases the original
/// authenticated program-set owners after the caller has classified the
/// terminal failure.
#[must_use = "terminal authenticated program custody must remain classified"]
pub struct AuthenticatedServiceTerminalProgramCustodyV1 {
    programs: AuthenticatedProgramCustodyV1,
}

impl AuthenticatedServiceTerminalProgramCustodyV1 {
    /// Returns every retained authenticated program-set owner.
    pub fn into_program_sets(self) -> Vec<AuthenticatedWorkerV3ProgramSetV1> {
        self.programs.into_program_sets()
    }
}

impl fmt::Debug for AuthenticatedServiceTerminalProgramCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceTerminalProgramCustodyV1")
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Queue creation failure retaining every recoverable authenticated input.
#[must_use = "authenticated queue creation failure retains program custody"]
pub enum AuthenticatedServiceQueueCreateFailureV1<const N: usize> {
    /// Authenticated bytes or descriptors failed before KFD transfer.
    Program {
        /// Exact program derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged authenticated program set.
        programs: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged allocation owner.
        allocations: Box<ServiceAllocationSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Structural queue validation rejected unchanged inputs.
    QueueRejected {
        /// Exact structural queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged authenticated program set.
        programs: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged allocation owner.
        allocations: Box<ServiceAllocationSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Native creation consumed structural inputs before failing terminally.
    QueueTerminal {
        /// Exact terminal queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Authenticated program custody, never transferred to KFD.
        programs: AuthenticatedWorkerV3ProgramSetV1,
    },
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueCreateFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => formatter.debug_tuple("Program").field(error).finish(),
            Self::QueueRejected { error, .. } => {
                formatter.debug_tuple("QueueRejected").field(error).finish()
            }
            Self::QueueTerminal { error, .. } => {
                formatter.debug_tuple("QueueTerminal").field(error).finish()
            }
        }
    }
}

/// Prepared authenticated custody of one persistent KFD fixed-dispatch queue.
#[must_use = "the authenticated live queue requires an explicit linear transition"]
pub struct AuthenticatedServiceQueueSessionV1<const N: usize> {
    queue: ServiceQueueSessionV1<N>,
    programs: AuthenticatedProgramCustodyV1,
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.queue)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> AuthenticatedServiceQueueSessionV1<N> {
    /// Creates a persistent queue only from authenticated program custody and addressless packets.
    pub fn create(
        programs: AuthenticatedWorkerV3ProgramSetV1,
        allocations: ServiceAllocationSessionV1,
        ring_bytes: u32,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Result<Self, AuthenticatedServiceQueueCreateFailureV1<N>> {
        let derived = match programs.derive_programs() {
            Ok(derived) => derived,
            Err(error) => {
                return Err(AuthenticatedServiceQueueCreateFailureV1::Program {
                    error: Box::new(error),
                    programs,
                    allocations: Box::new(allocations),
                    packets: Box::new(packets),
                });
            }
        };
        let batch = ServiceFixedBatchV1::new(derived, packets);
        match ServiceQueueSessionV1::create(allocations, ring_bytes, batch) {
            Ok(queue) => Ok(Self {
                queue,
                programs: AuthenticatedProgramCustodyV1::active(programs),
            }),
            Err(ServiceQueueCreateFailureV1::Rejected {
                error,
                allocations,
                batch,
            }) => {
                let (_derived, packets) = (*batch).into_parts();
                Err(AuthenticatedServiceQueueCreateFailureV1::QueueRejected {
                    error: Box::new(error),
                    programs,
                    allocations,
                    packets: Box::new(packets),
                })
            }
            Err(ServiceQueueCreateFailureV1::Terminal { error }) => {
                Err(AuthenticatedServiceQueueCreateFailureV1::QueueTerminal {
                    error: Box::new(error),
                    programs,
                })
            }
        }
    }

    /// Returns a redacted native queue observation.
    pub const fn observation(&self) -> fe2o3_kfd::ComputeAqlQueueObservationV1 {
        self.queue.observation()
    }

    /// Revalidates the active publication and publishes the exact fixed batch once.
    pub fn submit(
        self,
    ) -> Result<
        AuthenticatedServicePublishedQueueSessionV1<N>,
        AuthenticatedServiceQueueSubmitFailureV1<N>,
    > {
        if let Err(error) = self.programs.revalidate_active() {
            return Err(AuthenticatedServiceQueueSubmitFailureV1::Currentness {
                error: Box::new(error),
                retained: Box::new(self),
            });
        }
        let Self { queue, programs } = self;
        match queue.submit() {
            Ok(queue) => Ok(AuthenticatedServicePublishedQueueSessionV1 { queue, programs }),
            Err(inner) => Err(AuthenticatedServiceQueueSubmitFailureV1::Queue(Box::new(
                AuthenticatedServiceQueueOperationFailureV1 {
                    inner: Box::new(inner),
                    programs,
                },
            ))),
        }
    }

    /// Destroys the unpublished prepared queue and returns released program custody.
    ///
    /// This transition never publishes a packet. It is therefore valid for the
    /// unchanged owner returned by a submit-currentness rejection.
    ///
    /// ```compile_fail
    /// use fe2o3_host::AuthenticatedServiceQueueSessionV1;
    ///
    /// fn destroy_twice(queue: AuthenticatedServiceQueueSessionV1<1>) {
    ///     let _first = queue.destroy_and_release();
    ///     let _second = queue.destroy_and_release();
    /// }
    /// ```
    pub fn destroy_and_release(
        self,
    ) -> Result<AuthenticatedServiceQueueReleaseV1, AuthenticatedServiceQueueReleaseFailureV1> {
        let Self { queue, programs } = self;
        finish_release(queue.destroy_and_release(), programs)
    }
}

/// Publication failure retaining either retryable prepared or quarantined custody.
#[must_use = "publication failure retains authenticated queue custody"]
pub enum AuthenticatedServiceQueueSubmitFailureV1<const N: usize> {
    /// Currentness failed before packet publication; the prepared queue is unchanged.
    Currentness {
        /// Exact currentness or derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged prepared authenticated queue.
        retained: Box<AuthenticatedServiceQueueSessionV1<N>>,
    },
    /// The lower queue transition became quarantined.
    Queue(Box<AuthenticatedServiceQueueOperationFailureV1>),
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueSubmitFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Currentness { error, .. } => {
                formatter.debug_tuple("Currentness").field(error).finish()
            }
            Self::Queue(failure) => formatter.debug_tuple("Queue").field(failure).finish(),
        }
    }
}

/// Published authenticated custody of one exact queue generation.
#[must_use = "published authenticated queue custody must be completed"]
pub struct AuthenticatedServicePublishedQueueSessionV1<const N: usize> {
    queue: ServicePublishedQueueSessionV1<N>,
    programs: AuthenticatedProgramCustodyV1,
}

fn route_authenticated_wait_custody<T, E>(
    result: Result<T, E>,
    programs: AuthenticatedProgramCustodyV1,
) -> Result<(T, AuthenticatedProgramCustodyV1), (E, AuthenticatedProgramCustodyV1)> {
    match result {
        Ok(queue) => Ok((queue, programs)),
        Err(error) => Err((error, programs)),
    }
}

impl<const N: usize> fmt::Debug for AuthenticatedServicePublishedQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServicePublishedQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> AuthenticatedServicePublishedQueueSessionV1<N> {
    /// Polls once without releasing any authenticated roster owner.
    pub fn poll(
        self,
    ) -> Result<AuthenticatedServiceQueuePollV1<N>, AuthenticatedServiceQueueOperationFailureV1>
    {
        let Self { queue, programs } = self;
        match queue.poll() {
            Ok(ServiceQueuePollV1::Pending(queue)) => {
                Ok(AuthenticatedServiceQueuePollV1::Pending(Self {
                    queue,
                    programs,
                }))
            }
            Ok(ServiceQueuePollV1::Ready(queue)) => Ok(AuthenticatedServiceQueuePollV1::Ready(
                AuthenticatedServiceCompletedQueueSessionV1 { queue, programs },
            )),
            Err(inner) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }

    /// Polls once and retains the same-scan addressless progress observation.
    pub fn poll_with_progress(
        self,
    ) -> Result<
        AuthenticatedServiceQueuePollWithProgressV1<N>,
        AuthenticatedServiceQueueOperationFailureV1,
    > {
        let Self { queue, programs } = self;
        match queue.poll_with_progress() {
            Ok(ServiceQueuePollWithProgressV1::Pending { session, progress }) => {
                Ok(AuthenticatedServiceQueuePollWithProgressV1::Pending {
                    session: Self {
                        queue: session,
                        programs,
                    },
                    progress,
                })
            }
            Ok(ServiceQueuePollWithProgressV1::Ready { session, progress }) => {
                Ok(AuthenticatedServiceQueuePollWithProgressV1::Ready {
                    session: AuthenticatedServiceCompletedQueueSessionV1 {
                        queue: session,
                        programs,
                    },
                    progress,
                })
            }
            Err(inner) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }

    /// Waits with a bounded poll count while retaining every program owner.
    pub fn wait(
        self,
        polls: u32,
    ) -> Result<
        AuthenticatedServiceCompletedQueueSessionV1<N>,
        AuthenticatedServiceQueueOperationFailureV1,
    > {
        let Self { queue, programs } = self;
        match route_authenticated_wait_custody(queue.wait(polls), programs) {
            Ok((queue, programs)) => {
                Ok(AuthenticatedServiceCompletedQueueSessionV1 { queue, programs })
            }
            Err((inner, programs)) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }

    /// Waits until KFD's monotonic relative millisecond deadline while
    /// retaining every authenticated program owner.
    ///
    /// A timeout or any other terminal wait failure preserves the program
    /// owners beside the lower opaque quarantined queue.
    pub fn wait_for(
        self,
        timeout_milliseconds: u32,
    ) -> Result<
        AuthenticatedServiceCompletedQueueSessionV1<N>,
        AuthenticatedServiceQueueOperationFailureV1,
    > {
        let Self { queue, programs } = self;
        match route_authenticated_wait_custody(queue.wait_for(timeout_milliseconds), programs) {
            Ok((queue, programs)) => {
                Ok(AuthenticatedServiceCompletedQueueSessionV1 { queue, programs })
            }
            Err((inner, programs)) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }
}

/// One authenticated nonblocking completion result.
#[derive(Debug)]
pub enum AuthenticatedServiceQueuePollV1<const N: usize> {
    /// Completion remains pending.
    Pending(AuthenticatedServicePublishedQueueSessionV1<N>),
    /// Every signal is complete.
    Ready(AuthenticatedServiceCompletedQueueSessionV1<N>),
}

/// Authenticated queue custody paired with same-scan progress.
#[derive(Debug)]
pub enum AuthenticatedServiceQueuePollWithProgressV1<const N: usize> {
    /// Completion remains pending.
    Pending {
        /// Returned published custody.
        session: AuthenticatedServicePublishedQueueSessionV1<N>,
        /// Addressless progress from the consuming poll.
        progress: ServiceQueueProgressV1,
    },
    /// Every signal is complete.
    Ready {
        /// Returned completed custody.
        session: AuthenticatedServiceCompletedQueueSessionV1<N>,
        /// Addressless progress from the consuming poll.
        progress: ServiceQueueProgressV1,
    },
}

/// Completed authenticated custody before exact signal recycle.
#[must_use = "completed authenticated queue custody must be recycled"]
pub struct AuthenticatedServiceCompletedQueueSessionV1<const N: usize> {
    queue: ServiceCompletedQueueSessionV1<N>,
    programs: AuthenticatedProgramCustodyV1,
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceCompletedQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceCompletedQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> AuthenticatedServiceCompletedQueueSessionV1<N> {
    /// Recycles every completed signal while retaining program custody.
    pub fn recycle(
        self,
    ) -> Result<
        AuthenticatedServiceRecycledQueueSessionV1<N>,
        AuthenticatedServiceQueueOperationFailureV1,
    > {
        let Self { queue, programs } = self;
        match queue.recycle() {
            Ok(queue) => Ok(AuthenticatedServiceRecycledQueueSessionV1 { queue, programs }),
            Err(inner) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }
}

/// Recycled authenticated queue custody at a quiescent fixed-batch boundary.
#[must_use = "recycled authenticated queue custody must be reused, detached, or released"]
pub struct AuthenticatedServiceRecycledQueueSessionV1<const N: usize> {
    queue: ServiceRecycledQueueSessionV1<N>,
    programs: AuthenticatedProgramCustodyV1,
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceRecycledQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceRecycledQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> AuthenticatedServiceRecycledQueueSessionV1<N> {
    /// Creates a generation-bound coherent read request.
    pub const fn completed_read_request(
        &self,
        range: fe2o3_service_host::ServiceHostDispatchRangeV1,
    ) -> ServiceCompletedReadRequestV1 {
        self.queue.completed_read_request(range)
    }

    /// Creates a generation-bound enclosing-snapshot request.
    pub const fn completed_snapshot_request(
        &self,
        range: fe2o3_service_host::ServiceHostDispatchSnapshotRangeV1,
    ) -> ServiceCompletedSnapshotRequestV1 {
        self.queue.completed_snapshot_request(range)
    }

    /// Reads one completed coherent range without releasing program custody.
    pub fn read_completed(
        &mut self,
        request: ServiceCompletedReadRequestV1,
    ) -> Result<ServiceCompletedReadbackV1, ServiceQueueErrorV1> {
        self.queue.read_completed(request)
    }

    /// Reads one completed enclosing snapshot without releasing program custody.
    pub fn read_completed_snapshot(
        &mut self,
        request: ServiceCompletedSnapshotRequestV1,
    ) -> Result<ServiceCompletedReadbackV1, ServiceQueueErrorV1> {
        self.queue.read_completed_snapshot(request)
    }

    /// Revalidates the active roster before making the attached batch publishable again.
    pub fn reuse(
        self,
    ) -> Result<AuthenticatedServiceQueueSessionV1<N>, AuthenticatedServiceCurrentnessFailureV1<Self>>
    {
        if let Err(error) = self.programs.revalidate_active() {
            return Err(AuthenticatedServiceCurrentnessFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            });
        }
        let Self { queue, programs } = self;
        Ok(AuthenticatedServiceQueueSessionV1 {
            queue: queue.reuse(),
            programs,
        })
    }

    /// Detaches the fixed batch after completion and retains its roster owners as history.
    pub fn detach(
        self,
    ) -> Result<
        AuthenticatedServiceQueueUnboundSessionV1,
        AuthenticatedServiceQueueOperationFailureV1,
    > {
        let Self {
            queue,
            mut programs,
        } = self;
        match queue.detach() {
            Ok(queue) => {
                programs.retire_active();
                Ok(AuthenticatedServiceQueueUnboundSessionV1 { queue, programs })
            }
            Err(inner) => Err(AuthenticatedServiceQueueOperationFailureV1 {
                inner: Box::new(inner),
                programs,
            }),
        }
    }

    /// Destroys the quiescent queue and returns released program-set custody.
    pub fn destroy_and_release(
        self,
    ) -> Result<AuthenticatedServiceQueueReleaseV1, AuthenticatedServiceQueueReleaseFailureV1> {
        let Self { queue, programs } = self;
        finish_release(queue.destroy_and_release(), programs)
    }
}

include!("authenticated_service_queue/lifecycle.rs");

include!("authenticated_service_queue/tests.rs");
