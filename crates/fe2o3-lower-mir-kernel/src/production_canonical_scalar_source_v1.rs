use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as CsBlockV1,
    CanonicalKirDefinitionCoordinateV1 as CsDefinitionV1,
    CanonicalKirEdgeArgumentCoordinateV1 as CsEdgeArgumentV1,
    CanonicalKirEdgeCoordinateV1 as CsEdgeV1, CanonicalKirFunctionCoordinateV1 as CsFunctionV1,
    CanonicalKirOperationCoordinateV1 as CsOperationV1, CanonicalKirUseCoordinateV1 as CsUseV1,
    VerifiedCanonicalKernelIrModuleV12 as CsGraphV1,
};

type CsResultV1<T> = Result<T, ProductionCanonicalScalarSourceErrorV1>;

/// Refusal from original source custody, actual history, or fresh final checks.
#[derive(Debug)]
pub enum ProductionCanonicalScalarSourceErrorV1 {
    /// Complete original source replay, metadata or scope refused.
    Source(ProductionCanonicalRankedSourceErrorV1),
    /// The existing complete scalar/control source profile refused.
    SourcePolicy(ProductionCanonicalRankedPolicyErrorV1),
    /// Fresh original source assertion proof or exact original graph join refused.
    Assertion(ProductionCanonicalAssertionFailureV1),
    /// A checked adjacent assertion occurrence could not be transported.
    AssertionTransport {
        /// Complete original source-span ordinal.
        span: usize,
        /// Actual fixed-point round containing the refused transition.
        round: u16,
        /// True for the integer substage, false for the scalar substage.
        integer: bool,
        /// Exact failed occurrence or control requirement.
        reason: &'static str,
    },
    /// A private operation or root-qualified call lost its checked source transport.
    PrivateCallTransport {
        /// Exact original operation, never a relabeled final coordinate.
        operation: CsOperationV1,
        /// Actual fixed-point round containing the refused transition.
        round: u16,
        /// True for the integer substage, false for the scalar substage.
        integer: bool,
        /// Failed source, occurrence, or typed slot requirement.
        reason: &'static str,
    },
    /// The existing closed fixed-point factory or replay refused.
    FixedPoint(fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1),
    /// An actual adjacent inventory refused.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// The independent actual adjacent occurrence/control relation refused.
    Transition(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1),
    /// Real final fixed-nine failure, retaining its exact accepted history.
    FinalPolicy(fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1),
    /// A paid final policy query refused.
    Query(fe2o3_pliron::CanonicalRankedPolicyFailureV1),
    /// The caller's existing canonical ledger refused.
    Resource(ArgumentResourceV1),
    /// An actual original/final owner or borrowed inventory was substituted.
    InputCustody,
    /// A numeric source lineage roster did not cover its exact subject.
    Invalid(&'static str),
}
impl fmt::Display for ProductionCanonicalScalarSourceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => e.fmt(f),
            Self::SourcePolicy(e) => e.fmt(f),
            Self::Assertion(e) => e.fmt(f),
            Self::AssertionTransport {
                span,
                round,
                integer,
                reason,
            } => write!(
                f,
                "canonical assertion span {span}, round {round}, integer {integer}: {reason}"
            ),
            Self::FixedPoint(e) => e.fmt(f),
            Self::PrivateCallTransport {
                operation,
                round,
                integer,
                reason,
            } => write!(
                f,
                "canonical private/call operation {operation:?}, round {round}, integer {integer}: {reason}"
            ),
            Self::Inventory(e) => e.fmt(f),
            Self::Transition(e) => e.fmt(f),
            Self::FinalPolicy(e) => e.fmt(f),
            Self::Query(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::InputCustody => f.write_str("canonical scalar source owner custody mismatch"),
            Self::Invalid(reason) => write!(f, "canonical scalar source lineage: {reason}"),
        }
    }
}
impl std::error::Error for ProductionCanonicalScalarSourceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(e) => Some(e),
            Self::SourcePolicy(e) => Some(e),
            Self::Assertion(e) => Some(e),
            Self::FixedPoint(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Transition(e) => Some(e),
            Self::FinalPolicy(e) => Some(e),
            Self::Query(e) => Some(e),
            Self::Resource(e) => Some(e),
            Self::InputCustody
            | Self::Invalid(_)
            | Self::AssertionTransport { .. }
            | Self::PrivateCallTransport { .. } => None,
        }
    }
}
macro_rules! cs_error_from_v1 {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ProductionCanonicalScalarSourceErrorV1 {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
cs_error_from_v1!(ProductionCanonicalRankedSourceErrorV1, Source);
cs_error_from_v1!(ProductionCanonicalRankedPolicyErrorV1, SourcePolicy);
cs_error_from_v1!(ProductionCanonicalAssertionFailureV1, Assertion);
cs_error_from_v1!(fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1, FixedPoint);
cs_error_from_v1!(
    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
    Inventory
);
cs_error_from_v1!(
    fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1,
    Transition
);
cs_error_from_v1!(
    fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1,
    FinalPolicy
);
cs_error_from_v1!(fe2o3_pliron::CanonicalRankedPolicyFailureV1, Query);
cs_error_from_v1!(ArgumentResourceV1, Resource);
impl From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for ProductionCanonicalScalarSourceErrorV1 {
    fn from(error: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        Self::Source(error.into())
    }
}
impl From<fe2o3_kernel_analysis::CanonicalRankedViewErrorV1>
    for ProductionCanonicalScalarSourceErrorV1
{
    fn from(error: fe2o3_kernel_analysis::CanonicalRankedViewErrorV1) -> Self {
        Self::Source(error.into())
    }
}

fn cs_invalid_v1(reason: &'static str) -> ProductionCanonicalScalarSourceErrorV1 {
    ProductionCanonicalScalarSourceErrorV1::Invalid(reason)
}
fn cs_scope_v1<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CsResultV1<T>,
) -> CsResultV1<T> {
    cr_protected_v1(budget, |budget| Ok(run(budget)))?
}
fn cs_vec_v1<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<Vec<T>> {
    Ok(cr_vec_v1(count, budget)?)
}
fn cs_push_v1<T>(rows: &mut Vec<T>, row: T, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<()> {
    Ok(cr_push_v1(rows, row, budget)?)
}
fn cs_extent_v1<T>(rows: &Vec<T>) -> CsResultV1<usize> {
    Ok(argument_product_v1(
        rows.capacity(),
        std::mem::size_of::<T>(),
    )?)
}

/// Additional logical owner/history backing beyond transferred original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarStorageV1(usize);
impl ProductionCanonicalScalarStorageV1 {
    /// Reserve before further controlled work; this excludes the original floor.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Sole consumed original source and the existing complete checked scalar history.
/// This is neither final ranked completion nor target/publication authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarFixedPointOwnerV1;
/// fn duplicate(value: ProductionCanonicalScalarFixedPointOwnerV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarFixedPointOwnerV1;
/// fn forge() { let _ = ProductionCanonicalScalarFixedPointOwnerV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarFixedPointOwnerV1;
/// use fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1;
/// fn donor(history: CheckedScalarFixedPointOwnerV1) {
///     let _ = ProductionCanonicalScalarFixedPointOwnerV1::from_history(history);
/// }
/// ```
pub struct ProductionCanonicalScalarFixedPointOwnerV1 {
    original: ProductionPreRankedKirOwnerV1,
    history: fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    input_storage: usize,
    additional: ProductionCanonicalScalarStorageV1,
    retained: usize,
}
impl fmt::Debug for ProductionCanonicalScalarFixedPointOwnerV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCanonicalScalarFixedPointOwnerV1")
            .field("history", &self.history)
            .field("retained", &self.retained)
            .finish_non_exhaustive()
    }
}
impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Consume actual N, check its complete source profile, then derive the fixed
    /// existing neutral schedule. No caller history, graph or target is accepted.
    ///
    /// Prepay original.unit_local_source_storage_floor_v1() and siblings. Every
    /// exit restores entry storage. Success transfers the additional receipt;
    /// reserve it immediately while keeping the original floor paid. Failure
    /// drops the moved source/history before return; the caller retires its
    /// original reservation then. Source replay keeps its separate engine limits.
    pub fn try_prepare_v1(
        original: ProductionPreRankedKirOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<(Self, ProductionCanonicalScalarStorageV1)> {
        Self::prepare_with_source_v1(original, budget, |original, budget| {
            original.with_canonical_ranked_metadata_v1(budget, |source, budget| {
                Ok(cr_policy_source_profile_v1(source, budget))
            })??;
            Ok(())
        })
    }
    fn prepare_with_source_v1(
        original: ProductionPreRankedKirOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
        check_source: impl FnOnce(
            &ProductionPreRankedKirOwnerV1,
            &mut ArgumentBudgetV1<'_>,
        ) -> CsResultV1<()>,
    ) -> CsResultV1<(Self, ProductionCanonicalScalarStorageV1)> {
        cs_scope_v1(budget, move |budget| {
            budget.charge_work(3)?;
            let input_storage = original
                .unit_local_source_storage_floor_v1()
                .map_err(ProductionCanonicalRankedSourceErrorV1::from)?;
            if budget.storage() < input_storage {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.reserve_storage(std::mem::size_of::<Self>())?;
            check_source(&original, budget)?;
            let history = fe2o3_kernel_opt::prepare_checked_scalar_fixed_point_v1(
                original.executable(),
                budget,
            )?;
            budget.reserve_storage(history.retained_storage())?;
            history.replay_against(original.executable(), budget)?;
            budget.charge_work(3)?;
            let additional = ProductionCanonicalScalarStorageV1(argument_sum_v1(&[
                std::mem::size_of::<Self>(),
                history.retained_storage(),
            ])?);
            let retained = argument_sum_v1(&[input_storage, additional.0])?;
            Ok((
                Self {
                    original,
                    history,
                    input_storage,
                    additional,
                    retained,
                },
                additional,
            ))
        })
    }
    /// Exact original source/N, not source metadata relabeled as optimized output.
    pub const fn original_source(&self) -> &ProductionPreRankedKirOwnerV1 {
        &self.original
    }
    /// Actual owned final neutral graph, including the mandatory terminal round.
    pub fn output(&self) -> &CsGraphV1 {
        self.history.output()
    }
    /// Immutable real adjacent owners, complete maps and execution witnesses.
    pub const fn history(&self) -> &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1 {
        &self.history
    }
    /// Transferred original source/capture reservation, paid exactly once.
    pub const fn input_storage_floor_v1(&self) -> usize {
        self.input_storage
    }
    /// Complete original plus additional live owner/history reservation.
    pub const fn retained_storage_floor_v1(&self) -> usize {
        self.retained
    }
    /// Additional owner/history receipt, excluding original source and siblings.
    pub const fn additional_storage(&self) -> ProductionCanonicalScalarStorageV1 {
        self.additional
    }
    /// Final graph reports alone do not complete full compiler obligations.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No target, formal, artifact, publication or launch authority is granted.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

include!("production_canonical_scalar_transport_v1.rs");
include!("production_canonical_scalar_checks_v1.rs");
