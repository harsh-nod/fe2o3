//! Actual U descriptor evidence only; no producer, codec or publication grant.
use super::*;
#[path = "compiler_descriptor_nominal_loop_unroll_v3.rs"]
pub(crate) mod nominal_v3;
use fe2o3_lower_mir_kernel::{
    ProductionLoopUnrollErrorV1 as UnrollAdmission,
    ProductionOwnedLoopUnrollContinuationV1 as DirectU,
    ProductionOwnedUnitLocalLoopUnrollContinuationV1 as ErasedU,
};

#[derive(Debug)]
pub(crate) enum LoopUnrollDescriptorErrorV1 {
    Resource(Resource),
    Admission(Box<UnrollAdmission>),
    Descriptor(Box<CompilerDescriptorError>),
    Panicked,
}
impl fmt::Display for LoopUnrollDescriptorErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LoopUnrollDescriptorErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Admission(e) => Some(e.as_ref()),
            Self::Descriptor(e) => Some(e.as_ref()),
            Self::Panicked => None,
        }
    }
}
impl From<Resource> for LoopUnrollDescriptorErrorV1 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
type Result<T> = std::result::Result<T, LoopUnrollDescriptorErrorV1>;
fn descriptor(e: CompilerDescriptorError) -> LoopUnrollDescriptorErrorV1 {
    LoopUnrollDescriptorErrorV1::Descriptor(Box::new(e))
}

#[derive(Clone, Copy)]
pub(crate) enum UnrolledOwnerV1<'a> {
    Direct(&'a DirectU),
    Erased(&'a ErasedU),
}
impl<'a> UnrolledOwnerV1<'a> {
    #[cfg(test)]
    pub(crate) fn output(self) -> &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    #[cfg(test)]
    pub(crate) fn final_f(self) -> FinalOwnerV1<'a> {
        match self {
            Self::Direct(v) => FinalOwnerV1::Direct(v.prefix()),
            Self::Erased(v) => FinalOwnerV1::Erased(v.prefix()),
        }
    }
    fn required(self) -> Result<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(|e| LoopUnrollDescriptorErrorV1::Admission(Box::new(e)))
    }
    fn replay(self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(|e| LoopUnrollDescriptorErrorV1::Admission(Box::new(e)))
    }
    fn view(self) -> Result<CheckedDescriptorViewV1<'a>> {
        // Only original semantic/source-launch/N-or-E/B custody is inherited.
        // U's own complete fresh formal reports select the new final subject.
        let mut view = match self {
            Self::Direct(v) => {
                policy8::direct_view(v.prefix().prefix().prefix().prefix().prefix().prefix())
                    .map_err(descriptor)?
            }
            Self::Erased(v) => {
                policy8::erased_view(v.prefix().prefix().prefix().prefix().prefix().prefix())
            }
        };
        match self {
            Self::Direct(v) => {
                view.output = v.output();
                view.kernels = v.kernels();
            }
            Self::Erased(v) => {
                view.output = v.output();
                view.kernels = v.kernels();
            }
        }
        Ok(view)
    }
}

fn scoped<'w, T>(
    required: usize,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T>,
) -> Result<T> {
    if budget.storage() < required {
        return Err(Resource::Accounting.into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(LoopUnrollDescriptorErrorV1::Panicked)
        }
    };
    let valid = budget.work_ledger_identity_v1() == ledger && budget.storage() >= floor;
    if !valid {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if valid {
        budget.release_storage(budget.storage() - floor)?;
    }
    drop(payloads);
    result
}
const HEADER: usize = size_of::<UnrolledOwnerV1<'static>>()
    + size_of::<CheckedDescriptorViewV1<'static>>()
    + size_of::<Vec<ProductionGeometryV1>>();

/// Replays genuine U ownership and checks original typed/source/launch/target
/// custody against U's ABI and fresh formal obligations. No encoding or producer
/// version is introduced. Inherited descriptor/formal engine domains are unchanged;
/// this adapter prepays its own borrowed headers and requested geometry backing,
/// reconciles actual returned capacity, and drops the vector before scope refund.
pub(crate) fn validate_unrolled_descriptor_evidence_v1(
    owner: UnrolledOwnerV1<'_>,
    typed_roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(owner.required()?, budget, |budget| {
        budget.reserve_storage(HEADER)?;
        budget.charge_work(1)?;
        owner.replay(budget)?;
        let view = owner.view()?;
        let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            view.neutral,
            view.bound,
            profile,
            budget,
        )
        .map_err(|e| descriptor(CompilerDescriptorError::CheckedOutputTarget(e)))?;
        let requested = typed_roots
            .len()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        let geometries = validate_checked_output_descriptor_evidence_v1(
            typed_roots,
            &view,
            profile.device_target(),
        )
        .map_err(descriptor)?;
        let actual = geometries
            .capacity()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        budget.charge_work(1)?;
        if geometries.len() != typed_roots.len() {
            return Err(descriptor(
                CompilerDescriptorError::ProductionDescriptorMismatch(
                    "complete actual U descriptor geometry roster",
                ),
            ));
        }
        drop(geometries);
        Ok(())
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn final_u_descriptor_scope_preserves_success_and_ordinary_panic_charges() {
        for panic in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut budget = Budget::new(&mut work, 128);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(53).unwrap();
            let result = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                if panic {
                    panic!("ordinary descriptor unwind");
                }
                Ok(41)
            });
            if panic {
                assert!(matches!(result, Err(LoopUnrollDescriptorErrorV1::Panicked)));
            } else {
                assert_eq!(result.unwrap(), 41);
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (20, 53, 72)
            );
            assert_eq!(budget.failed_storage(), None);
        }
    }

    #[test]
    fn final_u_descriptor_scope_refunds_before_payload_destructor_panics() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("descriptor payload destructor");
            }
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
        let mut budget = Budget::new(&mut work, 128);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(53).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                std::panic::panic_any(Payload);
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (20, 53, 72)
        );
        assert_eq!(budget.failed_storage(), None);
    }

    #[test]
    fn final_u_descriptor_scope_never_refunds_a_foreign_ledger() {
        for panic in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut budget = Budget::new(&mut work, 128);
            let mut other = Budget::new(&mut other_work, 128);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(53).unwrap();
            other.charge_work(11).unwrap();
            other.reserve_storage(37).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let other_ledger = other.work_ledger_identity_v1();
            let result: Result<()> = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                std::mem::swap(budget, &mut other);
                if panic {
                    panic!("foreign descriptor ledger");
                }
                Ok(())
            });
            assert!(matches!(
                result,
                Err(LoopUnrollDescriptorErrorV1::Resource(Resource::Accounting))
            ));
            assert!(budget.work_ledger_identity_v1() == other_ledger);
            assert!(other.work_ledger_identity_v1() == ledger);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (11, 37, 37)
            );
            assert_eq!(
                (other.work(), other.storage(), other.peak_storage()),
                (20, 72, 72)
            );
            std::mem::swap(&mut budget, &mut other);
            budget.release_storage(19).unwrap();
            assert_eq!((budget.storage(), other.storage()), (53, 37));
        }
    }

    #[test]
    fn final_u_descriptor_scope_preserves_first_denial_history() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
        let mut budget = Budget::new(&mut work, 60);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(53).unwrap();
        for (amount, actual, accepted) in [(8, 61, 20), (9, 62, 23)] {
            let result = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget
                    .reserve_storage(amount)
                    .map_err(LoopUnrollDescriptorErrorV1::from)
            });
            match result {
                Err(LoopUnrollDescriptorErrorV1::Resource(Resource::Storage(error))) => {
                    assert_eq!((error.actual(), error.limit()), (actual, 60));
                }
                _ => panic!("exact first reserve refusal"),
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (accepted, 53, 53)
            );
            assert_eq!(budget.failed_storage(), Some(61));
        }
        let mut short_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(18);
        let mut short = Budget::new(&mut short_work, 128);
        short.charge_work(17).unwrap();
        short.reserve_storage(53).unwrap();
        let result: Result<()> = scoped(53, &mut short, |budget| {
            budget.charge_work(2)?;
            budget.reserve_storage(19)?;
            Ok(())
        });
        match result {
            Err(LoopUnrollDescriptorErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!((error.actual(), error.limit()), (19, 18));
            }
            _ => panic!("exact first work refusal"),
        }
        assert_eq!(
            (short.work(), short.storage(), short.peak_storage()),
            (17, 53, 53)
        );
        assert_eq!(short.failed_storage(), None);
    }

    pub(crate) fn exercise_unrolled_report_omission_v1(
        owner: UnrolledOwnerV1<'_>,
        typed_roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        budget: &mut Budget<'_>,
    ) {
        let floor = budget.storage();
        let result = scoped(owner.required().unwrap(), budget, |budget| {
            budget.reserve_storage(HEADER)?;
            let mut view = owner.view()?;
            assert_eq!(view.kernels.len(), 2);
            view.kernels = &view.kernels[1..];
            validate_checked_output_descriptor_evidence_v1(
                typed_roots,
                &view,
                profile.device_target(),
            )
            .map_err(descriptor)?;
            Ok(())
        });
        assert!(
            matches!(result, Err(LoopUnrollDescriptorErrorV1::Descriptor(e))
        if matches!(*e, CompilerDescriptorError::ProductionDescriptorMismatch("complete ordered typed/source/output/formal root roster")))
        );
        assert_eq!(budget.storage(), floor);
    }

    pub(crate) fn exercise_unrolled_first_header_denial_v1(
        owner: UnrolledOwnerV1<'_>,
        typed_roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        floor: usize,
    ) {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(64);
        {
            let mut budget = Budget::new(&mut work, floor + HEADER - 1);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result =
                validate_unrolled_descriptor_evidence_v1(owner, typed_roots, profile, &mut budget);
            match result {
                Err(LoopUnrollDescriptorErrorV1::Resource(Resource::Storage(e))) => {
                    assert_eq!(
                        (e.actual(), e.limit()),
                        (floor + HEADER, floor + HEADER - 1)
                    );
                }
                other => panic!("U descriptor header-first refusal: {other:?}"),
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (0, floor, floor)
            );
            assert_eq!(budget.failed_storage(), Some(floor + HEADER));
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
        assert_eq!(work.failed_work(), None);
    }
}
