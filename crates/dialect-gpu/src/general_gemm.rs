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

/// Lowered dynamic runtime ABI schema.
#[pliron_attr(name = "gpu.general_gemm_runtime_abi", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmRuntimeAbiAttr {
    /// Exact ordered A/B/C/M/N/K/lda/ldb/ldc/alpha/beta ABI roles.
    DynamicElevenArgumentBf16F32V1,
}

/// Lowered grid, lane, and output-fragment coordinate mapping.
#[pliron_attr(name = "gpu.general_gemm_grid_mapping", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmGridMappingAttr {
    /// Grid XY selects 16x16 tiles; one wave64 owns four C coordinates per lane.
    GridXy16Wave64FourComponentsV1,
}

/// Lowered bounded reduction-loop semantics.
#[pliron_attr(name = "gpu.general_gemm_phase_loop", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmPhaseLoopAttr {
    /// Canonical phase induction from zero to checked `ceil_div(K, 16)`.
    CheckedCeilDivK16InductionV1,
}

/// Exact global-memory transfer semantics.
#[pliron_attr(name = "gpu.general_gemm_global_transfer", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmGlobalTransferAttr {
    /// Guarded checked row-major scalar A load; false lanes produce BF16 zero.
    AScalarMaskedZeroFillV1,
    /// Guarded aligned/full A-v4 load with scalar masked zero-fill fallback.
    AVector4AlignedFullScalarFallbackZeroFillV1,
    /// Guarded checked row-major scalar B load; false lanes produce BF16 zero.
    BScalarMaskedZeroFillV1,
    /// Guarded checked row-major C load owned by one disjoint output lane.
    CGuardedDisjointLoadV1,
    /// Guarded checked row-major C store owned by one disjoint output lane.
    CGuardedDisjointStoreV1,
}

/// Exact XOR4 workgroup-memory transfer semantics.
#[pliron_attr(name = "gpu.general_gemm_lds_transfer", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmLdsTransferAttr {
    /// Each lane writes four staged A components through the XOR4 mapping.
    AWriteFourXor4V1,
    /// Each lane writes four staged B components through the XOR4 mapping.
    BWriteFourXor4V1,
    /// Each lane reads four published A components through the XOR4 mapping.
    AReadFourXor4V1,
    /// Each lane reads four published B components through the XOR4 mapping.
    BReadFourXor4V1,
}

/// Exact single-buffer phase lifecycle event.
#[pliron_attr(name = "gpu.general_gemm_epoch", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmEpochAttr {
    /// All staged writes publish before any LDS read.
    PublishWorkgroupAcquireReleaseV1,
    /// All LDS reads complete before the next phase can reuse the buffer.
    ReuseWorkgroupAcquireReleaseV1,
}

/// Exact carried matrix-accumulator recurrence.
#[pliron_attr(name = "gpu.general_gemm_mfma", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmMfmaAttr {
    /// BF16 16x16x16 wave64 MFMA carries four f32 accumulators per lane.
    Bf16F32Wave64CarriedF32x4V1,
}

/// Exact lowered C epilogue semantics.
#[pliron_attr(name = "gpu.general_gemm_epilogue", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmEpilogueAttr {
    /// Four guarded owners compute `alpha * accumulator + beta * C`.
    GuardedDisjointAlphaAccPlusBetaCV1,
}

macro_rules! closed_record_op {
    (
        $(#[$meta:meta])*
        $name:ident,
        $op_name:literal,
        $field:ident,
        $attr:ty
    ) => {
        $(#[$meta])*
        #[pliron_op(
            name = $op_name,
            format,
            interfaces = [NOpdsInterface<0>, NResultsInterface<0>, NRegionsInterface<0>],
            attributes = ($field: $attr)
        )]
        pub struct $name;
    };
}

closed_record_op!(
    /// Binds the exact dynamic ABI roles consumed by explicit GPU operations.
    GeneralGemmRuntimeAbiOp,
    "gpu.general_gemm_runtime_abi",
    general_gemm_runtime_abi,
    GeneralGemmRuntimeAbiAttr
);
closed_record_op!(
    /// Binds workgroup/lane coordinates to the output tile and four owners.
    GeneralGemmGridMappingOp,
    "gpu.general_gemm_grid_mapping",
    general_gemm_grid_mapping,
    GeneralGemmGridMappingAttr
);
closed_record_op!(
    /// Represents the complete bounded dynamic K-phase loop.
    GeneralGemmPhaseLoopOp,
    "gpu.general_gemm_phase_loop",
    general_gemm_phase_loop,
    GeneralGemmPhaseLoopAttr
);
closed_record_op!(
    /// Represents one guarded checked global-memory transfer.
    GeneralGemmGlobalTransferOp,
    "gpu.general_gemm_global_transfer",
    general_gemm_global_transfer,
    GeneralGemmGlobalTransferAttr
);
closed_record_op!(
    /// Represents one exact XOR4 LDS write or read.
    GeneralGemmLdsTransferOp,
    "gpu.general_gemm_lds_transfer",
    general_gemm_lds_transfer,
    GeneralGemmLdsTransferAttr
);
closed_record_op!(
    /// Represents one publish or reuse epoch boundary.
    GeneralGemmEpochOp,
    "gpu.general_gemm_epoch",
    general_gemm_epoch,
    GeneralGemmEpochAttr
);
closed_record_op!(
    /// Represents the loop-carried BF16-to-f32 MFMA recurrence.
    GeneralGemmMfmaOp,
    "gpu.general_gemm_mfma",
    general_gemm_mfma,
    GeneralGemmMfmaAttr
);
closed_record_op!(
    /// Represents the exact alpha/beta C epilogue.
    GeneralGemmEpilogueOp,
    "gpu.general_gemm_epilogue",
    general_gemm_epilogue,
    GeneralGemmEpilogueAttr
);

macro_rules! impl_closed_record_op {
    ($name:ident, $setter:ident, $getter:ident, $attr:ty) => {
        impl $name {
            /// Builds one bounded typed record.
            pub fn new(context: &mut Context, attribute: $attr) -> Self {
                let operation = Operation::new(
                    context,
                    Self::get_concrete_op_info(),
                    vec![],
                    vec![],
                    vec![],
                    0,
                );
                let op = Self::from_operation(operation);
                op.$setter(context, attribute);
                op
            }
        }

        impl Verify for $name {
            fn verify(&self, context: &Context) -> PlironResult<()> {
                let operation = self.get_operation();
                let operation = operation.deref(context);
                if operation.get_num_operands() != 0
                    || operation.get_num_results() != 0
                    || operation.get_num_successors() != 0
                    || operation.num_regions() != 0
                    || operation.attributes.0.len() != 1
                    || self.$getter(context).is_none()
                {
                    return verify_err!(
                        self.loc(context),
                        "{} has a malformed or unbounded payload",
                        self.get_opid()
                    );
                }
                Ok(())
            }
        }
    };
}

impl_closed_record_op!(
    GeneralGemmRuntimeAbiOp,
    set_attr_general_gemm_runtime_abi,
    get_attr_general_gemm_runtime_abi,
    GeneralGemmRuntimeAbiAttr
);
impl_closed_record_op!(
    GeneralGemmGridMappingOp,
    set_attr_general_gemm_grid_mapping,
    get_attr_general_gemm_grid_mapping,
    GeneralGemmGridMappingAttr
);
impl_closed_record_op!(
    GeneralGemmPhaseLoopOp,
    set_attr_general_gemm_phase_loop,
    get_attr_general_gemm_phase_loop,
    GeneralGemmPhaseLoopAttr
);
impl_closed_record_op!(
    GeneralGemmGlobalTransferOp,
    set_attr_general_gemm_global_transfer,
    get_attr_general_gemm_global_transfer,
    GeneralGemmGlobalTransferAttr
);
impl_closed_record_op!(
    GeneralGemmLdsTransferOp,
    set_attr_general_gemm_lds_transfer,
    get_attr_general_gemm_lds_transfer,
    GeneralGemmLdsTransferAttr
);
impl_closed_record_op!(
    GeneralGemmEpochOp,
    set_attr_general_gemm_epoch,
    get_attr_general_gemm_epoch,
    GeneralGemmEpochAttr
);
impl_closed_record_op!(
    GeneralGemmMfmaOp,
    set_attr_general_gemm_mfma,
    get_attr_general_gemm_mfma,
    GeneralGemmMfmaAttr
);
impl_closed_record_op!(
    GeneralGemmEpilogueOp,
    set_attr_general_gemm_epilogue,
    get_attr_general_gemm_epilogue,
    GeneralGemmEpilogueAttr
);
