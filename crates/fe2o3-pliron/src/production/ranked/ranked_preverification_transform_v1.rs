//! Closed, deterministic ranked-recipe transformation pipeline.
//!
//! This pipeline runs before live PLIRON construction. The subsequent nine
//! production analyses therefore retain their exact-identity-only contract.

use super::{
    ProductionRankedKernelV1,
    ranked_cfg_ssa_canonicalize_v1::{
        canonicalize_and_validate_cfg_ssa_v1, require_canonical_cfg_ssa_v1,
    },
    ranked_index_constant_fold_v1::{
        ProductionRankedTranslationErrorV1, fold_and_validate_index_constants_v1,
        require_folded_index_constants_v1,
    },
    ranked_memory_type_legalize_v1::{
        legalize_and_validate_memory_types_v1, require_explicit_memory_types_v1,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRankedPreverificationPassV1 {
    CanonicalizeCfgSsaEdges,
    LegalizeMemoryTypes,
    CanonicalizeIndexExpressions,
}

pub const PRODUCTION_RANKED_PREVERIFICATION_PASS_ORDER_V1: [ProductionRankedPreverificationPassV1;
    3] = [
    ProductionRankedPreverificationPassV1::CanonicalizeCfgSsaEdges,
    ProductionRankedPreverificationPassV1::LegalizeMemoryTypes,
    ProductionRankedPreverificationPassV1::CanonicalizeIndexExpressions,
];

pub(super) fn transform_and_validate_ranked_preverification_v1(
    mut kernel: ProductionRankedKernelV1,
) -> Result<ProductionRankedKernelV1, ProductionRankedTranslationErrorV1> {
    for pass in PRODUCTION_RANKED_PREVERIFICATION_PASS_ORDER_V1 {
        kernel = match pass {
            ProductionRankedPreverificationPassV1::CanonicalizeCfgSsaEdges => {
                canonicalize_and_validate_cfg_ssa_v1(kernel)?
            }
            ProductionRankedPreverificationPassV1::LegalizeMemoryTypes => {
                legalize_and_validate_memory_types_v1(kernel)?
            }
            ProductionRankedPreverificationPassV1::CanonicalizeIndexExpressions => {
                fold_and_validate_index_constants_v1(kernel)?
            }
        };
    }
    require_ranked_preverification_normal_form_v1(&kernel)?;
    Ok(kernel)
}

pub(super) fn require_ranked_preverification_normal_form_v1(
    kernel: &ProductionRankedKernelV1,
) -> Result<(), ProductionRankedTranslationErrorV1> {
    require_canonical_cfg_ssa_v1(kernel)?;
    require_explicit_memory_types_v1(kernel)?;
    require_folded_index_constants_v1(kernel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{
        ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedTerminatorV1,
        ProductionRankedValueIdV1, ProductionRankedValueV1,
    };
    use dialect_kernel::MemorySpaceAttr;

    fn local(value: u32) -> ProductionRankedValueV1 {
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(value))
    }

    fn raw_kernel() -> ProductionRankedKernelV1 {
        let mut kernel = ProductionRankedKernelV1 {
            function_name: "preverification".to_owned(),
            argument_count: 0,
            blocks: vec![
                ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::View {
                            result: ProductionRankedValueIdV1::new(0),
                            element_width: 32,
                            writable: false,
                            shape: vec![16],
                            dynamic_extents: Vec::new(),
                            allocation_origin: 1,
                            noalias_class: 1,
                        },
                        ProductionRankedOperationV1::IndexConstant {
                            result: ProductionRankedValueIdV1::new(1),
                            value: 0x1ff,
                        },
                        ProductionRankedOperationV1::IndexUnsignedCast {
                            result: ProductionRankedValueIdV1::new(2),
                            source: local(1),
                            bit_width: 8,
                        },
                    ],
                    ProductionRankedTerminatorV1::BranchArgs {
                        arguments: Vec::new(),
                        target: 1,
                    },
                ),
                ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
            ],
            tree_work: 0,
        };
        kernel.tree_work = kernel.validate().expect("valid raw recipe");
        kernel
    }

    #[test]
    fn fixed_order_produces_one_position_preserving_normal_form() {
        assert_eq!(
            PRODUCTION_RANKED_PREVERIFICATION_PASS_ORDER_V1,
            [
                ProductionRankedPreverificationPassV1::CanonicalizeCfgSsaEdges,
                ProductionRankedPreverificationPassV1::LegalizeMemoryTypes,
                ProductionRankedPreverificationPassV1::CanonicalizeIndexExpressions,
            ]
        );
        let before = raw_kernel();
        let after = transform_and_validate_ranked_preverification_v1(before.clone()).unwrap();
        assert_eq!(after.blocks.len(), before.blocks.len());
        assert_eq!(
            after.blocks[0].operations.len(),
            before.blocks[0].operations.len()
        );
        assert_eq!(after.tree_work, before.tree_work);
        assert!(matches!(
            after.blocks[0].terminator,
            ProductionRankedTerminatorV1::Branch { target: 1 }
        ));
        assert!(matches!(
            after.blocks[0].operations[0],
            ProductionRankedOperationV1::ViewInSpace {
                memory_space: MemorySpaceAttr::Global,
                ..
            }
        ));
        assert!(matches!(
            after.blocks[0].operations[2],
            ProductionRankedOperationV1::IndexConstant { value: 0xff, .. }
        ));
        require_ranked_preverification_normal_form_v1(&after).unwrap();
    }

    #[test]
    fn production_transform_is_idempotent() {
        let once = transform_and_validate_ranked_preverification_v1(raw_kernel()).unwrap();
        let twice = transform_and_validate_ranked_preverification_v1(once.clone()).unwrap();
        assert_eq!(twice, once);
    }

    #[test]
    fn public_constructor_runs_the_complete_preverification_pipeline() {
        let raw = raw_kernel();
        let kernel =
            ProductionRankedKernelV1::new(&raw.function_name, raw.argument_count, raw.blocks)
                .expect("production constructor must run checked preverification transforms");
        assert!(matches!(
            kernel.blocks()[0].terminator(),
            ProductionRankedTerminatorV1::Branch { target: 1 }
        ));
        assert!(matches!(
            kernel.blocks()[0].operations()[0],
            ProductionRankedOperationV1::ViewInSpace {
                memory_space: MemorySpaceAttr::Global,
                ..
            }
        ));
        assert!(matches!(
            kernel.blocks()[0].operations()[2],
            ProductionRankedOperationV1::IndexConstant { value: 0xff, .. }
        ));
        require_ranked_preverification_normal_form_v1(&kernel).unwrap();
    }
}
