//! Actual U descriptor evidence only; no producer, codec or publication grant.
use super::*;
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
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(LoopUnrollDescriptorErrorV1::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(budget.storage() - floor)?;
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
