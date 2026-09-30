// Resource selection is a closed typed match, independent of diagnostic chains.
fn aggregate_inventory_resource_v30(
    error: &fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1 as E;
    match error {
        E::Resource(error) => Some(*error),
        E::InconsistentOwner => None,
    }
}

fn aggregate_transition_resource_v30(
    error: &fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1 as E;
    match error {
        E::Resource(error) => Some(*error),
        E::Arithmetic => Some(ArgumentResourceV1::Arithmetic),
        E::InvalidCoordinate | E::IncompleteRows | E::Rule(_) => None,
    }
}

fn aggregate_encode_resource_v30(
    error: &fe2o3_kernel_ir::KernelIrEncodeError,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_ir::KernelIrEncodeError as E;
    match error {
        E::WorkLimit(error) => Some(ArgumentResourceV1::Work(*error)),
        E::Allocation => Some(ArgumentResourceV1::Allocation),
        E::Overflow { .. } => Some(ArgumentResourceV1::Arithmetic),
        E::TooLarge { .. }
        | E::LimitExceeded { .. }
        | E::TypeNestingTooDeep { .. }
        | E::UnsupportedInVersion { .. }
        | E::NonCanonical { .. } => None,
    }
}

fn aggregate_decode_resource_v30(
    error: &fe2o3_kernel_ir::KernelIrDecodeError,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_ir::KernelIrDecodeError as E;
    match error {
        E::Resource(error) => Some(*error),
        E::WorkLimit(error) => Some(ArgumentResourceV1::Work(*error)),
        E::Encode(error) => aggregate_encode_resource_v30(error),
        E::TooLarge { .. }
        | E::InvalidMagic
        | E::UnknownVersion(_)
        | E::UnsupportedFlags(_)
        | E::InvalidLength { .. }
        | E::Truncated
        | E::TrailingBytes
        | E::ReservedNonZero { .. }
        | E::UnknownTag { .. }
        | E::InvalidUtf8 { .. }
        | E::LimitExceeded { .. }
        | E::TypeNestingTooDeep { .. }
        | E::NonCanonical
        | E::InvalidSemanticOperationInstance => None,
    }
}

fn aggregate_admission_resource_v30(
    error: &fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as E,
        StorageLayoutErrorV1 as L,
    };
    match error {
        E::Resource(error)
        | E::Layout(L::Resource(error))
        | E::Verification(V::Resource(error)) => Some(*error),
        E::Encode(error) => aggregate_encode_resource_v30(error),
        E::Decode(error) => aggregate_decode_resource_v30(error),
        E::Layout(L::Invalid { .. }) | E::Verification(V::Verification(_)) => None,
    }
}

fn aggregate_canonical12_resource_v30(
    error: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV12,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV12 as E;
    match error {
        E::Encode(error) => aggregate_encode_resource_v30(error),
        E::Decode(error) => aggregate_decode_resource_v30(error),
        E::Verification(_) | E::NotExactV12 { .. } | E::RoundTripMismatch | E::IdentityMismatch => {
            None
        }
    }
}

fn aggregate_admission12_resource_v30(
    error: &fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrReplayAdmissionErrorV12 as E,
        MeteredVerifiedCanonicalKernelIrErrorV12 as C,
    };
    match error {
        E::Resource(error) | E::Canonical(C::VerificationResource { error, .. }) => Some(*error),
        E::Encode(error) => aggregate_encode_resource_v30(error),
        E::Decode(error) => aggregate_decode_resource_v30(error),
        E::Canonical(C::Canonical(error) | C::PostVerificationCanonical { error, .. }) => {
            aggregate_canonical12_resource_v30(error)
        }
        E::Canonical(C::WorkLimit(error) | C::VerificationWorkLimit { error, .. }) => {
            Some(ArgumentResourceV1::Work(*error))
        }
        E::Verification(_) | E::CanonicalMismatch | E::Canonical(C::Verification { .. }) => None,
    }
}

fn aggregate_map_resource_v30(
    error: &fe2o3_pliron::KirOptimizationMapErrorV12,
) -> Option<ArgumentResourceV1> {
    use fe2o3_pliron::KirOptimizationMapErrorV12 as E;
    match error {
        E::Resources(error) => Some(*error),
        E::Arithmetic => Some(ArgumentResourceV1::Arithmetic),
        E::Allocation => Some(ArgumentResourceV1::Allocation),
        E::Limit
        | E::Identity
        | E::Lifecycle
        | E::Coverage
        | E::Passes
        | E::Relation
        | E::UnsupportedMutation => None,
    }
}

fn aggregate_scalar_resource_v30(
    error: &fe2o3_pliron::KirNeutralOptimizationErrorV18,
) -> Option<ArgumentResourceV1> {
    use fe2o3_pliron::{
        KirBridgeErrorV12 as B12, KirBridgeErrorV18 as B18, KirNeutralOptimizationErrorV18 as E,
        PlironOptimizationErrorV12 as X,
    };
    match error {
        E::Resource(error)
        | E::Bridge(B18::Resource(error))
        | E::Execution(X::Resources(error))
        | E::Execution(X::Bridge(B12::Resource(error))) => Some(*error),
        E::Bridge(B18::Canonical(error)) => aggregate_admission_resource_v30(error),
        E::Execution(X::Bridge(B12::Canonical(error))) => aggregate_admission12_resource_v30(error),
        E::Mapping(error) | E::Execution(X::Mapping(error)) => aggregate_map_resource_v30(error),
        E::Execution(X::Accounting) => Some(ArgumentResourceV1::Accounting),
        E::Bridge(B18::Allocation) => Some(ArgumentResourceV1::Allocation),
        E::Bridge(B18::Bridge(_) | B18::SessionSetup)
        | E::Execution(
            X::Bridge(B12::Bridge(_) | B12::SessionSetup) | X::Execution(_) | X::AlreadyExecuted,
        )
        | E::Pass(_)
        | E::Endpoint
        | E::Limit
        | E::Panicked => None,
    }
}

fn aggregate_optimizer_resource_v30(
    error: &fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18,
) -> Option<ArgumentResourceV1> {
    use fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18 as C;
    use fe2o3_kernel_opt::{OwnedAggregateFixedpointErrorV18 as E, OwnedAggregateSsaErrorV18 as A};
    use fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1 as D;
    match error {
        E::Resource(error)
        | E::Aggregate(A::Resource(error))
        | E::Aggregate(A::Check(C::Resource(error)))
        | E::Adoption(D::Resource(error)) => Some(*error),
        E::Scalar(error) => aggregate_scalar_resource_v30(error),
        E::Inventory(error)
        | E::Aggregate(A::Inventory(error))
        | E::Aggregate(A::Check(C::Inventory(error)))
        | E::Adoption(D::Inventory(error)) => aggregate_inventory_resource_v30(error),
        E::Transition(error) | E::Adoption(D::Transition(error)) => {
            aggregate_transition_resource_v30(error)
        }
        E::Map(error) => aggregate_map_resource_v30(error),
        E::Aggregate(A::Admission(error)) => aggregate_admission_resource_v30(error),
        E::Adoption(D::OriginAccounting) => Some(ArgumentResourceV1::Accounting),
        E::Adoption(D::Origin(never)) => match *never {},
        E::Aggregate(
            A::Check(C::Planner(_) | C::Inconsistent(_) | C::ForeignInput | C::Panicked)
            | A::Inconsistent(_)
            | A::ForeignInput
            | A::Panicked,
        )
        | E::Adoption(D::Panicked)
        | E::Inconsistent(_)
        | E::RoundLimit { .. }
        | E::ForeignInput
        | E::Panicked => None,
    }
}

fn aggregate_source_resource_v30(
    error: &ProductionAggregateSourceErrorV30,
) -> Option<ArgumentResourceV1> {
    use ProductionAggregateSourceErrorV30 as E;
    use ProductionPendingScopedSourceErrorV29 as Pending;
    use ProductionSourceOwnedViewErrorV18 as S;
    use fe2o3_kernel_analysis::{
        CanonicalKirMemorySsaErrorV1 as M, CanonicalKirPrivateMemoryErrorV1 as P,
        CanonicalKirSparseErrorV1 as Q,
    };
    use fe2o3_pliron::CanonicalAnalysisScopeErrorV1 as A;
    match error {
        E::Optimization(error) => aggregate_optimizer_resource_v30(error),
        E::Inventory(error)
        | E::Source(S::Analysis(A::Inventory(error)))
        | E::Source(S::PrivateMemory(P::Inventory(error))) => {
            aggregate_inventory_resource_v30(error)
        }
        E::Transition(error) => aggregate_transition_resource_v30(error),
        E::Source(S::Source(Pending::Canonical(error))) => aggregate_admission_resource_v30(error),
        E::Source(S::Source(Pending::Occurrences(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error),
        )))
        | E::Source(S::Source(Pending::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        ))) => Some(*error),
        E::Source(
            S::Resource(error)
            | S::Analysis(
                A::Resource(error)
                | A::Sparse(Q::Resource(error))
                | A::MemorySsa(M::Resource(error)),
            )
            | S::PrivateMemory(P::Resource(error)),
        ) => Some(*error),
        E::Source(
            S::Source(Pending::Source(_) | Pending::Occurrences(_))
            | S::Binding(_)
            | S::Analysis(
                A::Sparse(
                    Q::InputLimit { .. }
                    | Q::InconsistentInventory
                    | Q::InvalidUseCoordinate { .. },
                )
                | A::MemorySsa(
                    M::InputLimit { .. }
                    | M::InconsistentInventory
                    | M::InvalidBlock(_)
                    | M::InvalidOperation(_)
                    | M::InvalidNode(_)
                    | M::NotPhi(_),
                ),
            )
            | S::PrivateMemory(P::Unsupported { .. } | P::Panicked),
        ) => None,
    }
}
