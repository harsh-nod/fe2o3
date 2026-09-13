use crate::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, Function, Type, ValueId,
    VerificationNumericIndexRowV1, VerificationNumericIndexV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VerificationDefinitionSiteV1 {
    FunctionParameter,
    BlockParameter(BlockId),
    Operation(BlockId, usize),
}

#[derive(Clone, Copy)]
pub(crate) struct VerificationDefinitionV1<'module> {
    pub(crate) ty: &'module Type,
    pub(crate) site: VerificationDefinitionSiteV1,
}

pub(crate) struct VerificationFunctionStateV1<'module> {
    blocks: VerificationNumericIndexV1<&'module BasicBlock>,
    definitions: VerificationNumericIndexV1<VerificationDefinitionV1<'module>>,
}

impl<'module> VerificationFunctionStateV1<'module> {
    // Retain conservative per-row bounds with one spare cell. Stable radix
    // ordering does not require a stored input ordinal.
    const BLOCK_ROW_STORAGE: usize = 3;
    const DEFINITION_ROW_STORAGE: usize = 6;

    pub(crate) fn build(
        function: &'module Function,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<Self>, CanonicalKernelIrVerificationResourceErrorV1> {
        let checkpoint = budget.storage_checkpoint();
        match Self::build_inner(function, budget) {
            Ok(built) => Ok(built),
            Err(error) => {
                let _ = budget.rollback_storage(checkpoint);
                Err(error)
            }
        }
    }

    fn build_inner(
        function: &'module Function,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<Self>, CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(body) = function.body.as_ref() else {
            budget.charge_work(1)?;
            return Ok(None);
        };

        let block_count = body.blocks.len();
        let mut blocks = body.blocks.iter();
        let blocks = VerificationNumericIndexV1::build(
            block_count,
            Self::BLOCK_ROW_STORAGE,
            budget,
            |_| {
                let block = blocks
                    .next()
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
                Ok(VerificationNumericIndexRowV1 {
                    key: block.id.0,
                    value: block,
                })
            },
        )?;

        let function_parameter_count = body
            .parameters
            .len()
            .min(function.signature.parameters.len());
        budget.charge_work(block_count)?;
        let mut nested_visits = 0_usize;
        let mut nested_definitions = 0_usize;
        for block in &body.blocks {
            nested_definitions = nested_definitions
                .checked_add(block.parameters.len())
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            nested_visits = nested_visits
                .checked_add(block.operations.len())
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        }
        budget.charge_work(block_count)?;
        budget.charge_work(nested_visits)?;
        for block in &body.blocks {
            for operation in &block.operations {
                nested_definitions = nested_definitions
                    .checked_add(operation.results.len())
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            }
        }
        let definition_count = function_parameter_count
            .checked_add(nested_definitions)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;

        // The flattened source iterator walks the block and operation headers
        // a second time while the index owns the exact definition payload.
        budget.charge_work(block_count)?;
        budget.charge_work(nested_visits)?;
        let function_parameters = body
            .parameters
            .iter()
            .copied()
            .zip(function.signature.parameters.iter())
            .map(|(value, ty)| {
                (
                    value,
                    VerificationDefinitionV1 {
                        ty,
                        site: VerificationDefinitionSiteV1::FunctionParameter,
                    },
                )
            });
        let nested = body.blocks.iter().flat_map(|block| {
            let parameters = block.parameters.iter().map(|parameter| {
                (
                    parameter.id,
                    VerificationDefinitionV1 {
                        ty: &parameter.ty,
                        site: VerificationDefinitionSiteV1::BlockParameter(block.id),
                    },
                )
            });
            let results = block.operations.iter().enumerate().flat_map(
                move |(operation_ordinal, operation)| {
                    operation.results.iter().map(move |result| {
                        (
                            result.id,
                            VerificationDefinitionV1 {
                                ty: &result.ty,
                                site: VerificationDefinitionSiteV1::Operation(
                                    block.id,
                                    operation_ordinal,
                                ),
                            },
                        )
                    })
                },
            );
            parameters.chain(results)
        });
        let mut source_definitions = function_parameters.chain(nested);
        let definitions = VerificationNumericIndexV1::build(
            definition_count,
            Self::DEFINITION_ROW_STORAGE,
            budget,
            |_| {
                let (value, definition) = source_definitions
                    .next()
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
                Ok(VerificationNumericIndexRowV1 {
                    key: value.0,
                    value: definition,
                })
            },
        )?;
        // Authenticate that the full flattened B/O roster was consumed,
        // including empty-result suffixes after the last yielded definition.
        budget.charge_work(1)?;
        if source_definitions.next().is_some() {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }

        Ok(Some(Self {
            blocks,
            definitions,
        }))
    }

    pub(crate) fn block_rows(&self) -> &[VerificationNumericIndexRowV1<&'module BasicBlock>] {
        self.blocks.rows()
    }

    pub(crate) fn definition_rows(
        &self,
    ) -> &[VerificationNumericIndexRowV1<VerificationDefinitionV1<'module>>] {
        self.definitions.rows()
    }

    pub(crate) fn block(
        &self,
        block: BlockId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<&'module BasicBlock>, CanonicalKernelIrVerificationResourceErrorV1> {
        self.blocks.find(block.0, budget)
    }

    pub(crate) fn definition(
        &self,
        value: ValueId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        Option<VerificationDefinitionV1<'module>>,
        CanonicalKernelIrVerificationResourceErrorV1,
    > {
        self.definitions.find(value.0, budget)
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let definition_result = self.definitions.release(budget);
        let block_result = self.blocks.release(budget);
        definition_result.and(block_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanonicalKernelIrWorkBudgetV1, FunctionBody, FunctionRole, Operation, OperationKind,
        Signature, Terminator,
    };

    fn sparse_function() -> Function {
        Function {
            id: "sparse".into(),
            signature: Signature::new(vec![Type::INDEX], vec![]),
            role: FunctionRole::InternalHelper,
            body: Some(FunctionBody {
                parameters: vec![ValueId(u32::MAX)],
                blocks: vec![
                    BasicBlock {
                        id: BlockId(u32::MAX),
                        parameters: vec![],
                        operations: vec![Operation {
                            results: vec![crate::ValueDef {
                                id: ValueId(17),
                                ty: Type::INDEX,
                            }],
                            kind: OperationKind::Constant(crate::Constant::Index(1)),
                        }],
                        terminator: Some(Terminator::Return { values: vec![] }),
                    },
                    BasicBlock {
                        id: BlockId(3),
                        parameters: vec![crate::ValueDef {
                            id: ValueId(17),
                            ty: Type::INDEX,
                        }],
                        operations: vec![],
                        terminator: Some(Terminator::Return { values: vec![] }),
                    },
                ],
            }),
            required_capabilities: Default::default(),
        }
    }

    #[test]
    fn sparse_indices_preserve_last_definition_and_exact_storage() {
        // Both rosters invert at the first pair: each probe costs three.
        // Each sparse mode census/selector costs (3*N-2)+5 before the unchanged
        // radix probe/passes. Definitions also pay B/O visits8 and terminal1.
        // Queries still cost 1+2 and 1+3; neither span admits a dense table.
        const BLOCK_SORT_WORK: usize = (3 * 2 - 2) + 5 + 3 + (2 + 4 * (3 * 2 + 512));
        const DEFINITION_SORT_WORK: usize = (3 * 3 - 2) + 5 + 3 + (3 + 4 * (3 * 3 + 512));
        const EXACT_WORK: usize = 2 + BLOCK_SORT_WORK + 9 + 3 + DEFINITION_SORT_WORK + 3 + 4;
        const EXACT_STORAGE: usize = 2 * VerificationFunctionStateV1::BLOCK_ROW_STORAGE
            + 2 * 3 * VerificationFunctionStateV1::DEFINITION_ROW_STORAGE;
        let function = sparse_function();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE - 1);
        assert!(matches!(
            VerificationFunctionStateV1::build(&function, &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(_))
        ));
        assert_eq!(budget.storage(), 0);
        // Radix work is prepaid before scratch denial; terminal1 and queries7 do not run.
        assert_eq!(budget.work(), EXACT_WORK - 8);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(
            state
                .block(BlockId(u32::MAX), &mut budget)
                .unwrap()
                .unwrap()
                .id,
            BlockId(u32::MAX)
        );
        assert!(matches!(
            state.definition(ValueId(17), &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
        ));
        assert_eq!(budget.work(), EXACT_WORK - 1);
        state.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        let state = VerificationFunctionStateV1::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(
            state
                .block(BlockId(u32::MAX), &mut budget)
                .unwrap()
                .unwrap()
                .id,
            BlockId(u32::MAX)
        );
        assert!(matches!(
            state.definition(ValueId(17), &mut budget).unwrap(),
            Some(VerificationDefinitionV1 {
                site: VerificationDefinitionSiteV1::BlockParameter(BlockId(3)),
                ..
            })
        ));
        state.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), EXACT_STORAGE);
        assert_eq!(budget.work(), EXACT_WORK);
    }
}
