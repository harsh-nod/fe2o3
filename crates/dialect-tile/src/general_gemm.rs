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

/// Exact lane, fragment, and LDS mapping for the wave64 general-GEMM tile.
#[pliron_attr(name = "tile.general_gemm_xor4_mapping", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneralGemmXor4MappingAttr {
    /// Grid XY maps 16x16 output tiles; wave64 owns four outputs and XOR4 LDS banks.
    GridXy16Wave64FourComponentsV1,
}

/// Materialized exact wave64/XOR4 general-GEMM mapping.
#[pliron_op(
    name = "tile.general_gemm_xor4",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (general_gemm_xor4_mapping: GeneralGemmXor4MappingAttr)
)]
pub struct GeneralGemmXor4Op;

impl GeneralGemmXor4Op {
    /// Builds the only admitted mapping.
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
        op.set_attr_general_gemm_xor4_mapping(
            context,
            GeneralGemmXor4MappingAttr::GridXy16Wave64FourComponentsV1,
        );
        op
    }
}

impl Verify for GeneralGemmXor4Op {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let operation = operation.deref(context);
        if operation.get_num_operands() != 0
            || operation.get_num_results() != 0
            || operation.get_num_successors() != 0
            || operation.num_regions() != 0
            || operation.attributes.0.len() != 1
            || self
                .get_attr_general_gemm_xor4_mapping(context)
                .is_none_or(|value| {
                    *value != GeneralGemmXor4MappingAttr::GridXy16Wave64FourComponentsV1
                })
        {
            return verify_err!(
                self.loc(context),
                "tile.general_gemm_xor4 has a non-canonical mapping payload"
            );
        }
        Ok(())
    }
}
