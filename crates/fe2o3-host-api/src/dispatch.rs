//! Bounded finite and persistent-task dispatch contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, DiagnosticSeverityV1, HostRequestIdentityV1, HostResultIdentityV1,
    HostResultReferenceV1, OperationResultClassV1, check_preimage_bound, encode_diagnostics,
    validate_diagnostics, validate_strictly_ordered,
};
use crate::{
    ArgumentSetIdV1, CompletionSignalIdV1, DispatchContractIdV1, DispatchRequestIdV1,
    DispatchResultIdV1, DispatchSubmissionIdV1, EntryPointIdV1, HostContractErrorV1,
    HostDiagnosticV1, LoadOutcomeV1, LoadResultIdV1, LoadResultV1, LoadedObjectIdV1,
    OperationContextV1, ResourceBindingV1, ServiceInstanceIdV1, TaskSchemaIdV1,
};

/// Hard maximum resource-binding count in one dispatch request.
pub const MAX_DISPATCH_BINDINGS_V1: usize = 128;
/// Hard maximum predecessor-completion count in one dispatch request.
pub const MAX_DISPATCH_DEPENDENCIES_V1: usize = 128;

/// One explicit completion dependency of a dispatch request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DispatchDependencyV1 {
    completion_signal_identity: CompletionSignalIdV1,
    producer_submission_identity: DispatchSubmissionIdV1,
}

impl DispatchDependencyV1 {
    /// Creates an inert dependency binding.
    pub const fn new(
        completion_signal_identity: CompletionSignalIdV1,
        producer_submission_identity: DispatchSubmissionIdV1,
    ) -> Self {
        Self {
            completion_signal_identity,
            producer_submission_identity,
        }
    }

    /// Returns the predecessor completion-signal commitment.
    pub const fn completion_signal_identity(self) -> CompletionSignalIdV1 {
        self.completion_signal_identity
    }

    /// Returns the predecessor submission commitment.
    pub const fn producer_submission_identity(self) -> DispatchSubmissionIdV1 {
        self.producer_submission_identity
    }

    fn encode(self, encoder: &mut EncoderV1) {
        encoder.digest(self.completion_signal_identity.digest());
        encoder.digest(self.producer_submission_identity.digest());
    }
}

/// Execution shape described by one dispatch request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchKindV1 {
    /// One finite kernel dispatch.
    Finite,
    /// One task submitted to a separately described issue #135 service.
    PersistentTask {
        /// Persistent service-instance commitment.
        service_instance_identity: ServiceInstanceIdV1,
        /// Closed task-schema commitment.
        task_schema_identity: TaskSchemaIdV1,
        /// Canonical tag within the closed schema.
        task_tag: u32,
        /// Service epoch preventing cross-restart task confusion.
        service_epoch: u64,
    },
}

/// Result disposition of one dispatch-description request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchOutcomeV1 {
    /// A runtime adapter reported submission of the described operation.
    Submitted {
        /// Commitment to the exact submission description.
        submission_identity: DispatchSubmissionIdV1,
        /// Commitment wait/dependency records may name; not a runtime handle.
        completion_signal_identity: CompletionSignalIdV1,
    },
    /// The dispatch description was rejected before submission.
    Rejected,
    /// The described dispatch operation failed before submission.
    Failed,
}

/// Complete runtime-neutral V1 dispatch request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchRequestV1 {
    identity: DispatchRequestIdV1,
    context: OperationContextV1,
    load_result_identity: LoadResultIdV1,
    loaded_object_identity: LoadedObjectIdV1,
    load_generation: u64,
    entry_point_identity: EntryPointIdV1,
    dispatch_contract_identity: DispatchContractIdV1,
    argument_set_identity: ArgumentSetIdV1,
    kind: DispatchKindV1,
    bindings: Vec<ResourceBindingV1>,
    dependencies: Vec<DispatchDependencyV1>,
}

impl DispatchRequestV1 {
    /// Creates a request bound to an exact successful load result.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DispatchRequestIdV1,
        context: OperationContextV1,
        load_result: &LoadResultV1,
        entry_point_identity: EntryPointIdV1,
        dispatch_contract_identity: DispatchContractIdV1,
        argument_set_identity: ArgumentSetIdV1,
        kind: DispatchKindV1,
        bindings: Vec<ResourceBindingV1>,
        dependencies: Vec<DispatchDependencyV1>,
    ) -> Result<Self, HostContractErrorV1> {
        if bindings.len() > MAX_DISPATCH_BINDINGS_V1 {
            return Err(HostContractErrorV1::TooManyItems {
                field: ContractFieldV1::DispatchBindings,
                actual: bindings.len(),
                maximum: MAX_DISPATCH_BINDINGS_V1,
            });
        }
        for (expected, binding) in bindings.iter().enumerate() {
            if usize::from(binding.ordinal()) != expected {
                return Err(HostContractErrorV1::NonCanonicalOrder {
                    field: ContractFieldV1::DispatchBindings,
                });
            }
        }
        validate_strictly_ordered(
            &dependencies,
            MAX_DISPATCH_DEPENDENCIES_V1,
            ContractFieldV1::DispatchDependencies,
        )?;
        if dependencies
            .windows(2)
            .any(|pair| pair[0].completion_signal_identity == pair[1].completion_signal_identity)
        {
            return Err(HostContractErrorV1::Duplicate {
                field: ContractFieldV1::DispatchDependencies,
            });
        }
        if let DispatchKindV1::PersistentTask {
            service_epoch: 0, ..
        } = kind
        {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::ServiceEpoch,
            });
        }
        let LoadOutcomeV1::Loaded {
            loaded_object_identity,
            load_generation,
        } = load_result.outcome()
        else {
            return Err(HostContractErrorV1::InvalidOutcome);
        };
        Ok(Self {
            identity,
            context,
            load_result_identity: load_result.identity(),
            loaded_object_identity,
            load_generation,
            entry_point_identity,
            dispatch_contract_identity,
            argument_set_identity,
            kind,
            bindings,
            dependencies,
        })
    }

    /// Returns the caller-supplied request commitment.
    pub const fn identity(&self) -> DispatchRequestIdV1 {
        self.identity
    }

    /// Returns the parallel-operation context.
    pub const fn context(&self) -> &OperationContextV1 {
        &self.context
    }

    /// Returns the exact upstream load result commitment.
    pub const fn load_result_identity(&self) -> LoadResultIdV1 {
        self.load_result_identity
    }

    /// Returns the loaded-object commitment copied from the load result.
    pub const fn loaded_object_identity(&self) -> LoadedObjectIdV1 {
        self.loaded_object_identity
    }

    /// Returns the nonzero load generation copied from the load result.
    pub const fn load_generation(&self) -> u64 {
        self.load_generation
    }

    /// Returns the selected entry-point commitment.
    pub const fn entry_point_identity(&self) -> EntryPointIdV1 {
        self.entry_point_identity
    }

    /// Returns the dispatch-contract commitment.
    pub const fn dispatch_contract_identity(&self) -> DispatchContractIdV1 {
        self.dispatch_contract_identity
    }

    /// Returns the exact argument-set commitment.
    pub const fn argument_set_identity(&self) -> ArgumentSetIdV1 {
        self.argument_set_identity
    }

    /// Returns the finite or persistent-task shape.
    pub const fn kind(&self) -> DispatchKindV1 {
        self.kind
    }

    /// Returns contiguous bindings ordered by argument ordinal.
    pub fn bindings(&self) -> &[ResourceBindingV1] {
        &self.bindings
    }

    /// Returns the canonical predecessor-completion set.
    pub fn dependencies(&self) -> &[DispatchDependencyV1] {
        &self.dependencies
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(DispatchRequestIdV1::DOMAIN_V1);
        self.context.encode(&mut encoder);
        encoder.digest(self.load_result_identity.digest());
        encoder.digest(self.loaded_object_identity.digest());
        encoder.u64(self.load_generation);
        encoder.digest(self.entry_point_identity.digest());
        encoder.digest(self.dispatch_contract_identity.digest());
        encoder.digest(self.argument_set_identity.digest());
        match self.kind {
            DispatchKindV1::Finite => encoder.u8(1),
            DispatchKindV1::PersistentTask {
                service_instance_identity,
                task_schema_identity,
                task_tag,
                service_epoch,
            } => {
                encoder.u8(2);
                encoder.digest(service_instance_identity.digest());
                encoder.digest(task_schema_identity.digest());
                encoder.u32(task_tag);
                encoder.u64(service_epoch);
            }
        }
        encoder.usize_as_u16(self.bindings.len());
        for binding in &self.bindings {
            binding.encode(&mut encoder);
        }
        encoder.usize_as_u16(self.dependencies.len());
        for dependency in &self.dependencies {
            dependency.encode(&mut encoder);
        }
        check_preimage_bound(encoder.finish())
    }
}

/// Complete inert result of one V1 dispatch request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchResultV1 {
    identity: DispatchResultIdV1,
    request_identity: DispatchRequestIdV1,
    loaded_object_identity: LoadedObjectIdV1,
    outcome: DispatchOutcomeV1,
    diagnostics: Vec<HostDiagnosticV1>,
}

impl DispatchResultV1 {
    /// Creates a dispatch result bound to the exact request.
    pub fn new(
        identity: DispatchResultIdV1,
        request: &DispatchRequestV1,
        outcome: DispatchOutcomeV1,
        diagnostics: Vec<HostDiagnosticV1>,
    ) -> Result<Self, HostContractErrorV1> {
        let require_error = matches!(
            outcome,
            DispatchOutcomeV1::Rejected | DispatchOutcomeV1::Failed
        );
        validate_diagnostics(&diagnostics, require_error)?;
        if matches!(outcome, DispatchOutcomeV1::Submitted { .. })
            && diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity() == DiagnosticSeverityV1::Error)
        {
            return Err(HostContractErrorV1::InvalidOutcome);
        }
        Ok(Self {
            identity,
            request_identity: request.identity,
            loaded_object_identity: request.loaded_object_identity,
            outcome,
            diagnostics,
        })
    }

    /// Returns the caller-supplied result commitment.
    pub const fn identity(&self) -> DispatchResultIdV1 {
        self.identity
    }

    /// Returns the exact dispatch request commitment.
    pub const fn request_identity(&self) -> DispatchRequestIdV1 {
        self.request_identity
    }

    /// Returns the exact loaded-object commitment named by the request.
    pub const fn loaded_object_identity(&self) -> LoadedObjectIdV1 {
        self.loaded_object_identity
    }

    /// Returns the dispatch disposition.
    pub const fn outcome(&self) -> DispatchOutcomeV1 {
        self.outcome
    }

    /// Returns bounded diagnostics in producer order.
    pub fn diagnostics(&self) -> &[HostDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns a flow-erased binding for terminal-state validation.
    pub const fn result_reference(&self) -> HostResultReferenceV1 {
        let class = match self.outcome {
            DispatchOutcomeV1::Submitted { .. } => OperationResultClassV1::Succeeded,
            DispatchOutcomeV1::Rejected => OperationResultClassV1::Rejected,
            DispatchOutcomeV1::Failed => OperationResultClassV1::Failed,
        };
        HostResultReferenceV1::new(
            HostResultIdentityV1::Dispatch(self.identity),
            HostRequestIdentityV1::Dispatch(self.request_identity),
            class,
        )
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(DispatchResultIdV1::DOMAIN_V1);
        encoder.digest(self.request_identity.digest());
        encoder.digest(self.loaded_object_identity.digest());
        match self.outcome {
            DispatchOutcomeV1::Submitted {
                submission_identity,
                completion_signal_identity,
            } => {
                encoder.u8(1);
                encoder.digest(submission_identity.digest());
                encoder.digest(completion_signal_identity.digest());
            }
            DispatchOutcomeV1::Rejected => encoder.u8(2),
            DispatchOutcomeV1::Failed => encoder.u8(3),
        }
        encode_diagnostics(&self.diagnostics, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}
