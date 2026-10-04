//! Fixed-width, domain-specific identity commitments.

/// Width of every V1 host API identity commitment.
pub const IDENTITY_BYTES_V1: usize = 32;

/// Opaque output of an integration-selected canonical digest operation.
///
/// Construction checks only width. It does not hash or authenticate bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostDigestV1([u8; IDENTITY_BYTES_V1]);

impl HostDigestV1 {
    /// Wraps an untrusted fixed-width commitment.
    pub const fn from_untrusted_bytes(bytes: [u8; IDENTITY_BYTES_V1]) -> Self {
        Self(bytes)
    }

    /// Borrows the opaque commitment bytes.
    pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES_V1] {
        &self.0
    }

    /// Returns the opaque commitment bytes.
    pub const fn into_bytes(self) -> [u8; IDENTITY_BYTES_V1] {
        self.0
    }
}

macro_rules! typed_identity {
    ($(#[$meta:meta])* $name:ident, $domain:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(HostDigestV1);

        impl $name {
            /// Domain prepended to the canonical V1 identity preimage.
            pub const DOMAIN_V1: &'static [u8] = $domain;

            /// Wraps a caller-supplied commitment without authenticating it.
            pub const fn from_untrusted_digest(digest: HostDigestV1) -> Self {
                Self(digest)
            }

            /// Returns the opaque commitment.
            pub const fn digest(self) -> HostDigestV1 {
                self.0
            }
        }
    };
}

typed_identity!(
    /// Identity of a parallel host-operation namespace.
    FlowScopeIdV1,
    b"fe2o3.host.scope.v1"
);
typed_identity!(
    /// Identity of one logical host operation across retries.
    OperationIdV1,
    b"fe2o3.host.operation.v1"
);
typed_identity!(
    /// Identity of an opaque payload's semantic content.
    PayloadIdV1,
    b"fe2o3.host.payload.v1"
);
typed_identity!(
    /// Identity of an opaque payload format.
    PayloadFormatIdV1,
    b"fe2o3.host.payload-format.v1"
);
typed_identity!(
    /// Identity of the compiler/frontend semantic profile.
    CompilerProfileIdV1,
    b"fe2o3.host.compiler-profile.v1"
);
typed_identity!(
    /// Identity of a target-neutral requested target profile.
    TargetProfileIdV1,
    b"fe2o3.host.target-profile.v1"
);
typed_identity!(
    /// Identity of a complete compile configuration.
    CompileConfigurationIdV1,
    b"fe2o3.host.compile-configuration.v1"
);
typed_identity!(
    /// Identity of an artifact-admission policy.
    AdmissionPolicyIdV1,
    b"fe2o3.host.admission-policy.v1"
);
typed_identity!(
    /// Identity of one evidence or obligation claim.
    ClaimIdV1,
    b"fe2o3.host.claim.v1"
);
typed_identity!(
    /// Identity assigned to an accepted admission assessment.
    AdmissionAssessmentIdV1,
    b"fe2o3.host.admission-assessment.v1"
);
typed_identity!(
    /// Identity of the runtime-neutral loader profile.
    LoaderProfileIdV1,
    b"fe2o3.host.loader-profile.v1"
);
typed_identity!(
    /// Identity of an opaque runtime context description.
    RuntimeContextIdV1,
    b"fe2o3.host.runtime-context.v1"
);
typed_identity!(
    /// Identity of a described loaded object, not a runtime handle.
    LoadedObjectIdV1,
    b"fe2o3.host.loaded-object.v1"
);
typed_identity!(
    /// Identity of an artifact entry point.
    EntryPointIdV1,
    b"fe2o3.host.entry-point.v1"
);
typed_identity!(
    /// Identity of an exact launch or task argument set.
    ArgumentSetIdV1,
    b"fe2o3.host.argument-set.v1"
);
typed_identity!(
    /// Identity of a dispatch launch contract.
    DispatchContractIdV1,
    b"fe2o3.host.dispatch-contract.v1"
);
typed_identity!(
    /// Identity of a persistent service instance description.
    ServiceInstanceIdV1,
    b"fe2o3.host.service-instance.v1"
);
typed_identity!(
    /// Identity of a closed persistent task schema.
    TaskSchemaIdV1,
    b"fe2o3.host.task-schema.v1"
);
typed_identity!(
    /// Identity of a described resource allocation, not a runtime handle.
    ResourceIdV1,
    b"fe2o3.host.resource.v1"
);
typed_identity!(
    /// Identity of one accepted dispatch submission description.
    DispatchSubmissionIdV1,
    b"fe2o3.host.dispatch-submission.v1"
);
typed_identity!(
    /// Identity of an inert completion signal description.
    CompletionSignalIdV1,
    b"fe2o3.host.completion-signal.v1"
);
typed_identity!(
    /// Identity of one observed completion record.
    CompletionRecordIdV1,
    b"fe2o3.host.completion-record.v1"
);
typed_identity!(
    /// Identity of a runtime-neutral deadline contract.
    DeadlineIdV1,
    b"fe2o3.host.deadline.v1"
);

typed_identity!(
    /// Commitment to a complete compile request.
    CompileRequestIdV1,
    b"fe2o3.host.compile.request.v1"
);
typed_identity!(
    /// Commitment to a complete compile result.
    CompileResultIdV1,
    b"fe2o3.host.compile.result.v1"
);
typed_identity!(
    /// Commitment to one compile operation state.
    CompileStateIdV1,
    b"fe2o3.host.compile.state.v1"
);
typed_identity!(
    /// Commitment to one compile state event.
    CompileEventIdV1,
    b"fe2o3.host.compile.event.v1"
);

typed_identity!(
    /// Commitment to a complete admission request.
    AdmitRequestIdV1,
    b"fe2o3.host.admit.request.v1"
);
typed_identity!(
    /// Commitment to a complete admission result.
    AdmitResultIdV1,
    b"fe2o3.host.admit.result.v1"
);
typed_identity!(
    /// Commitment to one admission operation state.
    AdmitStateIdV1,
    b"fe2o3.host.admit.state.v1"
);
typed_identity!(
    /// Commitment to one admission state event.
    AdmitEventIdV1,
    b"fe2o3.host.admit.event.v1"
);

typed_identity!(
    /// Commitment to a complete load request.
    LoadRequestIdV1,
    b"fe2o3.host.load.request.v1"
);
typed_identity!(
    /// Commitment to a complete load result.
    LoadResultIdV1,
    b"fe2o3.host.load.result.v1"
);
typed_identity!(
    /// Commitment to one load operation state.
    LoadStateIdV1,
    b"fe2o3.host.load.state.v1"
);
typed_identity!(
    /// Commitment to one load state event.
    LoadEventIdV1,
    b"fe2o3.host.load.event.v1"
);

typed_identity!(
    /// Commitment to a complete dispatch request.
    DispatchRequestIdV1,
    b"fe2o3.host.dispatch.request.v1"
);
typed_identity!(
    /// Commitment to a complete dispatch result.
    DispatchResultIdV1,
    b"fe2o3.host.dispatch.result.v1"
);
typed_identity!(
    /// Commitment to one dispatch operation state.
    DispatchStateIdV1,
    b"fe2o3.host.dispatch.state.v1"
);
typed_identity!(
    /// Commitment to one dispatch state event.
    DispatchEventIdV1,
    b"fe2o3.host.dispatch.event.v1"
);

typed_identity!(
    /// Commitment to a complete wait request.
    WaitRequestIdV1,
    b"fe2o3.host.wait.request.v1"
);
typed_identity!(
    /// Commitment to a complete wait result.
    WaitResultIdV1,
    b"fe2o3.host.wait.result.v1"
);
typed_identity!(
    /// Commitment to one wait operation state.
    WaitStateIdV1,
    b"fe2o3.host.wait.state.v1"
);
typed_identity!(
    /// Commitment to one wait state event.
    WaitEventIdV1,
    b"fe2o3.host.wait.event.v1"
);
