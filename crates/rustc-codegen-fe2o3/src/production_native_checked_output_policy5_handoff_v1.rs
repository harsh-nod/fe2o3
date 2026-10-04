//! Separate owning wrapper so the established Policy4 enum size/receipt stays
//! unchanged. All source/collector/actual-O join algorithms remain shared.
use super::*;
use crate::production_pipeline::{
    checked_output_policy5_v1::NativeSourceCheckedOutputProductionCompilationV1 as Direct5,
    erased_checked_output_policy5_v1::NativeSourceErasedCheckedOutputProductionCompilationV1 as Erased5,
};

#[allow(
    clippy::large_enum_variant,
    reason = "retain the whole stage in place; its wrapper delta is accounted without an allocation"
)]
enum Stage5 {
    Direct(Direct5),
    Erased(Erased5),
}
impl Stage5 {
    fn inputs(&self) -> StageInputsV1<'_> {
        match self {
            Self::Direct(v) => v.native_worker_inputs_v1(),
            Self::Erased(v) => v.native_worker_inputs_v1(),
        }
    }
    fn stored_header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<Direct5>(),
            Self::Erased(_) => size_of::<Erased5>(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeOutputHandoffStoragePolicy5V1(usize);
impl NativeOutputHandoffStoragePolicy5V1 {
    #[allow(
        dead_code,
        reason = "pending protected-native consumer reserves this additional receipt"
    )]
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owns complete original source/N/(E)/B/C/S/O, actual worker handoff, signed
/// source proof and collector. No raw constructor or publisher conversion.
#[allow(
    dead_code,
    reason = "protected-native publication still requires its independent signed producer"
)]
pub(crate) struct PreparedNativeCheckedOutputWorkerHandoffPolicy5V1 {
    stage: Stage5,
    retained_floor: usize,
}
#[allow(
    dead_code,
    reason = "protected-native publication remains a separate closed boundary"
)]
impl PreparedNativeCheckedOutputWorkerHandoffPolicy5V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.stage.inputs().output.owner.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.stage
            .inputs()
            .output
            .prepared
            .native_output_parts_v1()
            .0
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| check_stage(self.stage.inputs(), budget))
    }
}

impl Direct5 {
    #[allow(
        dead_code,
        reason = "requires genuine signed source custody, not an inert receipt"
    )]
    pub(crate) fn prepare_native_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeCheckedOutputWorkerHandoffPolicy5V1,
        NativeOutputHandoffStoragePolicy5V1,
    )> {
        prepare5(Stage5::Direct(self), budget)
    }
}
impl Erased5 {
    #[allow(
        dead_code,
        reason = "requires genuine signed original N and independently replayed E"
    )]
    pub(crate) fn prepare_native_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeCheckedOutputWorkerHandoffPolicy5V1,
        NativeOutputHandoffStoragePolicy5V1,
    )> {
        prepare5(Stage5::Erased(self), budget)
    }
}
fn prepare5(
    stage: Stage5,
    budget: &mut Budget<'_>,
) -> R<(
    PreparedNativeCheckedOutputWorkerHandoffPolicy5V1,
    NativeOutputHandoffStoragePolicy5V1,
)> {
    budget.charge_work(4)?;
    let inherited = stage.inputs().retained_floor;
    if budget.storage() < inherited {
        return Err(Resource::Accounting.into());
    }
    scoped(budget, move |budget| {
        let additional = size_of::<PreparedNativeCheckedOutputWorkerHandoffPolicy5V1>()
            .checked_sub(stage.stored_header())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(additional)?;
        check_stage(stage.inputs(), budget)?;
        Ok((
            PreparedNativeCheckedOutputWorkerHandoffPolicy5V1 {
                stage,
                retained_floor: inherited
                    .checked_add(additional)
                    .ok_or(Resource::Arithmetic)?,
            },
            NativeOutputHandoffStoragePolicy5V1(additional),
        ))
    })
}
