//! Sealed checked normalization for exact ranked index constants.
//!
//! The folder and validator deliberately use separate evaluators. The folder
//! proposes a same-site rewrite; the validator independently proves the exact
//! structural relation before the normalized recipe can leave its constructor.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use super::{
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1,
};
use dialect_kernel::IndexBinaryKindAttr;

const SEALED_RANKED_CONTEXT_V1: [u8; 32] = [0x31; 32];
const SEALED_FOLD_IMPLEMENTATION_V1: [u8; 32] = [0x52; 32];
const SEALED_FOLD_CONFIGURATION_V1: [u8; 32] = [0x73; 32];

static NEXT_RANKED_TRANSLATION_SESSION_V1: AtomicU64 = AtomicU64::new(1);

/// Fail-closed failure from the compiler-owned ranked translation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionRankedTranslationErrorV1 {
    SessionIdentityExhausted,
    ContextMismatch,
    ImplementationMismatch,
    ConfigurationMismatch,
    OutputBindingMismatch,
    FunctionIdentityChanged,
    FunctionSignatureChanged,
    BlockStructureChanged {
        block: usize,
        component: &'static str,
    },
    OperationChanged {
        block: usize,
        operation: usize,
        component: &'static str,
    },
    IllegalRewrite {
        block: usize,
        operation: usize,
    },
    IncorrectFoldedValue {
        block: usize,
        operation: usize,
        expected: u64,
        actual: u64,
    },
    MissedFold {
        block: usize,
        operation: usize,
    },
    TreeWorkChanged,
}

impl ProductionRankedTranslationErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SessionIdentityExhausted => "FE2O3-RANKED-TRANSFORM-001",
            Self::ContextMismatch | Self::ImplementationMismatch | Self::ConfigurationMismatch => {
                "FE2O3-RANKED-TRANSFORM-002"
            }
            Self::OutputBindingMismatch => "FE2O3-RANKED-TRANSFORM-003",
            Self::FunctionIdentityChanged | Self::FunctionSignatureChanged => {
                "FE2O3-RANKED-TRANSFORM-004"
            }
            Self::BlockStructureChanged { .. } => "FE2O3-RANKED-TRANSFORM-005",
            Self::OperationChanged { .. } => "FE2O3-RANKED-TRANSFORM-006",
            Self::IllegalRewrite { .. } => "FE2O3-RANKED-TRANSFORM-007",
            Self::IncorrectFoldedValue { .. } => "FE2O3-RANKED-TRANSFORM-008",
            Self::MissedFold { .. } => "FE2O3-RANKED-TRANSFORM-009",
            Self::TreeWorkChanged => "FE2O3-RANKED-TRANSFORM-010",
        }
    }
}

impl fmt::Display for ProductionRankedTranslationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::SessionIdentityExhausted => {
                formatter.write_str("ranked translation session identity space is exhausted")
            }
            Self::ContextMismatch => {
                formatter.write_str("ranked translation context binding changed")
            }
            Self::ImplementationMismatch => {
                formatter.write_str("ranked translation implementation binding changed")
            }
            Self::ConfigurationMismatch => {
                formatter.write_str("ranked translation configuration binding changed")
            }
            Self::OutputBindingMismatch => {
                formatter.write_str("ranked translation output changed after exact validation")
            }
            Self::FunctionIdentityChanged => {
                formatter.write_str("ranked translation changed the function identity")
            }
            Self::FunctionSignatureChanged => {
                formatter.write_str("ranked translation changed the function signature")
            }
            Self::BlockStructureChanged { block, component } => write!(
                formatter,
                "ranked translation changed {component} at block {block}"
            ),
            Self::OperationChanged {
                block,
                operation,
                component,
            } => write!(
                formatter,
                "ranked translation changed {component} at block {block}, operation {operation}"
            ),
            Self::IllegalRewrite { block, operation } => write!(
                formatter,
                "ranked translation attempted an unsupported rewrite at block {block}, operation {operation}"
            ),
            Self::IncorrectFoldedValue {
                block,
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "ranked translation produced {actual} instead of {expected} at block {block}, operation {operation}"
            ),
            Self::MissedFold { block, operation } => write!(
                formatter,
                "ranked translation output is not at a fixed point at block {block}, operation {operation}"
            ),
            Self::TreeWorkChanged => {
                formatter.write_str("ranked translation changed bounded operation-tree work")
            }
        }
    }
}

impl Error for ProductionRankedTranslationErrorV1 {}

struct RankedTranslationSessionV1 {
    session: NonZeroU64,
    context: [u8; 32],
    implementation: [u8; 32],
    configuration: [u8; 32],
    before: ProductionRankedKernelV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RankedTranslationDispositionV1 {
    Applied { rewrites: usize },
    NotApplicable,
}

#[derive(Debug)]
struct RankedTranslationReceiptV1 {
    session: NonZeroU64,
    context: [u8; 32],
    implementation: [u8; 32],
    configuration: [u8; 32],
    output: ProductionRankedKernelV1,
    disposition: RankedTranslationDispositionV1,
}

fn begin_translation_v1(
    before: ProductionRankedKernelV1,
) -> Result<RankedTranslationSessionV1, ProductionRankedTranslationErrorV1> {
    let session = NEXT_RANKED_TRANSLATION_SESSION_V1
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ProductionRankedTranslationErrorV1::SessionIdentityExhausted)?;
    let session = NonZeroU64::new(session)
        .ok_or(ProductionRankedTranslationErrorV1::SessionIdentityExhausted)?;
    Ok(RankedTranslationSessionV1 {
        session,
        context: SEALED_RANKED_CONTEXT_V1,
        implementation: SEALED_FOLD_IMPLEMENTATION_V1,
        configuration: SEALED_FOLD_CONFIGURATION_V1,
        before,
    })
}

fn fold_candidate_v1(kind: IndexBinaryKindAttr, lhs: u64, rhs: u64) -> Option<u64> {
    match kind {
        IndexBinaryKindAttr::Add => lhs.checked_add(rhs),
        IndexBinaryKindAttr::Multiply => lhs.checked_mul(rhs),
        IndexBinaryKindAttr::Divide if rhs != 0 => Some(lhs / rhs),
        IndexBinaryKindAttr::Remainder if rhs != 0 => Some(lhs % rhs),
        IndexBinaryKindAttr::Divide | IndexBinaryKindAttr::Remainder => None,
    }
}

fn preceding_constant_v1(
    constants: &BTreeMap<ProductionRankedValueIdV1, u64>,
    value: ProductionRankedValueV1,
) -> Option<u64> {
    let ProductionRankedValueV1::Local(identity) = value else {
        return None;
    };
    constants.get(&identity).copied()
}

fn run_folder_v1(kernel: &mut ProductionRankedKernelV1) {
    let mut constants = BTreeMap::new();
    for block in &mut kernel.blocks {
        for operation in &mut block.operations {
            let replacement = match operation {
                ProductionRankedOperationV1::IndexConstant { result, value } => {
                    constants.insert(*result, *value);
                    None
                }
                ProductionRankedOperationV1::IndexBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                } => preceding_constant_v1(&constants, *lhs)
                    .zip(preceding_constant_v1(&constants, *rhs))
                    .and_then(|(lhs, rhs)| fold_candidate_v1(*kind, lhs, rhs))
                    .map(|value| (*result, value)),
                _ => None,
            };
            if let Some((result, value)) = replacement {
                *operation = ProductionRankedOperationV1::IndexConstant { result, value };
                constants.insert(result, value);
            }
        }
    }
}

fn validator_evaluate_v1(kind: IndexBinaryKindAttr, lhs: u64, rhs: u64) -> Option<u64> {
    match kind {
        IndexBinaryKindAttr::Add => u64::checked_add(lhs, rhs),
        IndexBinaryKindAttr::Multiply => u64::checked_mul(lhs, rhs),
        IndexBinaryKindAttr::Divide => (rhs > 0).then(|| lhs / rhs),
        IndexBinaryKindAttr::Remainder => (rhs > 0).then(|| lhs % rhs),
    }
}

fn validator_constant_v1(
    constants: &BTreeMap<ProductionRankedValueIdV1, u64>,
    value: ProductionRankedValueV1,
) -> Option<u64> {
    match value {
        ProductionRankedValueV1::Local(identity) => constants.get(&identity).copied(),
        ProductionRankedValueV1::Argument(_) | ProductionRankedValueV1::BlockArgument { .. } => {
            None
        }
    }
}

fn replay_translation_v1(
    before: &ProductionRankedKernelV1,
    after: &ProductionRankedKernelV1,
) -> Result<usize, ProductionRankedTranslationErrorV1> {
    if before.function_name != after.function_name {
        return Err(ProductionRankedTranslationErrorV1::FunctionIdentityChanged);
    }
    if before.argument_count != after.argument_count {
        return Err(ProductionRankedTranslationErrorV1::FunctionSignatureChanged);
    }
    if before.blocks.len() != after.blocks.len() {
        return Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
            block: before.blocks.len().min(after.blocks.len()),
            component: "block count",
        });
    }
    if before.tree_work != after.tree_work {
        return Err(ProductionRankedTranslationErrorV1::TreeWorkChanged);
    }

    let mut constants = BTreeMap::new();
    let mut rewrites = 0_usize;
    for (block_index, (before_block, after_block)) in
        before.blocks.iter().zip(&after.blocks).enumerate()
    {
        if before_block.index_argument_count != after_block.index_argument_count {
            return Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
                block: block_index,
                component: "block argument types",
            });
        }
        if before_block.terminator != after_block.terminator {
            return Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
                block: block_index,
                component: "CFG terminator or successor operands",
            });
        }
        if before_block.operations.len() != after_block.operations.len() {
            return Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
                block: block_index,
                component: "operation count or positions",
            });
        }
        for (operation_index, (before_operation, after_operation)) in before_block
            .operations
            .iter()
            .zip(&after_block.operations)
            .enumerate()
        {
            if before_operation == after_operation {
                if let ProductionRankedOperationV1::IndexBinary { kind, lhs, rhs, .. } =
                    after_operation
                    && let Some(expected) = validator_constant_v1(&constants, *lhs)
                        .zip(validator_constant_v1(&constants, *rhs))
                        .and_then(|(lhs, rhs)| validator_evaluate_v1(*kind, lhs, rhs))
                {
                    let _ = expected;
                    return Err(ProductionRankedTranslationErrorV1::MissedFold {
                        block: block_index,
                        operation: operation_index,
                    });
                }
            } else {
                let (
                    ProductionRankedOperationV1::IndexBinary {
                        result,
                        kind,
                        lhs,
                        rhs,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: after_result,
                        value: actual,
                    },
                ) = (before_operation, after_operation)
                else {
                    return Err(ProductionRankedTranslationErrorV1::OperationChanged {
                        block: block_index,
                        operation: operation_index,
                        component: "operation kind, attributes, operands, result type, effect, or proof site",
                    });
                };
                if result != after_result {
                    return Err(ProductionRankedTranslationErrorV1::OperationChanged {
                        block: block_index,
                        operation: operation_index,
                        component: "SSA result identity",
                    });
                }
                let expected = validator_constant_v1(&constants, *lhs)
                    .zip(validator_constant_v1(&constants, *rhs))
                    .and_then(|(lhs, rhs)| validator_evaluate_v1(*kind, lhs, rhs))
                    .ok_or(ProductionRankedTranslationErrorV1::IllegalRewrite {
                        block: block_index,
                        operation: operation_index,
                    })?;
                if expected != *actual {
                    return Err(ProductionRankedTranslationErrorV1::IncorrectFoldedValue {
                        block: block_index,
                        operation: operation_index,
                        expected,
                        actual: *actual,
                    });
                }
                rewrites += 1;
            }

            if let ProductionRankedOperationV1::IndexConstant { result, value } = after_operation {
                constants.insert(*result, *value);
            }
        }
    }
    Ok(rewrites)
}

fn finish_translation_v1(
    session: RankedTranslationSessionV1,
    after: &ProductionRankedKernelV1,
) -> Result<RankedTranslationReceiptV1, ProductionRankedTranslationErrorV1> {
    let RankedTranslationSessionV1 {
        session,
        context,
        implementation,
        configuration,
        before,
    } = session;
    if context != SEALED_RANKED_CONTEXT_V1 {
        return Err(ProductionRankedTranslationErrorV1::ContextMismatch);
    }
    if implementation != SEALED_FOLD_IMPLEMENTATION_V1 {
        return Err(ProductionRankedTranslationErrorV1::ImplementationMismatch);
    }
    if configuration != SEALED_FOLD_CONFIGURATION_V1 {
        return Err(ProductionRankedTranslationErrorV1::ConfigurationMismatch);
    }
    let disposition = match replay_translation_v1(&before, after)? {
        0 => RankedTranslationDispositionV1::NotApplicable,
        rewrites => RankedTranslationDispositionV1::Applied { rewrites },
    };
    drop(before);
    Ok(RankedTranslationReceiptV1 {
        session,
        context,
        implementation,
        configuration,
        output: after.clone(),
        disposition,
    })
}

fn consume_translation_v1(
    receipt: RankedTranslationReceiptV1,
    after: &ProductionRankedKernelV1,
) -> Result<RankedTranslationDispositionV1, ProductionRankedTranslationErrorV1> {
    if receipt.session.get() == 0
        || receipt.context != SEALED_RANKED_CONTEXT_V1
        || receipt.implementation != SEALED_FOLD_IMPLEMENTATION_V1
        || receipt.configuration != SEALED_FOLD_CONFIGURATION_V1
    {
        return Err(ProductionRankedTranslationErrorV1::ContextMismatch);
    }
    if receipt.output != *after {
        return Err(ProductionRankedTranslationErrorV1::OutputBindingMismatch);
    }
    Ok(receipt.disposition)
}

pub(super) fn fold_and_validate_index_constants_v1(
    before: ProductionRankedKernelV1,
) -> Result<ProductionRankedKernelV1, ProductionRankedTranslationErrorV1> {
    let session = begin_translation_v1(before)?;
    let mut after = session.before.clone();
    run_folder_v1(&mut after);
    let receipt = finish_translation_v1(session, &after)?;
    consume_translation_v1(receipt, &after)?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{ProductionRankedBlockV1, ProductionRankedTerminatorV1};
    use dialect_kernel::AccessKindAttr;
    use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
    use fe2o3_proof_contracts::DigestV1;

    fn constant(identity: u32, value: u64) -> ProductionRankedOperationV1 {
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(identity),
            value,
        }
    }

    fn binary(
        identity: u32,
        kind: IndexBinaryKindAttr,
        lhs: u32,
        rhs: u32,
    ) -> ProductionRankedOperationV1 {
        ProductionRankedOperationV1::IndexBinary {
            result: ProductionRankedValueIdV1::new(identity),
            kind,
            lhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(lhs)),
            rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(rhs)),
        }
    }

    fn raw_kernel(
        function_name: &str,
        argument_count: usize,
        operations: Vec<ProductionRankedOperationV1>,
    ) -> ProductionRankedKernelV1 {
        let mut kernel = ProductionRankedKernelV1 {
            function_name: function_name.to_owned(),
            argument_count,
            blocks: vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
            tree_work: 0,
        };
        kernel.tree_work = kernel.validate().expect("valid raw test recipe");
        kernel
    }

    fn folded_value(kernel: &ProductionRankedKernelV1, operation: usize) -> Option<u64> {
        match &kernel.blocks[0].operations[operation] {
            ProductionRankedOperationV1::IndexConstant { value, .. } => Some(*value),
            _ => None,
        }
    }

    #[test]
    fn folds_each_supported_checked_operation() {
        for (kind, lhs, rhs, expected) in [
            (IndexBinaryKindAttr::Add, 9, 4, 13),
            (IndexBinaryKindAttr::Multiply, 9, 4, 36),
            (IndexBinaryKindAttr::Divide, 9, 4, 2),
            (IndexBinaryKindAttr::Remainder, 9, 4, 1),
        ] {
            let before = raw_kernel(
                "checked_fold",
                0,
                vec![constant(0, lhs), constant(1, rhs), binary(2, kind, 0, 1)],
            );
            let after = fold_and_validate_index_constants_v1(before).expect("checked fold");
            assert_eq!(folded_value(&after, 2), Some(expected));
        }
    }

    #[test]
    fn reaches_a_forward_fixed_point_for_chained_constants() {
        let before = raw_kernel(
            "chained_fold",
            0,
            vec![
                constant(0, 5),
                constant(1, 7),
                binary(2, IndexBinaryKindAttr::Add, 0, 1),
                constant(3, 2),
                binary(4, IndexBinaryKindAttr::Multiply, 2, 3),
            ],
        );
        let after = fold_and_validate_index_constants_v1(before).expect("chained fold");
        assert_eq!(folded_value(&after, 2), Some(12));
        assert_eq!(folded_value(&after, 4), Some(24));
    }

    #[test]
    fn overflow_and_zero_divisors_remain_unfolded() {
        for (kind, lhs, rhs) in [
            (IndexBinaryKindAttr::Add, u64::MAX, 1),
            (IndexBinaryKindAttr::Multiply, u64::MAX, 2),
            (IndexBinaryKindAttr::Divide, 1, 0),
            (IndexBinaryKindAttr::Remainder, 1, 0),
        ] {
            let before = raw_kernel(
                "undefined_fold",
                0,
                vec![constant(0, lhs), constant(1, rhs), binary(2, kind, 0, 1)],
            );
            let after =
                fold_and_validate_index_constants_v1(before).expect("noncandidate is retained");
            assert!(matches!(
                after.blocks[0].operations[2],
                ProductionRankedOperationV1::IndexBinary { .. }
            ));
        }
    }

    #[test]
    fn arguments_and_block_values_are_not_treated_as_constants() {
        let before = raw_kernel(
            "dynamic_operand",
            1,
            vec![
                constant(0, 2),
                ProductionRankedOperationV1::IndexBinary {
                    result: ProductionRankedValueIdV1::new(1),
                    kind: IndexBinaryKindAttr::Add,
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                },
            ],
        );
        let after =
            fold_and_validate_index_constants_v1(before).expect("dynamic binary is retained");
        assert!(matches!(
            after.blocks[0].operations[1],
            ProductionRankedOperationV1::IndexBinary { .. }
        ));
    }

    #[test]
    fn validator_rejects_an_incorrect_folded_value() {
        let before = raw_kernel(
            "wrong_value",
            0,
            vec![
                constant(0, 4),
                constant(1, 3),
                binary(2, IndexBinaryKindAttr::Multiply, 0, 1),
            ],
        );
        let mut after = before.clone();
        run_folder_v1(&mut after);
        after.blocks[0].operations[2] = constant(2, 13);
        assert_eq!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::IncorrectFoldedValue {
                block: 0,
                operation: 2,
                expected: 12,
                actual: 13,
            })
        );
    }

    #[test]
    fn validator_rejects_a_missed_fold() {
        let before = raw_kernel(
            "missed_fold",
            0,
            vec![
                constant(0, 4),
                constant(1, 3),
                binary(2, IndexBinaryKindAttr::Add, 0, 1),
            ],
        );
        assert_eq!(
            replay_translation_v1(&before, &before),
            Err(ProductionRankedTranslationErrorV1::MissedFold {
                block: 0,
                operation: 2,
            })
        );
    }

    #[test]
    fn validator_rejects_a_fold_without_two_preceding_constants() {
        let before = raw_kernel(
            "illegal_fold",
            1,
            vec![
                constant(0, 3),
                ProductionRankedOperationV1::IndexBinary {
                    result: ProductionRankedValueIdV1::new(1),
                    kind: IndexBinaryKindAttr::Add,
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                },
            ],
        );
        let mut after = before.clone();
        after.blocks[0].operations[1] = constant(1, 9);
        assert_eq!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::IllegalRewrite {
                block: 0,
                operation: 1,
            })
        );
    }

    #[test]
    fn validator_rejects_function_signature_and_cfg_changes() {
        let before = raw_kernel("structure", 1, vec![]);
        let mut renamed = before.clone();
        renamed.function_name = "renamed".to_owned();
        assert_eq!(
            replay_translation_v1(&before, &renamed),
            Err(ProductionRankedTranslationErrorV1::FunctionIdentityChanged)
        );

        let mut signature = before.clone();
        signature.argument_count = 2;
        assert_eq!(
            replay_translation_v1(&before, &signature),
            Err(ProductionRankedTranslationErrorV1::FunctionSignatureChanged)
        );

        let mut cfg = before.clone();
        cfg.blocks[0].terminator = ProductionRankedTerminatorV1::Trap;
        assert!(matches!(
            replay_translation_v1(&before, &cfg),
            Err(ProductionRankedTranslationErrorV1::BlockStructureChanged {
                block: 0,
                component: "CFG terminator or successor operands",
            })
        ));
    }

    #[test]
    fn validator_rejects_operation_and_ssa_identity_changes() {
        let before = raw_kernel(
            "operation_change",
            0,
            vec![
                constant(0, 2),
                constant(1, 3),
                binary(2, IndexBinaryKindAttr::Add, 0, 1),
            ],
        );
        let mut after = before.clone();
        run_folder_v1(&mut after);
        after.blocks[0].operations[0] = ProductionRankedOperationV1::IndexUnknown {
            result: ProductionRankedValueIdV1::new(0),
        };
        assert!(matches!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::OperationChanged {
                block: 0,
                operation: 0,
                ..
            })
        ));

        let mut after = before.clone();
        run_folder_v1(&mut after);
        after.blocks[0].operations[2] = constant(9, 5);
        assert!(matches!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::OperationChanged {
                block: 0,
                operation: 2,
                component: "SSA result identity",
            })
        ));
    }

    #[test]
    fn validator_rejects_type_effect_and_proof_site_changes() {
        let before = raw_kernel(
            "type_change",
            0,
            vec![ProductionRankedOperationV1::View {
                result: ProductionRankedValueIdV1::new(0),
                element_width: 32,
                writable: false,
                shape: vec![1],
                dynamic_extents: vec![],
                allocation_origin: 1,
                noalias_class: 1,
            }],
        );
        let mut after = before.clone();
        let ProductionRankedOperationV1::View { element_width, .. } =
            &mut after.blocks[0].operations[0]
        else {
            panic!("view operation");
        };
        *element_width = 16;
        assert!(matches!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::OperationChanged {
                block: 0,
                operation: 0,
                component: "operation kind, attributes, operands, result type, effect, or proof site",
            })
        ));

        let before = raw_kernel(
            "effect_change",
            0,
            vec![
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                constant(1, 0),
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                    indices: vec![ProductionRankedValueV1::Local(
                        ProductionRankedValueIdV1::new(1),
                    )],
                },
            ],
        );
        let mut after = before.clone();
        let ProductionRankedOperationV1::Access { kind, .. } = &mut after.blocks[0].operations[2]
        else {
            panic!("access operation");
        };
        *kind = AccessKindAttr::Write;
        assert!(matches!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::OperationChanged {
                block: 0,
                operation: 2,
                component: "operation kind, attributes, operands, result type, effect, or proof site",
            })
        ));

        let subjects = FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            DigestV1::from_untrusted_bytes([1; 32]),
            DigestV1::ZERO,
            DigestV1::from_untrusted_bytes([2; 32]),
            DigestV1::from_untrusted_bytes([3; 32]),
            DigestV1::from_untrusted_bytes([4; 32]),
        )
        .expect("subjects");
        let before = raw_kernel(
            "proof_site_change",
            0,
            vec![
                ProductionRankedOperationV1::SemanticConstant {
                    result: ProductionRankedValueIdV1::new(0),
                    value: 9,
                },
                ProductionRankedOperationV1::SemanticConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 9,
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                    expected: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                    subjects,
                },
            ],
        );
        let mut after = before.clone();
        let ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            ..
        } = &mut after.blocks[0].operations[2]
        else {
            panic!("proof request");
        };
        core::mem::swap(actual, expected);
        assert!(matches!(
            replay_translation_v1(&before, &after),
            Err(ProductionRankedTranslationErrorV1::OperationChanged {
                block: 0,
                operation: 2,
                component: "operation kind, attributes, operands, result type, effect, or proof site",
            })
        ));
    }

    #[test]
    fn session_rejects_sealed_identity_changes() {
        let before = raw_kernel("session_binding", 0, vec![]);

        let mut wrong_context = begin_translation_v1(before.clone()).expect("session");
        wrong_context.context[0] ^= 1;
        assert_eq!(
            finish_translation_v1(wrong_context, &before).unwrap_err(),
            ProductionRankedTranslationErrorV1::ContextMismatch
        );

        let mut wrong_implementation = begin_translation_v1(before.clone()).expect("session");
        wrong_implementation.implementation[0] ^= 1;
        assert_eq!(
            finish_translation_v1(wrong_implementation, &before).unwrap_err(),
            ProductionRankedTranslationErrorV1::ImplementationMismatch
        );

        let mut wrong_configuration = begin_translation_v1(before.clone()).expect("session");
        wrong_configuration.configuration[0] ^= 1;
        assert_eq!(
            finish_translation_v1(wrong_configuration, &before).unwrap_err(),
            ProductionRankedTranslationErrorV1::ConfigurationMismatch
        );
    }

    #[test]
    fn consumed_receipt_rejects_a_stale_output() {
        let before = raw_kernel(
            "output_binding",
            0,
            vec![
                constant(0, 2),
                constant(1, 3),
                binary(2, IndexBinaryKindAttr::Add, 0, 1),
            ],
        );
        let session = begin_translation_v1(before.clone()).expect("session");
        let mut after = session.before.clone();
        run_folder_v1(&mut after);
        let receipt = finish_translation_v1(session, &after).expect("validated receipt");
        after.blocks[0].operations[2] = constant(2, 6);
        assert_eq!(
            consume_translation_v1(receipt, &after),
            Err(ProductionRankedTranslationErrorV1::OutputBindingMismatch)
        );

        let session = begin_translation_v1(before).expect("session");
        let mut after = session.before.clone();
        run_folder_v1(&mut after);
        let mut receipt = finish_translation_v1(session, &after).expect("validated receipt");
        receipt.output.blocks[0].operations[2] = constant(2, 6);
        assert_eq!(
            consume_translation_v1(receipt, &after),
            Err(ProductionRankedTranslationErrorV1::OutputBindingMismatch)
        );
    }

    #[test]
    fn identity_translation_is_valid_when_no_candidate_exists() {
        let before = raw_kernel(
            "not_applicable",
            1,
            vec![ProductionRankedOperationV1::IndexUnknown {
                result: ProductionRankedValueIdV1::new(0),
            }],
        );
        let after = fold_and_validate_index_constants_v1(before.clone())
            .expect("identity translation is sealed");
        assert_eq!(before, after);
    }

    #[test]
    fn receipt_distinguishes_applied_and_not_applicable() {
        let unchanged = raw_kernel("not_applicable_receipt", 0, vec![constant(0, 1)]);
        let session = begin_translation_v1(unchanged.clone()).expect("session");
        let receipt = finish_translation_v1(session, &unchanged).expect("identity relation");
        assert_eq!(
            consume_translation_v1(receipt, &unchanged).expect("consume"),
            RankedTranslationDispositionV1::NotApplicable
        );

        let before = raw_kernel(
            "applied_receipt",
            0,
            vec![
                constant(0, 1),
                constant(1, 2),
                binary(2, IndexBinaryKindAttr::Add, 0, 1),
            ],
        );
        let session = begin_translation_v1(before).expect("session");
        let mut after = session.before.clone();
        run_folder_v1(&mut after);
        let receipt = finish_translation_v1(session, &after).expect("fold relation");
        assert_eq!(
            consume_translation_v1(receipt, &after).expect("consume"),
            RankedTranslationDispositionV1::Applied { rewrites: 1 }
        );
    }

    #[test]
    fn downstream_graph_identity_observes_the_normalized_recipe() {
        let folded = ProductionRankedKernelV1::new(
            "identity_binding",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    constant(0, 5),
                    constant(1, 7),
                    binary(2, IndexBinaryKindAttr::Add, 0, 1),
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .expect("folded recipe");
        let direct = ProductionRankedKernelV1::new(
            "identity_binding",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![constant(0, 5), constant(1, 7), constant(2, 12)],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .expect("direct recipe");
        assert_eq!(folded, direct);
        assert_eq!(
            super::super::super::middle_end_evidence_v4::derive_exact_ranked_graph_identity_v1(
                &folded
            ),
            super::super::super::middle_end_evidence_v4::derive_exact_ranked_graph_identity_v1(
                &direct
            )
        );
    }

    #[test]
    fn proof_request_and_formula_identities_are_derived_after_folding() {
        let subjects = FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            DigestV1::from_untrusted_bytes([1; 32]),
            DigestV1::ZERO,
            DigestV1::from_untrusted_bytes([2; 32]),
            DigestV1::from_untrusted_bytes([3; 32]),
            DigestV1::from_untrusted_bytes([4; 32]),
        )
        .expect("subjects");
        let request = ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(3)),
            expected: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(4)),
            subjects,
        };
        let folded = ProductionRankedKernelV1::new(
            "proof_binding",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    constant(0, 5),
                    constant(1, 7),
                    binary(2, IndexBinaryKindAttr::Add, 0, 1),
                    ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(3),
                        value: 11,
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(4),
                        value: 11,
                    },
                    request.clone(),
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .expect("folded proof request");
        let direct = ProductionRankedKernelV1::new(
            "proof_binding",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    constant(0, 5),
                    constant(1, 7),
                    constant(2, 12),
                    ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(3),
                        value: 11,
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(4),
                        value: 11,
                    },
                    request,
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .expect("direct proof request");
        assert_eq!(
            super::super::super::middle_end_evidence_v4::derive_functional_refinement_graph_identity_v2(
                &folded,
            ),
            super::super::super::middle_end_evidence_v4::derive_functional_refinement_graph_identity_v2(
                &direct,
            ),
        );
        let actual = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(3));
        let expected = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(4));
        assert_eq!(
            super::super::normalized_functional_refinement_formula_hash_for_kernel_v2(
                &folded, 0, 5, actual, expected, subjects,
            )
            .expect("folded formula"),
            super::super::normalized_functional_refinement_formula_hash_for_kernel_v2(
                &direct, 0, 5, actual, expected, subjects,
            )
            .expect("direct formula"),
        );
    }

    #[test]
    fn diagnostic_codes_are_stable_and_distinguish_relation_failures() {
        assert_eq!(
            ProductionRankedTranslationErrorV1::MissedFold {
                block: 0,
                operation: 0,
            }
            .code(),
            "FE2O3-RANKED-TRANSFORM-009"
        );
        assert_eq!(
            ProductionRankedTranslationErrorV1::IncorrectFoldedValue {
                block: 0,
                operation: 0,
                expected: 1,
                actual: 2,
            }
            .code(),
            "FE2O3-RANKED-TRANSFORM-008"
        );
    }
}
