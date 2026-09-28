//! Paid actual-owner affine extraction. No memory-report or admission authority.
use super::source_context_bytes_v2::{ByteSourceContextV2, SourceContextErrorV2};
use super::*;
use crate::formal_memory_obligations::affine_engine_v2::{self, Expression, State};
use crate::verification_typed_storage_v2::{allocate_vector_v2, prior_denial_v2, vector_bytes_v2};
use crate::{ControlFlowLimits, VerifiedCanonicalKernelIrModuleV18};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Failure = SourceContextErrorV2;
type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy)]
struct Row {
    key: u32,
    expression: Option<Expression>,
    visiting: bool,
}

struct Builder<'context, 'source, 'budget, 'work> {
    context: &'context mut ByteSourceContextV2<'source, 'work>,
    source: &'source Function,
    budget: &'budget mut Budget<'work>,
    rows: Vec<Row>,
    work: Vec<AffineWork>,
    next: usize,
}
impl Builder<'_, '_, '_, '_> {
    fn position(&mut self, value: ValueId) -> Result<Option<usize>> {
        Ok(verification_find_last_by_v1(
            &self.rows,
            1,
            self.budget,
            |row| row.key.cmp(&value.0),
        )?)
    }
}
impl<'source> State<'source> for Builder<'_, 'source, '_, '_> {
    type Error = Failure;
    fn root(&mut self) -> Result<Option<ValueId>> {
        loop {
            self.budget.charge_work(1)?;
            let Some(row) = self.rows.get(self.next) else {
                return Ok(None);
            };
            let value = ValueId(row.key);
            self.next = self.next.checked_add(1).ok_or(ResourceError::Arithmetic)?;
            if self
                .context
                .operation(self.source, value, self.budget)?
                .is_some_and(|(op, _)| affine_result_is_supported(op))
            {
                return Ok(Some(value));
            }
        }
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>> {
        self.context.unique_origin(self.source, value, self.budget)
    }
    fn operation(&mut self, value: ValueId) -> Result<Option<&'source Operation>> {
        self.context
            .operation(self.source, value, self.budget)
            .map(|row| row.map(|(op, _)| op))
    }
    fn cached(&mut self, value: ValueId) -> Result<Option<Expression>> {
        // An undefined dependency is Unsupported, never a newly invented row.
        Ok(self
            .position(value)?
            .map_or(Some(Err(IndexExpressionError::Unsupported)), |index| {
                self.rows[index].expression
            }))
    }
    fn cache(&mut self, value: ValueId, expression: Expression) -> Result<()> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        self.rows[index].expression = Some(expression);
        Ok(())
    }
    fn enter(&mut self, value: ValueId) -> Result<bool> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        Ok(!std::mem::replace(&mut self.rows[index].visiting, true))
    }
    fn leave(&mut self, value: ValueId) -> Result<()> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        self.rows[index].visiting = false;
        Ok(())
    }
    fn push(&mut self, work: AffineWork) -> Result<()> {
        let mut meter = meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX);
        meter.push(&mut self.work, work)?;
        Ok(())
    }
    fn pop(&mut self) -> Result<Option<AffineWork>> {
        self.budget.charge_work(1)?;
        Ok(self.work.pop())
    }
    fn step(&mut self, work: usize) -> Result<()> {
        Ok(self.budget.charge_work(work)?)
    }
}

#[must_use = "dropping affine extraction without release retains its resource charge"]
pub(in crate::formal_memory_obligations) struct ActualOwnerAffineV18<'owner, 'work> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    root: &'owner crate::Kernel,
    source: &'owner Function,
    context: ByteSourceContextV2<'owner, 'work>,
    rows: Vec<Row>,
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
    first_error: Option<Failure>,
}

impl<'owner, 'work> ActualOwnerAffineV18<'owner, 'work> {
    pub(in crate::formal_memory_obligations) fn build(
        owner: &'owner VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        limits: ControlFlowLimits,
        budget: &mut Budget<'work>,
    ) -> Result<Self> {
        prior_denial_v2(budget)?;
        budget.charge_work(1)?;
        let floor = budget.storage();
        let slot = budget as *mut Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        budget.reserve_storage(frame_bytes_v18())?;
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
            let root = owner
                .module()
                .kernels
                .get(root_index)
                .ok_or(ResourceError::Accounting)?;
            let mut source = None;
            for function in &owner.module().functions {
                let work = function
                    .id
                    .as_str()
                    .len()
                    .checked_add(root.entry.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(ResourceError::Arithmetic)?;
                budget.charge_work(work)?;
                if function.id == root.entry {
                    source = Some(function);
                    break;
                }
            }
            let source = source.ok_or(ResourceError::Accounting)?;
            let mut context = ByteSourceContextV2::build(source, limits, budget)?
                .ok_or(ResourceError::Accounting)?;
            let phase_floor = budget.storage();
            let definitions = context.definition_rows_v2(source, budget)?;
            let mut rows = allocate_vector_v2(definitions.len(), budget)?;
            budget.charge_work(definitions.len())?;
            rows.extend(definitions.iter().map(|row| Row {
                key: row.key,
                expression: None,
                visiting: false,
            }));
            let mut builder = Builder {
                context: &mut context,
                source,
                budget,
                rows,
                work: Vec::new(),
                next: 0,
            };
            affine_engine_v2::run(&mut builder)?;
            let Builder { rows, work, .. } = builder;
            drop(work);
            let retained_rows = vector_bytes_v2(&rows)?;
            let scratch = budget
                .storage()
                .checked_sub(phase_floor)
                .and_then(|n| n.checked_sub(retained_rows))
                .ok_or(ResourceError::Accounting)?;
            budget.release_storage(scratch)?;
            Ok((root, source, context, rows))
        }));
        match result {
            Ok(Ok((root, source, context, rows))) => {
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ResourceError::Accounting)?;
                Ok(Self {
                    owner,
                    root,
                    source,
                    context,
                    rows,
                    slot,
                    ledger,
                    floor,
                    retained,
                    first_error: None,
                })
            }
            Ok(Err(error)) => {
                budget.rollback_storage(floor)?;
                Err(error)
            }
            Err(payload) => {
                budget.rollback_storage(floor)?;
                resume_unwind(payload)
            }
        }
    }

    fn identity(&self, budget: &Budget<'_>) -> Result<()> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(ResourceError::Accounting.into());
        }
        Ok(())
    }
    fn keep<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            self.first_error.get_or_insert_with(|| error.clone());
        }
        result
    }
    fn check(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &Budget<'_>,
    ) -> Result<()> {
        self.identity(budget)?;
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        let result = (|| {
            prior_denial_v2(budget)?;
            if !std::ptr::eq(owner, self.owner)
                || !owner
                    .module()
                    .kernels
                    .get(root_index)
                    .is_some_and(|root| std::ptr::eq(root, self.root))
                || budget.storage()
                    < self
                        .floor
                        .checked_add(self.retained)
                        .ok_or(ResourceError::Arithmetic)?
            {
                return Err(ResourceError::Accounting.into());
            }
            Ok(())
        })();
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn expression(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Expression> {
        self.check(owner, root_index, budget)?;
        let result = (|| {
            let Some(origin) = self.context.unique_origin(self.source, value, budget)? else {
                return Ok(Err(IndexExpressionError::Unsupported));
            };
            let position =
                verification_find_last_by_v1(&self.rows, 1, budget, |row| row.key.cmp(&origin.0))?;
            Ok(position
                .and_then(|index| self.rows[index].expression)
                .unwrap_or(Err(IndexExpressionError::Unsupported)))
        })();
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn release(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.identity(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            return Err(ResourceError::Accounting.into());
        }
        let retained = self.retained;
        drop(self);
        Ok(budget.release_storage(retained)?)
    }
}

fn frame_bytes_v18() -> usize {
    type Output = (
        &'static crate::Kernel,
        &'static Function,
        ByteSourceContextV2<'static, 'static>,
        Vec<Row>,
    );
    size_of::<ActualOwnerAffineV18<'static, 'static>>()
        + size_of::<Builder<'static, 'static, 'static, 'static>>()
        + size_of::<meter::LiveGuardMeter<'static, 'static>>()
        + size_of::<std::thread::Result<Result<Output>>>()
        + size_of::<Result<ActualOwnerAffineV18<'static, 'static>>>()
        + size_of::<(
            &VerifiedCanonicalKernelIrModuleV18,
            usize,
            ControlFlowLimits,
            &mut Budget<'static>,
        )>()
}

#[cfg(test)]
#[path = "affine_source_bytes_v18_tests.rs"]
mod tests;
