//! Borrowed paid query facade, never a copied CFG or independent owner token.
use super::*;
use crate::VerifiedCanonicalKernelIrModuleV18;
use std::cell::RefCell;

pub(in crate::formal_memory_obligations) fn frame_bytes_v20() -> usize {
    // Each fixed query closure borrows only a subset of these inputs. Keep
    // separate envelopes for that closure, its immediately invoking driver,
    // and keep()'s error-cloning closure; their lifetimes can overlap.
    type QueryCaptures = (
        &'static BorrowedReportSourceV20<'static, 'static, 'static>,
        &'static VerifiedCanonicalKernelIrModuleV18,
        &'static usize,
        &'static ValueId,
        &'static BlockId,
        &'static BlockId,
        &'static BlockId,
    );
    type DriverCaptures = (
        QueryCaptures,
        &'static BorrowedReportSourceV20<'static, 'static, 'static>,
        &'static mut Budget<'static>,
    );
    size_of::<BorrowedReportSourceV20<'static, 'static, 'static>>()
        + size_of::<ContextResult<Option<DefinitionV20<'static>>>>()
        + size_of::<ContextResult<Option<&'static crate::BasicBlock>>>()
        + size_of::<ContextResult<Option<usize>>>()
        + size_of::<ContextResult<bool>>()
        + size_of::<std::cell::RefMut<'static, &'static mut ByteSourceContextV2<'static, 'static>>>(
        )
        + size_of::<std::cell::Ref<'static, Option<SourceContextErrorV2>>>()
        + size_of::<std::cell::RefMut<'static, Option<SourceContextErrorV2>>>()
        + size_of::<QueryCaptures>()
        + size_of::<DriverCaptures>()
        + size_of::<&SourceContextErrorV2>()
        + size_of::<
            std::result::Result<
                Option<(usize, crate::VerificationDefinitionV1<'static>)>,
                crate::CanonicalKernelIrVerificationResourceErrorV1,
            >,
        >()
        + size_of::<(
            &BorrowedReportSourceV20<'_, '_, '_>,
            &mut Budget<'_>,
            ValueId,
            BlockId,
            BlockId,
            BlockId,
        )>()
}

pub(in crate::formal_memory_obligations) type DefinitionV20<'source> =
    (usize, &'source Type, Option<&'source Operation>);

pub(in crate::formal_memory_obligations) trait RetainedSourceQueriesV20<'source> {
    fn check(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> ContextResult<()>;
    fn block_count(&self, budget: &mut Budget<'_>) -> ContextResult<usize>;
    fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<&'source crate::BasicBlock>>;
    fn block_ordinal(
        &self,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<usize>>;
    fn reachable(&self, block: BlockId, budget: &mut Budget<'_>) -> ContextResult<bool>;
    fn definition_count(&self, budget: &mut Budget<'_>) -> ContextResult<usize>;
    fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<DefinitionV20<'source>>>;
    fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<bool>;
}

pub(in crate::formal_memory_obligations) struct BorrowedReportSourceV20<'borrow, 'source, 'work> {
    context: RefCell<&'borrow mut ByteSourceContextV2<'source, 'work>>,
    owner: &'source VerifiedCanonicalKernelIrModuleV18,
    source: &'source Function,
    root: usize,
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    first: RefCell<Option<SourceContextErrorV2>>,
}

impl<'borrow, 'source, 'work> BorrowedReportSourceV20<'borrow, 'source, 'work> {
    pub(in crate::formal_memory_obligations) fn new(
        context: &'borrow mut ByteSourceContextV2<'source, 'work>,
        owner: &'source VerifiedCanonicalKernelIrModuleV18,
        root: usize,
        budget: &Budget<'_>,
    ) -> Self {
        Self {
            source: context.source,
            context: RefCell::new(context),
            owner,
            root,
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            first: RefCell::new(None),
        }
    }

    fn same(&self, budget: &Budget<'_>) -> bool {
        self.slot == budget as *const Budget<'_> as usize
            && self.ledger == budget.work_ledger_identity_v1()
    }

    fn keep<T>(&self, result: ContextResult<T>) -> ContextResult<T> {
        if let Err(error) = &result {
            self.first.borrow_mut().get_or_insert_with(|| error.clone());
        }
        result
    }

    fn query<T>(
        &self,
        budget: &mut Budget<'_>,
        run: impl FnOnce(&mut ByteSourceContextV2<'source, 'work>, &mut Budget<'_>) -> ContextResult<T>,
    ) -> ContextResult<T> {
        // Foreign account refusals cannot poison or refund the original scope.
        if !self.same(budget) {
            return Err(ResourceError::Accounting.into());
        }
        if let Some(error) = self.first.borrow().as_ref() {
            return Err(error.clone());
        }
        let result = (|| {
            prior_denial_v2(budget)?;
            if budget.storage() < self.floor {
                return Err(ResourceError::Accounting.into());
            }
            let mut context = self
                .context
                .try_borrow_mut()
                .map_err(|_| ResourceError::Accounting)?;
            context.check(self.source, budget)?;
            budget.charge_work(1)?;
            run(&mut context, budget)
        })();
        self.keep(result)
    }
}

impl<'source> RetainedSourceQueriesV20<'source> for BorrowedReportSourceV20<'_, 'source, '_> {
    fn check(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> ContextResult<()> {
        self.query(budget, |_, _| {
            if !std::ptr::eq(owner, self.owner) || root != self.root {
                return Err(ResourceError::Accounting.into());
            }
            Ok(())
        })
    }

    fn block_count(&self, budget: &mut Budget<'_>) -> ContextResult<usize> {
        self.query(budget, |context, budget| {
            let (flow, _) = context.guarded_inputs(self.source, budget)?;
            budget.charge_work(1)?;
            Ok(flow.block_count())
        })
    }

    fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<&'source crate::BasicBlock>> {
        self.query(budget, |context, budget| {
            let (flow, _) = context.guarded_inputs(self.source, budget)?;
            budget.charge_work(1)?;
            let Some(block) = flow.block_id(ordinal) else {
                return Ok(None);
            };
            context.block(self.source, block, budget)
        })
    }

    fn block_ordinal(
        &self,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<usize>> {
        self.query(budget, |context, budget| {
            let (flow, _) = context.guarded_inputs(self.source, budget)?;
            budget.charge_work(lookup_work_v2(flow.block_count())?)?;
            Ok(flow.block_position(block))
        })
    }

    fn reachable(&self, block: BlockId, budget: &mut Budget<'_>) -> ContextResult<bool> {
        self.query(budget, |context, budget| {
            let (flow, _) = context.guarded_inputs(self.source, budget)?;
            budget.charge_work(lookup_work_v2(flow.block_count())?)?;
            Ok(flow.is_reachable(block))
        })
    }

    fn definition_count(&self, budget: &mut Budget<'_>) -> ContextResult<usize> {
        self.query(budget, |context, budget| {
            Ok(context.index.definition_count_v20(self.source, budget)?)
        })
    }

    fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<DefinitionV20<'source>>> {
        self.query(budget, |context, budget| {
            let Some((ordinal, definition)) =
                context
                    .index
                    .definition_ordinal_v20(self.source, value, budget)?
            else {
                return Ok(None);
            };
            budget.charge_work(1)?;
            let operation =
                context
                    .operation(self.source, value, budget)?
                    .and_then(|(operation, _)| match operation.results.as_slice() {
                        [result] if result.id == value => Some(operation),
                        _ => None,
                    });
            Ok(Some((ordinal, definition.ty, operation)))
        })
    }

    fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<bool> {
        self.query(budget, |context, budget| {
            let body = self.source.body.as_ref().ok_or(ResourceError::Accounting)?;
            let entry = body.blocks.first().ok_or(ResourceError::Accounting)?.id;
            let (flow, _) = context.guarded_inputs(self.source, budget)?;
            // source reachability, target predecessor lookup, and both dominance
            // lookups; at most two predecessor elements are inspected, never a
            // complete degree scan. Boolean edge truth stays the caller's duty.
            let work = lookup_work_v2(flow.block_count())?
                .checked_mul(4)
                .and_then(|n| n.checked_add(5))
                .ok_or(ResourceError::Arithmetic)?;
            budget.charge_work(work)?;
            if target == entry || !flow.is_reachable(source) {
                return Ok(false);
            }
            let Some(mut predecessors) = flow.predecessor_blocks(target) else {
                return Ok(false);
            };
            Ok(predecessors.next() == Some(source)
                && predecessors.next().is_none()
                && flow.dominates(target, access))
        })
    }
}
