//! Fail-closed refinement boundary for genuinely transforming PLIRON passes.
//!
//! The fixed eight production verifier stages are analysis-only and use exact
//! structural preservation. A pass that intentionally changes PLIRON belongs
//! here instead: one sealed session binds the live before and after owners,
//! their exact canonical structures, the compiler-owned pass implementation
//! and configuration identities, and an independent checker's result.
//!
//! No production transformation is registered in V1 because the workspace has
//! no independent PLIRON-to-PLIRON semantic checker yet. The generic boundary
//! is exercised with private adversarial fixtures only. An unsupported pass
//! cannot obtain a receipt.

#![allow(
    dead_code,
    reason = "the sealed generic boundary is intentionally dormant while the production transformation registry is empty"
)]

use std::{
    fmt,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_pliron_owner_core::{
    ContextIdentity, ContextIdentityError, ensure_context_identity, require_context_identity,
};
use pliron::{
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    op::Op,
    operation::Operation,
};

use crate::{
    PlironIrIdentityErrorV1, PlironIrStructuralIdentityV1, derive_pliron_ir_structural_identity_v1,
};

pub const SUPPORTED_PRODUCTION_PLIRON_TRANSFORMATIONS_V1: usize = 0;
const MAX_TRANSFORM_REFINEMENT_DETAIL_BYTES_V1: usize = 512;

static NEXT_TRANSFORM_REFINEMENT_SESSION_V1: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironTransformSnapshotSideV1 {
    Before,
    After,
}

impl fmt::Display for PlironTransformSnapshotSideV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Before => "before",
            Self::After => "after",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlironTransformCheckerDispositionV1 {
    Proved,
    Rejected,
    Incomplete,
    Unsupported,
}

/// Fail-closed diagnostics for the transforming-pass refinement boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironTransformRefinementErrorV1 {
    OwnerIdentityUnavailable {
        side: PlironTransformSnapshotSideV1,
        source: ContextIdentityError,
    },
    StructuralIdentityUnavailable {
        side: PlironTransformSnapshotSideV1,
        source: PlironIrIdentityErrorV1,
    },
    SessionIdentityExhausted,
    NoStructuralTransformation,
    PassImplementationMismatch,
    PassConfigurationMismatch,
    CheckerIdentityMismatch,
    CheckerBindingMismatch {
        component: &'static str,
    },
    CheckerResultReplayed,
    CheckerRejected {
        detail: String,
    },
    CheckerIncomplete {
        detail: String,
    },
    UnsupportedTransformation {
        pass: &'static str,
        detail: String,
    },
    InvalidCheckerDetail,
}

impl PlironTransformRefinementErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::OwnerIdentityUnavailable { .. }
            | Self::StructuralIdentityUnavailable { .. }
            | Self::SessionIdentityExhausted => "FE2O3-TRANSFORM-001",
            Self::NoStructuralTransformation => "FE2O3-TRANSFORM-002",
            Self::PassImplementationMismatch | Self::PassConfigurationMismatch => {
                "FE2O3-TRANSFORM-003"
            }
            Self::CheckerIdentityMismatch | Self::CheckerBindingMismatch { .. } => {
                "FE2O3-TRANSFORM-004"
            }
            Self::CheckerResultReplayed => "FE2O3-TRANSFORM-005",
            Self::CheckerRejected { .. } => "FE2O3-TRANSFORM-006",
            Self::CheckerIncomplete { .. } => "FE2O3-TRANSFORM-007",
            Self::UnsupportedTransformation { .. } => "FE2O3-TRANSFORM-008",
            Self::InvalidCheckerDetail => "FE2O3-TRANSFORM-009",
        }
    }
}

impl fmt::Display for PlironTransformRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::OwnerIdentityUnavailable { side, source } => {
                write!(formatter, "{side} PLIRON owner identity is unavailable: {source}")
            }
            Self::StructuralIdentityUnavailable { side, source } => {
                write!(formatter, "{side} PLIRON structural identity is unavailable: {source}")
            }
            Self::SessionIdentityExhausted => {
                formatter.write_str("transform-refinement session identity space is exhausted")
            }
            Self::NoStructuralTransformation => formatter.write_str(
                "the before and after PLIRON structures are exactly identical; use the analysis-only preservation boundary for a read-only pass",
            ),
            Self::PassImplementationMismatch => formatter.write_str(
                "the executed pass implementation identity differs from the sealed refinement contract",
            ),
            Self::PassConfigurationMismatch => formatter.write_str(
                "the executed pass configuration identity differs from the sealed refinement contract",
            ),
            Self::CheckerIdentityMismatch => formatter.write_str(
                "the semantic checker identity differs from the sealed refinement contract",
            ),
            Self::CheckerBindingMismatch { component } => write!(
                formatter,
                "the checker-issued result does not bind the live {component}"
            ),
            Self::CheckerResultReplayed => formatter.write_str(
                "the checker-issued result belongs to another one-shot refinement session",
            ),
            Self::CheckerRejected { detail } => {
                write!(formatter, "the semantic checker rejected the transformation: {detail}")
            }
            Self::CheckerIncomplete { detail } => write!(
                formatter,
                "the semantic checker could not prove the transformation: {detail}"
            ),
            Self::UnsupportedTransformation { pass, detail } => write!(
                formatter,
                "transforming pass {pass} is unsupported: {detail}"
            ),
            Self::InvalidCheckerDetail => formatter.write_str(
                "the checker result contains an empty, oversized, or noncanonical diagnostic",
            ),
        }
    }
}

impl std::error::Error for PlironTransformRefinementErrorV1 {}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SealedTransformIdentityV1([u8; 32]);

impl fmt::Debug for SealedTransformIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealedTransformIdentityV1(<compiler-owned>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlironTransformContractV1 {
    pass: &'static str,
    implementation: SealedTransformIdentityV1,
    configuration: SealedTransformIdentityV1,
    checker: SealedTransformIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlironTransformExecutionV1 {
    implementation: SealedTransformIdentityV1,
    configuration: SealedTransformIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LivePlironFunctionOwnerV1 {
    context: ContextIdentity,
    function: Ptr<Operation>,
}

fn capture_owner_v1(
    side: PlironTransformSnapshotSideV1,
    context: &mut Context,
    function: &FuncOp,
) -> Result<LivePlironFunctionOwnerV1, PlironTransformRefinementErrorV1> {
    let context = ensure_context_identity(context).map_err(|source| {
        PlironTransformRefinementErrorV1::OwnerIdentityUnavailable { side, source }
    })?;
    Ok(LivePlironFunctionOwnerV1 {
        context,
        function: function.get_operation(),
    })
}

fn next_session_v1() -> Result<NonZeroU64, PlironTransformRefinementErrorV1> {
    let value = NEXT_TRANSFORM_REFINEMENT_SESSION_V1
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| PlironTransformRefinementErrorV1::SessionIdentityExhausted)?;
    NonZeroU64::new(value).ok_or(PlironTransformRefinementErrorV1::SessionIdentityExhausted)
}

fn capture_structure_v1(
    side: PlironTransformSnapshotSideV1,
    context: &Context,
    function: &FuncOp,
) -> Result<PlironIrStructuralIdentityV1, PlironTransformRefinementErrorV1> {
    derive_pliron_ir_structural_identity_v1(context, function).map_err(|source| {
        PlironTransformRefinementErrorV1::StructuralIdentityUnavailable { side, source }
    })
}

struct PlironTransformRefinementSessionV1 {
    session: NonZeroU64,
    contract: PlironTransformContractV1,
    before_owner: LivePlironFunctionOwnerV1,
    before: PlironIrStructuralIdentityV1,
}

fn begin_pliron_transform_refinement_v1(
    context: &mut Context,
    function: &FuncOp,
    contract: PlironTransformContractV1,
) -> Result<PlironTransformRefinementSessionV1, PlironTransformRefinementErrorV1> {
    let before_owner = capture_owner_v1(PlironTransformSnapshotSideV1::Before, context, function)?;
    let before = capture_structure_v1(PlironTransformSnapshotSideV1::Before, context, function)?;
    Ok(PlironTransformRefinementSessionV1 {
        session: next_session_v1()?,
        contract,
        before_owner,
        before,
    })
}

struct PlironTransformCheckerInputV1<'a> {
    session: NonZeroU64,
    contract: PlironTransformContractV1,
    before_owner: LivePlironFunctionOwnerV1,
    after_owner: LivePlironFunctionOwnerV1,
    before: &'a PlironIrStructuralIdentityV1,
    after: &'a PlironIrStructuralIdentityV1,
}

impl PlironTransformCheckerInputV1<'_> {
    fn issue(
        &self,
        disposition: PlironTransformCheckerDispositionV1,
        detail: impl Into<String>,
    ) -> CheckerIssuedTransformRefinementV1 {
        CheckerIssuedTransformRefinementV1 {
            session: self.session,
            pass: self.contract.pass,
            implementation: self.contract.implementation,
            configuration: self.contract.configuration,
            checker: self.contract.checker,
            before_owner: self.before_owner,
            after_owner: self.after_owner,
            before: self.before.clone(),
            after: self.after.clone(),
            disposition,
            detail: detail.into(),
        }
    }
}

/// Crate-private checker seam. Public callers cannot inject a callback or
/// construct a checker-issued result.
trait PlironTransformSemanticCheckerV1 {
    fn identity(&self) -> SealedTransformIdentityV1;

    fn check(
        &mut self,
        input: &PlironTransformCheckerInputV1<'_>,
    ) -> CheckerIssuedTransformRefinementV1;
}

#[derive(Clone)]
struct CheckerIssuedTransformRefinementV1 {
    session: NonZeroU64,
    pass: &'static str,
    implementation: SealedTransformIdentityV1,
    configuration: SealedTransformIdentityV1,
    checker: SealedTransformIdentityV1,
    before_owner: LivePlironFunctionOwnerV1,
    after_owner: LivePlironFunctionOwnerV1,
    before: PlironIrStructuralIdentityV1,
    after: PlironIrStructuralIdentityV1,
    disposition: PlironTransformCheckerDispositionV1,
    detail: String,
}

/// Non-cloneable result of one exact, checker-proved named transformation.
///
/// This type has no public constructor or wire representation. The receipt is
/// intended to be moved exactly once into a future compiler-owned admission
/// join. It proves only the named transformation relation checked in this
/// session, not general program equivalence.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::PlironTransformRefinementReceiptV1;
///
/// fn duplicate(receipt: &PlironTransformRefinementReceiptV1) {
///     let _ = (*receipt).clone();
/// }
/// ```
#[must_use]
pub struct PlironTransformRefinementReceiptV1 {
    session: NonZeroU64,
    pass: &'static str,
    implementation: SealedTransformIdentityV1,
    configuration: SealedTransformIdentityV1,
    checker: SealedTransformIdentityV1,
    before_owner: LivePlironFunctionOwnerV1,
    after_owner: LivePlironFunctionOwnerV1,
    before: PlironIrStructuralIdentityV1,
    after: PlironIrStructuralIdentityV1,
}

impl fmt::Debug for PlironTransformRefinementReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlironTransformRefinementReceiptV1")
            .field("pass", &self.pass)
            .field("session", &"<process-local>")
            .field("implementation", &self.implementation)
            .field("configuration", &self.configuration)
            .field("checker", &self.checker)
            .field("before_owner", &"<process-local>")
            .field("after_owner", &"<process-local>")
            .finish_non_exhaustive()
    }
}

impl PlironTransformRefinementReceiptV1 {
    pub const fn pass_name(&self) -> &'static str {
        self.pass
    }

    pub const fn records_checker_proved_named_refinement(&self) -> bool {
        true
    }

    pub const fn records_one_shot_session_binding(&self) -> bool {
        self.session.get() != 0
    }

    pub const fn grants_general_operational_equivalence_authority(&self) -> bool {
        false
    }

    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }

    pub fn exactly_binds_before(
        &self,
        context: &Context,
        function: &FuncOp,
    ) -> Result<bool, ContextIdentityError> {
        Ok(
            require_context_identity(context)? == self.before_owner.context
                && function.get_operation() == self.before_owner.function
                && derive_pliron_ir_structural_identity_v1(context, function)
                    .is_ok_and(|identity| identity.exactly_matches(&self.before)),
        )
    }

    pub fn exactly_binds_after(
        &self,
        context: &Context,
        function: &FuncOp,
    ) -> Result<bool, ContextIdentityError> {
        Ok(
            require_context_identity(context)? == self.after_owner.context
                && function.get_operation() == self.after_owner.function
                && derive_pliron_ir_structural_identity_v1(context, function)
                    .is_ok_and(|identity| identity.exactly_matches(&self.after)),
        )
    }
}

fn validate_detail_v1(detail: &str) -> Result<(), PlironTransformRefinementErrorV1> {
    if detail.is_empty()
        || detail.len() > MAX_TRANSFORM_REFINEMENT_DETAIL_BYTES_V1
        || detail.bytes().any(|byte| byte == 0 || !byte.is_ascii())
    {
        return Err(PlironTransformRefinementErrorV1::InvalidCheckerDetail);
    }
    Ok(())
}

fn finish_pliron_transform_refinement_v1<C: PlironTransformSemanticCheckerV1>(
    session: PlironTransformRefinementSessionV1,
    after_context: &mut Context,
    after_function: &FuncOp,
    execution: PlironTransformExecutionV1,
    checker: &mut C,
) -> Result<PlironTransformRefinementReceiptV1, PlironTransformRefinementErrorV1> {
    if execution.implementation != session.contract.implementation {
        return Err(PlironTransformRefinementErrorV1::PassImplementationMismatch);
    }
    if execution.configuration != session.contract.configuration {
        return Err(PlironTransformRefinementErrorV1::PassConfigurationMismatch);
    }
    if checker.identity() != session.contract.checker {
        return Err(PlironTransformRefinementErrorV1::CheckerIdentityMismatch);
    }

    let after_owner = capture_owner_v1(
        PlironTransformSnapshotSideV1::After,
        after_context,
        after_function,
    )?;
    let after = capture_structure_v1(
        PlironTransformSnapshotSideV1::After,
        after_context,
        after_function,
    )?;
    if session.before.exactly_matches(&after) {
        return Err(PlironTransformRefinementErrorV1::NoStructuralTransformation);
    }

    let input = PlironTransformCheckerInputV1 {
        session: session.session,
        contract: session.contract,
        before_owner: session.before_owner,
        after_owner,
        before: &session.before,
        after: &after,
    };
    let result = checker.check(&input);
    validate_detail_v1(&result.detail)?;

    if result.session != input.session {
        return Err(PlironTransformRefinementErrorV1::CheckerResultReplayed);
    }
    for (matches, component) in [
        (result.pass == input.contract.pass, "pass identity"),
        (
            result.implementation == input.contract.implementation,
            "pass implementation identity",
        ),
        (
            result.configuration == input.contract.configuration,
            "pass configuration identity",
        ),
        (result.checker == input.contract.checker, "checker identity"),
        (result.before_owner == input.before_owner, "before IR owner"),
        (result.after_owner == input.after_owner, "after IR owner"),
        (
            result.before.exactly_matches(input.before),
            "exact before IR structure",
        ),
        (
            result.after.exactly_matches(input.after),
            "exact after IR structure",
        ),
    ] {
        if !matches {
            return Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch { component });
        }
    }

    match result.disposition {
        PlironTransformCheckerDispositionV1::Proved => Ok(PlironTransformRefinementReceiptV1 {
            session: result.session,
            pass: result.pass,
            implementation: result.implementation,
            configuration: result.configuration,
            checker: result.checker,
            before_owner: result.before_owner,
            after_owner: result.after_owner,
            before: result.before,
            after: result.after,
        }),
        PlironTransformCheckerDispositionV1::Rejected => {
            Err(PlironTransformRefinementErrorV1::CheckerRejected {
                detail: result.detail,
            })
        }
        PlironTransformCheckerDispositionV1::Incomplete => {
            Err(PlironTransformRefinementErrorV1::CheckerIncomplete {
                detail: result.detail,
            })
        }
        PlironTransformCheckerDispositionV1::Unsupported => Err(
            PlironTransformRefinementErrorV1::UnsupportedTransformation {
                pass: result.pass,
                detail: result.detail,
            },
        ),
    }
}

/// No transforming PLIRON pass has an independent semantic checker in V1.
pub const fn has_supported_production_pliron_transformation_v1() -> bool {
    SUPPORTED_PRODUCTION_PLIRON_TRANSFORMATIONS_V1 != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_kernel::{
        DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, ReturnOp,
        register_dialect,
    };
    use pliron::{
        basic_block::BasicBlock, builtin::types::FunctionType, context::Ptr, dialect::DialectName,
    };

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        dialect_proof::register_dialect(&mut context).unwrap();
        context
    }

    fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
        operation.get_operation().insert_at_back(block, context);
    }

    fn arithmetic_function(context: &mut Context, kind: IndexBinaryKindAttr) -> FuncOp {
        let function = FuncOp::new(
            context,
            "transform_fixture".try_into().unwrap(),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        let lhs = IndexConstantOp::new(context, 2);
        let rhs = IndexConstantOp::new(context, 3);
        let binary = IndexBinaryOp::new(context, kind, lhs.result(context), rhs.result(context));
        let ret = ReturnOp::new(context);
        append(context, entry, &lhs);
        append(context, entry, &rhs);
        append(context, entry, &binary);
        append(context, entry, &ret);
        function
    }

    fn identity(byte: u8) -> SealedTransformIdentityV1 {
        SealedTransformIdentityV1([byte; 32])
    }

    fn contract() -> PlironTransformContractV1 {
        PlironTransformContractV1 {
            pass: "test-only-index-rewrite",
            implementation: identity(1),
            configuration: identity(2),
            checker: identity(3),
        }
    }

    fn execution() -> PlironTransformExecutionV1 {
        PlironTransformExecutionV1 {
            implementation: identity(1),
            configuration: identity(2),
        }
    }

    struct ScriptedCheckerV1 {
        identity: SealedTransformIdentityV1,
        disposition: PlironTransformCheckerDispositionV1,
        detail: &'static str,
        mutate: Option<fn(&mut CheckerIssuedTransformRefinementV1)>,
        captured: Option<CheckerIssuedTransformRefinementV1>,
    }

    impl ScriptedCheckerV1 {
        fn proved() -> Self {
            Self {
                identity: identity(3),
                disposition: PlironTransformCheckerDispositionV1::Proved,
                detail: "test-only checker established the named fixture relation",
                mutate: None,
                captured: None,
            }
        }
    }

    impl PlironTransformSemanticCheckerV1 for ScriptedCheckerV1 {
        fn identity(&self) -> SealedTransformIdentityV1 {
            self.identity
        }

        fn check(
            &mut self,
            input: &PlironTransformCheckerInputV1<'_>,
        ) -> CheckerIssuedTransformRefinementV1 {
            let mut result = input.issue(self.disposition, self.detail);
            if let Some(mutate) = self.mutate {
                mutate(&mut result);
            }
            self.captured = Some(result.clone());
            result
        }
    }

    fn boundary(
        checker: &mut impl PlironTransformSemanticCheckerV1,
    ) -> Result<PlironTransformRefinementReceiptV1, PlironTransformRefinementErrorV1> {
        let before_context = &mut setup();
        let before = arithmetic_function(before_context, IndexBinaryKindAttr::Add);
        let session =
            begin_pliron_transform_refinement_v1(before_context, &before, contract()).unwrap();
        let after_context = &mut setup();
        let after = arithmetic_function(after_context, IndexBinaryKindAttr::Multiply);
        finish_pliron_transform_refinement_v1(session, after_context, &after, execution(), checker)
    }

    #[test]
    fn production_registry_is_empty_and_grants_no_fallback() {
        assert_eq!(SUPPORTED_PRODUCTION_PLIRON_TRANSFORMATIONS_V1, 0);
        assert!(!has_supported_production_pliron_transformation_v1());
    }

    #[test]
    fn private_checker_fixture_can_issue_only_a_narrow_non_authoritative_receipt() {
        let mut checker = ScriptedCheckerV1::proved();
        let receipt = boundary(&mut checker).unwrap();
        assert_eq!(receipt.pass_name(), "test-only-index-rewrite");
        assert!(receipt.records_checker_proved_named_refinement());
        assert!(receipt.records_one_shot_session_binding());
        assert!(!receipt.grants_general_operational_equivalence_authority());
        assert!(!receipt.grants_lowering_or_launch_authority());
        assert!(format!("{receipt:?}").contains("<compiler-owned>"));
    }

    #[test]
    fn receipt_binds_exact_live_owners_not_equal_reconstructions() {
        let before_context = &mut setup();
        let before = arithmetic_function(before_context, IndexBinaryKindAttr::Add);
        let session =
            begin_pliron_transform_refinement_v1(before_context, &before, contract()).unwrap();
        let after_context = &mut setup();
        let after = arithmetic_function(after_context, IndexBinaryKindAttr::Multiply);
        let mut checker = ScriptedCheckerV1::proved();
        let receipt = finish_pliron_transform_refinement_v1(
            session,
            after_context,
            &after,
            execution(),
            &mut checker,
        )
        .unwrap();
        assert_eq!(
            receipt.exactly_binds_before(before_context, &before),
            Ok(true)
        );
        assert_eq!(receipt.exactly_binds_after(after_context, &after), Ok(true));

        let foreign_context = &mut setup();
        ensure_context_identity(foreign_context).unwrap();
        let foreign = arithmetic_function(foreign_context, IndexBinaryKindAttr::Multiply);
        assert_eq!(
            receipt.exactly_binds_after(foreign_context, &foreign),
            Ok(false)
        );
    }

    #[test]
    fn a_forged_before_binding_cannot_issue_a_receipt() {
        let mut checker = ScriptedCheckerV1::proved();
        checker.mutate = Some(|result| result.before_owner = result.after_owner);
        assert!(matches!(
            boundary(&mut checker),
            Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch {
                component: "before IR owner"
            })
        ));
    }

    #[test]
    fn a_forged_after_structure_cannot_issue_a_receipt() {
        let mut checker = ScriptedCheckerV1::proved();
        checker.mutate = Some(|result| result.after = result.before.clone());
        assert!(matches!(
            boundary(&mut checker),
            Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch {
                component: "exact after IR structure"
            })
        ));
    }

    #[test]
    fn pass_implementation_and_configuration_mismatches_fail_before_checking() {
        let before_context = &mut setup();
        let before = arithmetic_function(before_context, IndexBinaryKindAttr::Add);
        let after_context = &mut setup();
        let after = arithmetic_function(after_context, IndexBinaryKindAttr::Multiply);

        let session =
            begin_pliron_transform_refinement_v1(before_context, &before, contract()).unwrap();
        let mut wrong_implementation = execution();
        wrong_implementation.implementation = identity(9);
        let mut checker = ScriptedCheckerV1::proved();
        assert_eq!(
            finish_pliron_transform_refinement_v1(
                session,
                after_context,
                &after,
                wrong_implementation,
                &mut checker,
            )
            .unwrap_err(),
            PlironTransformRefinementErrorV1::PassImplementationMismatch,
        );

        let session =
            begin_pliron_transform_refinement_v1(before_context, &before, contract()).unwrap();
        let mut wrong_configuration = execution();
        wrong_configuration.configuration = identity(8);
        assert_eq!(
            finish_pliron_transform_refinement_v1(
                session,
                after_context,
                &after,
                wrong_configuration,
                &mut checker,
            )
            .unwrap_err(),
            PlironTransformRefinementErrorV1::PassConfigurationMismatch,
        );
        assert!(checker.captured.is_none());
    }

    #[test]
    fn forged_checker_pass_and_configuration_bindings_fail_closed() {
        let mut checker = ScriptedCheckerV1::proved();
        checker.mutate = Some(|result| result.implementation = identity(9));
        assert!(matches!(
            boundary(&mut checker),
            Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch {
                component: "pass implementation identity"
            })
        ));

        let mut checker = ScriptedCheckerV1::proved();
        checker.mutate = Some(|result| result.configuration = identity(8));
        assert!(matches!(
            boundary(&mut checker),
            Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch {
                component: "pass configuration identity"
            })
        ));
    }

    #[test]
    fn checker_identity_and_result_identity_mismatches_fail_closed() {
        let mut checker = ScriptedCheckerV1::proved();
        checker.identity = identity(7);
        assert_eq!(
            boundary(&mut checker).unwrap_err(),
            PlironTransformRefinementErrorV1::CheckerIdentityMismatch,
        );

        let mut checker = ScriptedCheckerV1::proved();
        checker.mutate = Some(|result| result.checker = identity(7));
        assert!(matches!(
            boundary(&mut checker),
            Err(PlironTransformRefinementErrorV1::CheckerBindingMismatch {
                component: "checker identity"
            })
        ));
    }

    struct ReplayCheckerV1 {
        identity: SealedTransformIdentityV1,
        result: Option<CheckerIssuedTransformRefinementV1>,
    }

    impl PlironTransformSemanticCheckerV1 for ReplayCheckerV1 {
        fn identity(&self) -> SealedTransformIdentityV1 {
            self.identity
        }

        fn check(
            &mut self,
            _input: &PlironTransformCheckerInputV1<'_>,
        ) -> CheckerIssuedTransformRefinementV1 {
            self.result.take().unwrap()
        }
    }

    #[test]
    fn checker_result_replay_across_sessions_is_rejected() {
        let mut first = ScriptedCheckerV1::proved();
        let _ = boundary(&mut first).unwrap();
        let stale = first.captured.take().unwrap();
        let mut replay = ReplayCheckerV1 {
            identity: identity(3),
            result: Some(stale),
        };
        assert_eq!(
            boundary(&mut replay).unwrap_err(),
            PlironTransformRefinementErrorV1::CheckerResultReplayed,
        );
    }

    #[test]
    fn rejected_incomplete_and_unsupported_results_never_issue_receipts() {
        for (disposition, expected_code) in [
            (
                PlironTransformCheckerDispositionV1::Rejected,
                "FE2O3-TRANSFORM-006",
            ),
            (
                PlironTransformCheckerDispositionV1::Incomplete,
                "FE2O3-TRANSFORM-007",
            ),
            (
                PlironTransformCheckerDispositionV1::Unsupported,
                "FE2O3-TRANSFORM-008",
            ),
        ] {
            let mut checker = ScriptedCheckerV1 {
                identity: identity(3),
                disposition,
                detail: "test-only checker did not establish refinement",
                mutate: None,
                captured: None,
            };
            let error = boundary(&mut checker).unwrap_err();
            assert_eq!(error.code(), expected_code);
            assert!(error.to_string().contains("did not establish refinement"));
        }
    }

    #[test]
    fn unchanged_ir_uses_the_analysis_only_boundary() {
        let context = &mut setup();
        let before = arithmetic_function(context, IndexBinaryKindAttr::Add);
        let session = begin_pliron_transform_refinement_v1(context, &before, contract()).unwrap();
        let mut checker = ScriptedCheckerV1::proved();
        assert_eq!(
            finish_pliron_transform_refinement_v1(
                session,
                context,
                &before,
                execution(),
                &mut checker,
            )
            .unwrap_err(),
            PlironTransformRefinementErrorV1::NoStructuralTransformation,
        );
        assert!(checker.captured.is_none());
    }

    #[test]
    fn malformed_checker_detail_is_not_accepted_as_a_diagnostic() {
        let mut checker = ScriptedCheckerV1::proved();
        checker.detail = "";
        assert_eq!(
            boundary(&mut checker).unwrap_err(),
            PlironTransformRefinementErrorV1::InvalidCheckerDetail,
        );
    }
}
