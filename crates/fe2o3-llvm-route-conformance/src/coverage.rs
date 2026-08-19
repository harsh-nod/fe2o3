use core::fmt;

/// Maximum number of UTF-8 bytes accepted by a conformance-case lookup.
pub const CONFORMANCE_CASE_NAME_MAX_BYTES_V1: usize = 64;

/// Maximum number of cases admitted by the V1 conformance corpus.
pub const MAX_CONFORMANCE_CASES_V1: usize = 48;

/// A semantic family tracked by the generic-CI corpus.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConformanceSemanticV1 {
    /// Canonical gfx942 target-feature state.
    TargetFeatures,
    /// Kernel calling convention.
    CallingConvention,
    /// Pointer address-space representation.
    AddressSpace,
    /// Pointer alignment attributes.
    Alignment,
    /// Atomic operation representation.
    AtomicOperation,
    /// Atomic memory-order representation.
    AtomicOrdering,
    /// Atomic synchronization-scope representation.
    AtomicScope,
    /// Intrinsic declaration and call representation.
    Intrinsic,
    /// Required module flags and named metadata.
    ModuleMetadata,
    /// Canonical source-origin identity.
    Origin,
    /// Canonical preservation-obligation identity.
    Obligation,
    /// Device-library kind, digest, and size identity.
    DeviceLibraryIdentity,
    /// Typed Pliron LLVM lowering lane.
    PlironLoweringLane,
    /// Isolated-worker handoff admission lane.
    WorkerAdmissionLane,
}

/// A hostile input that the current public V1 API is expected to reject.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpectedRejectionV1 {
    /// An unknown target-feature wire tag.
    UnknownTargetFeature,
    /// Duplicate target-feature tags carrying conflicting states.
    ConflictingTargetFeature,
    /// A known target feature with a noncanonical state.
    UnsupportedTargetFeatureState,
    /// An unknown calling-convention wire tag.
    UnknownCallingConvention,
    /// An unknown address-space wire tag.
    UnknownAddressSpace,
    /// A zero pointer alignment.
    ZeroAlignment,
    /// A non-power-of-two pointer alignment.
    NonPowerOfTwoAlignment,
    /// A pointer alignment above the V1 maximum.
    OversizedAlignment,
    /// An unknown module-flag wire tag.
    UnknownModuleFlag,
    /// An unknown named-metadata wire tag.
    UnknownNamedMetadata,
    /// A duplicate module flag.
    DuplicateModuleFlag,
    /// An unknown origin-kind wire tag.
    UnknownOriginKind,
    /// A mutation that no longer matches the canonical origin identity.
    NonCanonicalOriginIdentity,
    /// An unknown obligation-kind wire tag.
    UnknownObligationKind,
    /// A mutation that no longer matches the canonical obligation identity.
    NonCanonicalObligationIdentity,
    /// An unknown device-library-kind wire tag.
    UnknownDeviceLibraryKind,
    /// A duplicate device-library kind.
    DuplicateDeviceLibraryKind,
    /// An all-zero device-library digest identity.
    ZeroDeviceLibraryIdentity,
    /// A claimed handoff identity that differs from canonical bytes.
    WorkerHandoffIdentityMismatch,
    /// A measured worker-build field that differs from the admitted build.
    WorkerBuildIdentitySubstitution,
    /// A measured worker-build field that exceeds its diagnostic bound.
    WorkerBuildFieldTooLong,
    /// A handoff device-library kind outside the worker's closed set.
    WorkerUnsupportedDeviceLibrary,
    /// A function call outside the scalar Pliron lowering vocabulary.
    PlironLoweringUnsupportedCall,
    /// A scalar type outside the scalar Pliron lowering vocabulary.
    PlironLoweringUnsupportedType,
    /// An address space outside the scalar Pliron lowering vocabulary.
    PlironLoweringUnsupportedAddressSpace,
    /// A target policy outside the scalar Pliron lowering vocabulary.
    PlironLoweringUnsupportedTargetPolicy,
}

/// A semantic that cannot yet be exercised through a current public API.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoverageGapV1 {
    /// Handoff V1 has no atomic-operation field.
    AtomicOperationRepresentation,
    /// Handoff V1 has no atomic memory-order field.
    AtomicOrderingRepresentation,
    /// Handoff V1 has no atomic synchronization-scope field.
    AtomicScopeRepresentation,
    /// Handoff V1 has no intrinsic declaration or call field.
    IntrinsicRepresentation,
}

/// The expected generic-CI disposition for one named case.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConformanceExpectationV1 {
    /// A public handoff or lane API represents and canonicalizes the property.
    Represented,
    /// The public handoff V1 API must return the named typed rejection.
    ExpectedRejection(ExpectedRejectionV1),
    /// No current public API can express the semantic without inventing a contract.
    CoverageGap(CoverageGapV1),
}

/// One stable, named entry in the gfx942 generic-CI conformance corpus.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConformanceCaseV1 {
    name: &'static str,
    semantic: ConformanceSemanticV1,
    expectation: ConformanceExpectationV1,
}

impl ConformanceCaseV1 {
    const fn new(
        name: &'static str,
        semantic: ConformanceSemanticV1,
        expectation: ConformanceExpectationV1,
    ) -> Self {
        Self {
            name,
            semantic,
            expectation,
        }
    }

    /// Returns the stable case name used by CI and diagnostics.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the semantic family covered by this case.
    pub const fn semantic(self) -> ConformanceSemanticV1 {
        self.semantic
    }

    /// Returns the expected disposition without claiming backend correspondence.
    pub const fn expectation(self) -> ConformanceExpectationV1 {
        self.expectation
    }
}

const fn represented(name: &'static str, semantic: ConformanceSemanticV1) -> ConformanceCaseV1 {
    ConformanceCaseV1::new(name, semantic, ConformanceExpectationV1::Represented)
}

const fn rejected(
    name: &'static str,
    semantic: ConformanceSemanticV1,
    rejection: ExpectedRejectionV1,
) -> ConformanceCaseV1 {
    ConformanceCaseV1::new(
        name,
        semantic,
        ConformanceExpectationV1::ExpectedRejection(rejection),
    )
}

const fn gap(
    name: &'static str,
    semantic: ConformanceSemanticV1,
    gap: CoverageGapV1,
) -> ConformanceCaseV1 {
    ConformanceCaseV1::new(name, semantic, ConformanceExpectationV1::CoverageGap(gap))
}

/// Stable gfx942 handoff V1 corpus for generic CI.
///
/// `Represented` means only that the handoff model preserves the field. It is
/// not evidence of LLVM IR, machine code, code-object, or hardware behavior.
pub const GFX942_CONFORMANCE_CORPUS_V1: &[ConformanceCaseV1] = &[
    represented(
        "target-features.canonical-wave64-xnack-off",
        ConformanceSemanticV1::TargetFeatures,
    ),
    rejected(
        "target-features.unknown-tag",
        ConformanceSemanticV1::TargetFeatures,
        ExpectedRejectionV1::UnknownTargetFeature,
    ),
    rejected(
        "target-features.conflicting-state",
        ConformanceSemanticV1::TargetFeatures,
        ExpectedRejectionV1::ConflictingTargetFeature,
    ),
    rejected(
        "target-features.unsupported-state",
        ConformanceSemanticV1::TargetFeatures,
        ExpectedRejectionV1::UnsupportedTargetFeatureState,
    ),
    represented(
        "calling-convention.amdgpu-kernel",
        ConformanceSemanticV1::CallingConvention,
    ),
    rejected(
        "calling-convention.unknown-tag",
        ConformanceSemanticV1::CallingConvention,
        ExpectedRejectionV1::UnknownCallingConvention,
    ),
    represented(
        "address-space.all-handoff-v1",
        ConformanceSemanticV1::AddressSpace,
    ),
    rejected(
        "address-space.unknown-tag",
        ConformanceSemanticV1::AddressSpace,
        ExpectedRejectionV1::UnknownAddressSpace,
    ),
    represented(
        "alignment.pointer-attribute",
        ConformanceSemanticV1::Alignment,
    ),
    rejected(
        "alignment.zero",
        ConformanceSemanticV1::Alignment,
        ExpectedRejectionV1::ZeroAlignment,
    ),
    rejected(
        "alignment.non-power-of-two",
        ConformanceSemanticV1::Alignment,
        ExpectedRejectionV1::NonPowerOfTwoAlignment,
    ),
    rejected(
        "alignment.over-maximum",
        ConformanceSemanticV1::Alignment,
        ExpectedRejectionV1::OversizedAlignment,
    ),
    represented(
        "module-metadata.canonical",
        ConformanceSemanticV1::ModuleMetadata,
    ),
    rejected(
        "module-metadata.unknown-flag",
        ConformanceSemanticV1::ModuleMetadata,
        ExpectedRejectionV1::UnknownModuleFlag,
    ),
    rejected(
        "module-metadata.unknown-named",
        ConformanceSemanticV1::ModuleMetadata,
        ExpectedRejectionV1::UnknownNamedMetadata,
    ),
    rejected(
        "module-metadata.duplicate-flag",
        ConformanceSemanticV1::ModuleMetadata,
        ExpectedRejectionV1::DuplicateModuleFlag,
    ),
    represented("origin.canonical-identity", ConformanceSemanticV1::Origin),
    rejected(
        "origin.unknown-kind",
        ConformanceSemanticV1::Origin,
        ExpectedRejectionV1::UnknownOriginKind,
    ),
    rejected(
        "origin.identity-mutation",
        ConformanceSemanticV1::Origin,
        ExpectedRejectionV1::NonCanonicalOriginIdentity,
    ),
    represented(
        "obligation.canonical-identity",
        ConformanceSemanticV1::Obligation,
    ),
    rejected(
        "obligation.unknown-kind",
        ConformanceSemanticV1::Obligation,
        ExpectedRejectionV1::UnknownObligationKind,
    ),
    rejected(
        "obligation.identity-mutation",
        ConformanceSemanticV1::Obligation,
        ExpectedRejectionV1::NonCanonicalObligationIdentity,
    ),
    represented(
        "device-library.identity-carried",
        ConformanceSemanticV1::DeviceLibraryIdentity,
    ),
    represented(
        "device-library.identity-mutation-reidentifies-handoff",
        ConformanceSemanticV1::DeviceLibraryIdentity,
    ),
    rejected(
        "device-library.unknown-kind",
        ConformanceSemanticV1::DeviceLibraryIdentity,
        ExpectedRejectionV1::UnknownDeviceLibraryKind,
    ),
    rejected(
        "device-library.duplicate-kind",
        ConformanceSemanticV1::DeviceLibraryIdentity,
        ExpectedRejectionV1::DuplicateDeviceLibraryKind,
    ),
    rejected(
        "device-library.zero-identity",
        ConformanceSemanticV1::DeviceLibraryIdentity,
        ExpectedRejectionV1::ZeroDeviceLibraryIdentity,
    ),
    represented(
        "lane.worker-admission.canonical-inert",
        ConformanceSemanticV1::WorkerAdmissionLane,
    ),
    rejected(
        "lane.worker-admission.handoff-identity-mismatch",
        ConformanceSemanticV1::WorkerAdmissionLane,
        ExpectedRejectionV1::WorkerHandoffIdentityMismatch,
    ),
    rejected(
        "lane.worker-admission.build-identity-substitution",
        ConformanceSemanticV1::WorkerAdmissionLane,
        ExpectedRejectionV1::WorkerBuildIdentitySubstitution,
    ),
    rejected(
        "lane.worker-admission.build-field-too-long",
        ConformanceSemanticV1::WorkerAdmissionLane,
        ExpectedRejectionV1::WorkerBuildFieldTooLong,
    ),
    rejected(
        "lane.worker-admission.unsupported-device-library",
        ConformanceSemanticV1::WorkerAdmissionLane,
        ExpectedRejectionV1::WorkerUnsupportedDeviceLibrary,
    ),
    represented(
        "lane.pliron-lowering.canonical-deterministic",
        ConformanceSemanticV1::PlironLoweringLane,
    ),
    rejected(
        "lane.pliron-lowering.unsupported-call",
        ConformanceSemanticV1::PlironLoweringLane,
        ExpectedRejectionV1::PlironLoweringUnsupportedCall,
    ),
    rejected(
        "lane.pliron-lowering.unsupported-type",
        ConformanceSemanticV1::PlironLoweringLane,
        ExpectedRejectionV1::PlironLoweringUnsupportedType,
    ),
    rejected(
        "lane.pliron-lowering.unsupported-address-space",
        ConformanceSemanticV1::PlironLoweringLane,
        ExpectedRejectionV1::PlironLoweringUnsupportedAddressSpace,
    ),
    rejected(
        "lane.pliron-lowering.unsupported-target-policy",
        ConformanceSemanticV1::PlironLoweringLane,
        ExpectedRejectionV1::PlironLoweringUnsupportedTargetPolicy,
    ),
    gap(
        "atomic.operation.unrepresented",
        ConformanceSemanticV1::AtomicOperation,
        CoverageGapV1::AtomicOperationRepresentation,
    ),
    gap(
        "atomic.ordering.unrepresented",
        ConformanceSemanticV1::AtomicOrdering,
        CoverageGapV1::AtomicOrderingRepresentation,
    ),
    gap(
        "atomic.scope.unrepresented",
        ConformanceSemanticV1::AtomicScope,
        CoverageGapV1::AtomicScopeRepresentation,
    ),
    gap(
        "intrinsic.unrepresented",
        ConformanceSemanticV1::Intrinsic,
        CoverageGapV1::IntrinsicRepresentation,
    ),
];

/// Typed, bounded failure from a conformance-case lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageLookupErrorV1 {
    /// The supplied name exceeded [`CONFORMANCE_CASE_NAME_MAX_BYTES_V1`].
    NameTooLong {
        /// Observed UTF-8 byte length.
        observed: usize,
        /// Maximum accepted UTF-8 byte length.
        maximum: usize,
    },
    /// The bounded name is not a member of the V1 corpus.
    UnknownCase,
}

impl fmt::Display for CoverageLookupErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTooLong { observed, maximum } => write!(
                formatter,
                "conformance case name has {observed} bytes, maximum is {maximum}"
            ),
            Self::UnknownCase => formatter.write_str("unknown conformance case"),
        }
    }
}

impl std::error::Error for CoverageLookupErrorV1 {}

/// Looks up one stable V1 case without reflecting an unbounded input in errors.
pub fn conformance_case_v1(
    name: &str,
) -> Result<&'static ConformanceCaseV1, CoverageLookupErrorV1> {
    if name.len() > CONFORMANCE_CASE_NAME_MAX_BYTES_V1 {
        return Err(CoverageLookupErrorV1::NameTooLong {
            observed: name.len(),
            maximum: CONFORMANCE_CASE_NAME_MAX_BYTES_V1,
        });
    }
    GFX942_CONFORMANCE_CORPUS_V1
        .iter()
        .find(|case| case.name == name)
        .ok_or(CoverageLookupErrorV1::UnknownCase)
}
