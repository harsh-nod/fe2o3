use pliron::{
    builtin::op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface},
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op},
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    verify_err,
};

/// Closed schedule selector for the one general-GEMM algorithm body.
#[pliron_attr(name = "schedule.general_gemm_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmScheduleAttr {
    /// Scalar masked A/B staging.
    ReferenceWave64Xor4V1,
    /// Aligned full A-v4 staging with scalar A tail; B remains scalar.
    VectorizedAOnlyBf16GlobalTransferV1,
}

/// Exact bounded K-phase schedule.
#[pliron_attr(name = "schedule.general_gemm_phase", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmPhasePlanAttr {
    /// Dynamic `ceil_div(K, 16)` phases with one reusable LDS buffer.
    CeilDivK16SingleBufferV1,
}

/// Exact global-transfer policy selected by the schedule.
#[pliron_attr(name = "schedule.general_gemm_transfer", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmTransferPlanAttr {
    /// Masked scalar A and B transfers with zero-filled tails.
    ScalarAScalarBZeroFillV1,
    /// Aligned full A-v4 with scalar masked fallback; scalar masked B.
    VectorA4ScalarFallbackScalarBZeroFillV1,
}

/// Schedule-specific general-GEMM lowering plan.
#[pliron_op(
    name = "schedule.general_gemm_plan",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        general_gemm_kind: GeneralGemmScheduleAttr,
        general_gemm_phase: GeneralGemmPhasePlanAttr,
        general_gemm_transfer: GeneralGemmTransferPlanAttr
    )
)]
pub struct GeneralGemmPlanOp;

impl GeneralGemmPlanOp {
    /// Builds one of the two closed schedules.
    pub fn new(context: &mut Context, schedule: GeneralGemmScheduleAttr) -> Self {
        let transfer = match schedule {
            GeneralGemmScheduleAttr::ReferenceWave64Xor4V1 => {
                GeneralGemmTransferPlanAttr::ScalarAScalarBZeroFillV1
            }
            GeneralGemmScheduleAttr::VectorizedAOnlyBf16GlobalTransferV1 => {
                GeneralGemmTransferPlanAttr::VectorA4ScalarFallbackScalarBZeroFillV1
            }
        };
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_general_gemm_kind(context, schedule);
        op.set_attr_general_gemm_phase(context, GeneralGemmPhasePlanAttr::CeilDivK16SingleBufferV1);
        op.set_attr_general_gemm_transfer(context, transfer);
        op
    }
}

impl Verify for GeneralGemmPlanOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let operation = operation.deref(context);
        let schedule = self.get_attr_general_gemm_kind(context).map(|value| *value);
        let transfer = self
            .get_attr_general_gemm_transfer(context)
            .map(|value| *value);
        let matching = matches!(
            (schedule, transfer),
            (
                Some(GeneralGemmScheduleAttr::ReferenceWave64Xor4V1),
                Some(GeneralGemmTransferPlanAttr::ScalarAScalarBZeroFillV1)
            ) | (
                Some(GeneralGemmScheduleAttr::VectorizedAOnlyBf16GlobalTransferV1),
                Some(GeneralGemmTransferPlanAttr::VectorA4ScalarFallbackScalarBZeroFillV1)
            )
        );
        if operation.get_num_operands() != 0
            || operation.get_num_results() != 0
            || operation.get_num_successors() != 0
            || operation.num_regions() != 0
            || operation.attributes.0.len() != 3
            || self
                .get_attr_general_gemm_phase(context)
                .is_none_or(|value| *value != GeneralGemmPhasePlanAttr::CeilDivK16SingleBufferV1)
            || !matching
        {
            return verify_err!(
                self.loc(context),
                "schedule.general_gemm_plan has a non-canonical schedule payload"
            );
        }
        Ok(())
    }
}
