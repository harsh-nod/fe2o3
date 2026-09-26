//! Explicit success-gated typed launches and their Context-owned dependency roots.

use super::peer_custody::ScalarPeerDependencyV1;
use super::peer_reconciliation::DirectedPeerStateV1;
use super::*;

/// An exact event/producer pair in original dependency order, not launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendLaunchProducerV1 {
    pub event: u64,
    pub producer_submission: u64,
}

/// Ordinary typed launch with an exact success-gated producer roster.
/// Atomic and collective authorization remain separate backend contracts.
#[derive(Clone, Copy, Debug)]
pub struct BackendProducerAwareLaunchV1<'a> {
    pub stream: u64,
    pub kernel: u64,
    pub explicit_kernarg: &'a [u8],
    pub bindings: &'a [BackendBindingV1],
    pub dependencies: &'a [BackendLaunchProducerV1],
    pub geometry: RuntimeLaunchGeometryV1,
}

/// Opt-in ordinary launch contract; stable capability bits cannot enable it.
///
/// Before any effect, authenticate every event against its expected producer and
/// reject aliases naming one producer twice. Retain producer and launch resources
/// independently of public events. Publish a consumer only after every explicit
/// producer succeeded; failure, cancellation and quiescence without a result are
/// not success. Implicit stream ordering alone does not satisfy this contract.
/// All ordinary failure, uncertain-custody and completion contracts still apply.
/// Poll/wait/flush use the existing backend paths; this adds no alternate executor,
/// semantic authority, Worker transport or formal-refinement claim.
pub trait RuntimeProducerAwareLaunchBackendV1: RuntimeBackendV1 {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

pub(super) enum PreparedSubmissionCustodyV1 {
    Peer(PreparedPeerSubmissionV1),
    Launch(ProducerLaunchRootV1),
}

pub(super) struct ProducerLaunchRootV1 {
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) backend_stream: u64,
    kernel: u64,
    module: RuntimeModuleIdV1,
    module_record: ModuleRecordV1,
    pub(super) bindings: Vec<ContextReadSourceV1>,
    pub(super) sources: Vec<ContextReadSourceV1>,
    pub(super) destinations: Vec<RuntimeAllocationIdV1>,
    pub(super) dependencies: Vec<ScalarPeerDependencyV1>,
    pub(super) backend_submission: Option<u64>,
    pub(super) dependencies_held: bool,
    pub(super) state: DirectedPeerStateV1,
}

impl ProducerLaunchRootV1 {
    /// Checked interval union over one allocation's original writable bindings.
    pub(super) fn covers_input_v1(&self, source: ContextReadSourceV1) -> bool {
        let Some(end) = source
            .region
            .byte_offset
            .checked_add(source.region.byte_len)
        else {
            return false;
        };
        let mut cursor = source.region.byte_offset;
        for binding in &self.bindings {
            if binding.region.allocation != source.region.allocation
                || binding.record != source.record
                || binding.region.access == RuntimeAccessV1::Read
            {
                continue;
            }
            let Some(bound) = binding
                .region
                .byte_offset
                .checked_add(binding.region.byte_len)
            else {
                return false;
            };
            if binding.region.byte_offset > cursor {
                return false;
            }
            cursor = cursor.max(bound);
            if cursor >= end {
                return true;
            }
        }
        false
    }
}

impl<B: RuntimeProducerAwareLaunchBackendV1> RuntimeContextV1<B> {
    /// Queue a typed consumer of exact earlier producer outputs without a graph reservation.
    ///
    /// Requires a version journal and producers admitted through this same profile.
    /// Pure reads retain whole-allocation custody; every original pending read
    /// range must be covered by its named producer's writable ranges. Writable aliases
    /// with pending predecessors reject. Events must remain live until admission;
    /// afterwards Context retains producers independently of those public events.
    /// Device completion may precede logical completion while producers reconcile.
    pub fn launch_producer_aware_v1<A: RuntimeArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        arguments: &A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        self.launch_producer_request_v1(ContextLaunchRequestV1 {
            stream,
            kernel,
            arguments: ContextLaunchArgumentsV1::Live(arguments),
            geometry,
            dependencies,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn launch_producer_snapshot_v1<A: RuntimeArgumentsV1>(
        &mut self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        bytes: &[u8],
        bindings: &[RuntimeBindingV1],
        geometry: RuntimeLaunchGeometryV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        self.launch_producer_request_v1(ContextLaunchRequestV1 {
            stream,
            kernel,
            arguments: ContextLaunchArgumentsV1::Frozen(bytes, bindings),
            geometry,
            dependencies,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        })
    }

    fn launch_producer_request_v1<A: RuntimeArgumentsV1>(
        &mut self,
        request: ContextLaunchRequestV1<'_, A>,
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        if self.versions.is_none() {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        let dependencies = request.dependencies;
        let mut prepared = self.prepare_context_launch_profile_v1(request, None, true)?;
        let root = self.prepare_producer_launch_root_v1(&mut prepared, dependencies)?;
        let mut roster = [BackendLaunchProducerV1 {
            event: 0,
            producer_submission: 0,
        }; MAX_RUNTIME_DEPENDENCIES_V1];
        for dependency in &root.dependencies {
            roster[dependency.ordinal] = BackendLaunchProducerV1 {
                event: dependency.backend_event,
                producer_submission: dependency.backend_submission,
            };
        }
        let roster = &roster[..root.dependencies.len()];
        self.submit_prepared_launch_with_custody_v1(
            prepared,
            None,
            Some(PreparedSubmissionCustodyV1::Launch(root)),
            |backend, launch| {
                backend.submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                    stream: launch.stream,
                    kernel: launch.kernel,
                    explicit_kernarg: launch.explicit_kernarg,
                    bindings: launch.bindings,
                    dependencies: roster,
                    geometry: launch.geometry,
                })
            },
        )
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn prepare_producer_launch_root_v1(
        &mut self,
        prepared: &mut PreparedContextLaunchV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<ProducerLaunchRootV1, RuntimeValidationErrorV1> {
        let mut roster = self.prepare_dependency_roster_v1(dependencies)?;
        roster.sort_unstable_by_key(|dependency| (dependency.submission, dependency.event));
        let mut depth = 1;
        for (index, dependency) in roster.iter().enumerate() {
            if index > 0 && roster[index - 1].submission == dependency.submission {
                return Err(RuntimeValidationErrorV1::DuplicateDependency);
            }
            if dependency.device != prepared.stream_record.device {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            self.check_operation_custody_v1(dependency.submission)?;
            let producer = self
                .producer_launches
                .get(&dependency.submission)
                .ok_or(RuntimeValidationErrorV1::Unsupported)?;
            if !matches!(
                self.submissions[&dependency.submission].status,
                RuntimeCompletionStatusV1::Pending | RuntimeCompletionStatusV1::Succeeded
            ) {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            depth = depth.max(
                producer
                    .state
                    .depth
                    .checked_add(1)
                    .ok_or(RuntimeValidationErrorV1::Capacity)?,
            );
        }
        if depth > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies);
        }
        self.producer_launches
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let module = self.kernels[&prepared.kernel].module;
        let mut bindings = core::mem::take(&mut prepared.producer_bindings);
        bindings.sort_unstable_by_key(|binding| {
            (
                binding.region.allocation,
                binding.region.byte_offset,
                binding.region.byte_len,
            )
        });
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(prepared.journal_sources.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        sources.extend_from_slice(&prepared.journal_sources);
        let mut destinations = Vec::new();
        destinations
            .try_reserve_exact(prepared.journal_destinations.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        destinations.extend_from_slice(&prepared.journal_destinations);
        Ok(ProducerLaunchRootV1 {
            stream: prepared.stream,
            backend_stream: prepared.stream_record.backend_stream,
            kernel: prepared.kernel,
            module,
            module_record: self.modules[&module],
            bindings,
            sources,
            destinations,
            dependencies: roster,
            backend_submission: None,
            dependencies_held: true,
            state: DirectedPeerStateV1 {
                depth,
                cursor: 0,
                terminal: None,
            },
        })
    }

    pub(super) fn begin_producer_launch_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        root: ProducerLaunchRootV1,
    ) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            assert!(
                !self.producer_launches.contains_key(&id),
                "fresh launch root"
            );
            assert!(
                self.producer_launches.len() < self.producer_launches.capacity(),
                "preallocated launch root"
            );
            self.producer_launches.insert(id, root);
            for dependency in &self.producer_launches[&id].dependencies {
                let producer = self
                    .submissions
                    .get_mut(&dependency.submission)
                    .expect("preflighted producer");
                producer.dependency_retains = producer
                    .dependency_retains
                    .checked_add(1)
                    .expect("preflighted retain count");
            }
        }));
        if let Err(payload) = result {
            self.quarantine_after_async_command_panic_v1();
            std::panic::resume_unwind(payload);
        }
    }

    pub(super) fn validate_producer_launch_custody_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        let record = self.submissions.get(&id);
        let Some(root) = self.producer_launches.get(&id) else {
            return if record.is_some_and(|record| record.producer_launch) {
                Err(invalid)
            } else {
                Ok(())
            };
        };
        if self.scalar_peer_copies.contains_key(&id)
            || self.versions.is_none()
            || id.context_generation != self.context_generation
            || root.stream.context_generation != self.context_generation
            || root.backend_stream == 0
            || root.kernel == 0
            || root.module.context_generation != self.context_generation
            || root.state.depth == 0
            || root.state.depth > MAX_RUNTIME_DEPENDENCIES_V1
            || root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || root.state.cursor > root.dependencies.len()
            || root.state.terminal == Some(BackendPollV1::Pending)
            || root.state.terminal.is_none() && root.state.cursor != 0
            || root.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
        {
            return Err(invalid);
        }
        match record {
            Some(record)
                if record.producer_launch
                    && !record.scalar_peer_copy
                    && !record.directed_peer_copy
                    && root.backend_submission == Some(record.backend_submission)
                    && record.stream == root.stream
                    && record.device == root.module_record.device
                    && root.dependencies_held != record.quiescent => {}
            None if root.backend_submission.is_none() && root.dependencies_held => {}
            _ => return Err(invalid),
        }
        if !root.dependencies_held {
            return Ok(());
        }
        if self
            .kernels
            .get(&root.kernel)
            .is_none_or(|record| record.module != root.module)
            || self.modules.get(&root.module).is_none_or(|record| {
                record.backend_module != root.module_record.backend_module
                    || record.device != root.module_record.device
                    || record.image_sha256 != root.module_record.image_sha256
            })
        {
            return Err(invalid);
        }
        let mut previous = None;
        for binding in &root.bindings {
            let key = (
                binding.region.allocation,
                binding.region.byte_offset,
                binding.region.byte_len,
            );
            if previous.is_some_and(|previous| previous > key)
                || binding.region.byte_len == 0
                || binding.record.device != root.module_record.device
                || binding
                    .region
                    .byte_offset
                    .checked_add(binding.region.byte_len)
                    .is_none_or(|end| end > binding.record.byte_len)
                || self.allocations.get(&binding.region.allocation) != Some(&binding.record)
                || !self
                    .backend_allocations
                    .contains(&binding.record.backend_allocation)
                || !self.allocation_admission.has_expected_credit(
                    binding.region.allocation,
                    binding.record.device,
                    binding.record.byte_len,
                )
            {
                return Err(invalid);
            }
            previous = Some(key);
        }
        if root.sources.len() > root.bindings.len()
            || root.destinations.len() > root.bindings.len()
            || root
                .sources
                .windows(2)
                .any(|pair| pair[0].region.allocation >= pair[1].region.allocation)
            || root.destinations.windows(2).any(|pair| pair[0] >= pair[1])
            || root.sources.iter().any(|source| {
                source.region.access != RuntimeAccessV1::Read
                    || source.region.byte_offset != 0
                    || source.region.byte_len != source.record.byte_len
                    || root
                        .destinations
                        .binary_search(&source.region.allocation)
                        .is_ok()
                    || !root.bindings.iter().any(|binding| {
                        binding.region.allocation == source.region.allocation
                            && binding.record == source.record
                    })
            })
            || root.destinations.iter().any(|id| {
                !root.bindings.iter().any(|binding| {
                    binding.region.allocation == *id
                        && binding.region.access != RuntimeAccessV1::Read
                })
            })
            || root.bindings.iter().any(|binding| {
                let writes = root
                    .destinations
                    .binary_search(&binding.region.allocation)
                    .is_ok();
                if binding.region.access != RuntimeAccessV1::Read {
                    !writes
                } else {
                    !writes
                        && root
                            .sources
                            .binary_search_by_key(&binding.region.allocation, |source| {
                                source.region.allocation
                            })
                            .is_err()
                }
            })
        {
            return Err(invalid);
        }
        let mut ordinals = [false; MAX_RUNTIME_DEPENDENCIES_V1];
        let mut previous = None;
        let mut depth = 1;
        for (index, dependency) in root.dependencies.iter().enumerate() {
            let producer = self
                .submissions
                .get(&dependency.submission)
                .ok_or(invalid)?;
            let parent = self
                .producer_launches
                .get(&dependency.submission)
                .ok_or(invalid)?;
            if previous.is_some_and(|previous| previous >= dependency.submission)
                || dependency.ordinal >= root.dependencies.len()
                || ordinals[dependency.ordinal]
                || dependency.event.context_generation != self.context_generation
                || dependency.backend_event == 0
                || dependency.submission.context_generation != self.context_generation
                || dependency.submission.local >= id.local
                || dependency.device != root.module_record.device
                || producer.device != dependency.device
                || producer.stream != dependency.stream
                || producer.backend_submission != dependency.backend_submission
                || !producer.producer_launch
                || producer.dependency_retains == 0
                || !self
                    .backend_submissions
                    .contains(&dependency.backend_submission)
                || parent.state.depth == 0
                || parent.state.depth >= root.state.depth
                || index < root.state.cursor
                    && producer.status != RuntimeCompletionStatusV1::Succeeded
            {
                return Err(invalid);
            }
            ordinals[dependency.ordinal] = true;
            previous = Some(dependency.submission);
            depth = depth.max(parent.state.depth + 1);
        }
        if depth != root.state.depth {
            return Err(invalid);
        }
        Ok(())
    }

    pub(super) fn check_operation_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = self
            .validate_scalar_peer_custody_v1(id)
            .and_then(|()| self.validate_producer_launch_custody_v1(id));
        if result.is_err() {
            self.quarantine_after_async_command_panic_v1();
        }
        result
    }

    pub(super) fn release_operation_dependencies_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.check_operation_custody_v1(id)?;
        self.release_scalar_peer_dependencies_v1(id)?;
        if let Some(root) = self.producer_launches.get_mut(&id)
            && root.dependencies_held
        {
            for dependency in &root.dependencies {
                self.submissions
                    .get_mut(&dependency.submission)
                    .expect("retained producer")
                    .dependency_retains -= 1;
            }
            root.dependencies_held = false;
        }
        Ok(())
    }

    pub(super) fn producer_launch_retains_module_v1(&self, module: RuntimeModuleIdV1) -> bool {
        self.producer_launches
            .values()
            .any(|root| root.dependencies_held && root.module == module)
    }
}
