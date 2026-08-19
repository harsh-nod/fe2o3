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

/// Exact dynamic ABI schema for the first general-GEMM source profile.
#[pliron_attr(name = "kernel.general_gemm_abi", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmAbiSchemaAttr {
    /// A/B shared BF16 slices, disjoint f32 C, M/N/K, lda/ldb/ldc, alpha, beta.
    DynamicElevenArgumentBf16F32V1,
}

/// Exact source-level epilogue schema.
#[pliron_attr(name = "kernel.general_gemm_epilogue", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmEpilogueSchemaAttr {
    /// Four guarded disjoint C owners compute `alpha * accumulator + beta * C`.
    GuardedDisjointAlphaAccPlusBetaCV1,
}

/// Structured dynamic general-GEMM algorithm semantics.
#[pliron_op(
    name = "kernel.general_gemm",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        kernel_general_gemm_abi: GeneralGemmAbiSchemaAttr,
        kernel_general_gemm_epilogue: GeneralGemmEpilogueSchemaAttr
    )
)]
pub struct GeneralGemmOp;

impl GeneralGemmOp {
    /// Builds the one closed dynamic ABI and epilogue profile.
    pub fn canonical(context: &mut Context) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_general_gemm_abi(
            context,
            GeneralGemmAbiSchemaAttr::DynamicElevenArgumentBf16F32V1,
        );
        op.set_attr_kernel_general_gemm_epilogue(
            context,
            GeneralGemmEpilogueSchemaAttr::GuardedDisjointAlphaAccPlusBetaCV1,
        );
        op
    }
}

impl Verify for GeneralGemmOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let operation = operation.deref(context);
        if operation.get_num_operands() != 0
            || operation.get_num_results() != 0
            || operation.get_num_successors() != 0
            || operation.num_regions() != 0
            || operation.attributes.0.len() != 2
            || self
                .get_attr_kernel_general_gemm_abi(context)
                .is_none_or(|value| {
                    *value != GeneralGemmAbiSchemaAttr::DynamicElevenArgumentBf16F32V1
                })
            || self
                .get_attr_kernel_general_gemm_epilogue(context)
                .is_none_or(|value| {
                    *value != GeneralGemmEpilogueSchemaAttr::GuardedDisjointAlphaAccPlusBetaCV1
                })
        {
            return verify_err!(
                self.loc(context),
                "kernel.general_gemm has a non-canonical semantic payload"
            );
        }
        Ok(())
    }
}
