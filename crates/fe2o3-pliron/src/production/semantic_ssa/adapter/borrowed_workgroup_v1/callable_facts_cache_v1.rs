//! Cache callable-only classification under one immutable borrow-flow owner.
//! Signatures, candidate custody, argument positions and use sites stay outside.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1;

pub(super) struct Facts<'a> {
    pub context: Option<KernelContextBorrowV1<'a>>,
    pub workgroup_context: Option<WorkgroupContextBorrowV1<'a>>,
    pub global_matrix: Option<Box<GlobalBf16BorrowV1>>,
    pub contract: Option<&'a SemanticExecutionCapabilityContractV1>,
    pub allocation: Option<AllocationBorrow<'a>>,
    pub math_consumer: Option<MathConsumerBorrowV1<'a>>,
}

impl<'a> Facts<'a> {
    fn classify(
        types: Option<&[SemanticTypeDeclV1]>,
        callable: Option<&'a SemanticCallableDeclV1>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        // Preserve the original classification order and construction debit.
        budget.charge(16)?;
        let context =
            callable.and_then(|c| types.and_then(|t| KernelContextBorrowV1::for_callable(t, c)));
        let workgroup_context =
            callable.and_then(|c| types.and_then(|t| WorkgroupContextBorrowV1::for_callable(t, c)));
        let global_matrix =
            callable.and_then(|c| types.and_then(|t| GlobalBf16BorrowV1::for_callable(t, c)));
        let contract = match callable {
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            }) if binding.identity() == contract.source_identity() => Some(contract),
            _ => None,
        };
        if matches!(contract.map(|c| c.operation()), Some(E::LdsAllocate { .. })) {
            budget.charge(16)?;
        }
        let allocation =
            callable.and_then(|c| types.and_then(|t| AllocationBorrow::for_callable(t, c)));
        let math_consumer =
            callable.and_then(|c| types.and_then(|t| MathConsumerBorrowV1::for_callable(t, c)));
        let global_matrix = if let Some(fact) = global_matrix {
            // Do not pad every unrelated row with the large Global contract.
            // Charge its live classification value and retained allocation.
            budget.charge(
                2 * std::mem::size_of::<GlobalBf16BorrowV1>()
                    .div_ceil(std::mem::size_of::<usize>()),
            )?;
            Some(Box::new(fact))
        } else {
            None
        };
        Ok(Self {
            context,
            workgroup_context,
            global_matrix,
            contract,
            allocation,
            math_consumer,
        })
    }
}

pub(super) struct Cache<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: Option<&'a [SemanticTypeDeclV1]>,
    callables: &'a [SemanticCallableDeclV1],
    rows: Vec<(u32, Facts<'a>)>,
    started: bool,
    failed: bool,
}

impl<'a> Cache<'a> {
    pub(super) fn new(
        function: &'a SemanticFunctionDeclV1,
        types: Option<&'a [SemanticTypeDeclV1]>,
        callables: &'a [SemanticCallableDeclV1],
    ) -> Self {
        Self {
            function,
            types,
            callables,
            rows: Vec::new(),
            started: false,
            failed: false,
        }
    }

    pub(super) fn get(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: Option<&[SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        callee: SemanticCallableIdV1,
        budget: &mut Budget,
    ) -> Result<&Facts<'a>, ProductionSemanticSsaErrorV1> {
        // A failed owner cannot be resumed using a fresh allowance.
        if self.failed {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let same_types = match (self.types, types) {
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.callables, callables)
            || !same_types
        {
            self.failed = true;
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        match self.prepare(callee.index(), budget) {
            Ok(index) => Ok(&self.rows[index].1),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    fn prepare(
        &mut self,
        callee: u32,
        budget: &mut Budget,
    ) -> Result<usize, ProductionSemanticSsaErrorV1> {
        if !self.started {
            budget.charge(std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()))?;
            self.started = true;
        }
        budget.charge(1)?;
        let (mut low, mut high) = (0, self.rows.len());
        while low < high {
            budget.charge(1)?;
            let mid = low + (high - low) / 2;
            match self.rows[mid].0.cmp(&callee) {
                std::cmp::Ordering::Equal => return Ok(mid),
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
            }
        }
        let facts = Facts::classify(self.types, self.callables.get(callee as usize), budget)?;
        let cells = std::mem::size_of::<(u32, Facts<'a>)>().div_ceil(std::mem::size_of::<usize>());
        if self.rows.len() == self.rows.capacity() {
            let capacity = self
                .rows
                .capacity()
                .checked_mul(2)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?
                .max(1);
            // Pay both the new allocation and relocation of the still-live old
            // rows before reserve. A row contains fixed values/borrows and an
            // optional already-charged Global box, moved without deep cloning.
            budget.charge(
                capacity
                    .saturating_add(self.rows.len())
                    .saturating_mul(cells),
            )?;
            self.rows
                .try_reserve_exact(capacity - self.rows.len())
                .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            if let Err(error) = budget.charge(
                self.rows
                    .capacity()
                    .saturating_sub(capacity)
                    .saturating_mul(cells),
            ) {
                self.rows = Vec::new();
                return Err(error);
            }
        }
        budget.charge(
            self.rows
                .len()
                .saturating_sub(low)
                .saturating_add(1)
                .saturating_mul(cells),
        )?;
        self.rows.insert(low, (callee, facts));
        Ok(low)
    }
}

#[cfg(test)]
impl<'a> Cache<'a> {
    pub(super) fn test_state(&self) -> (usize, bool, bool) {
        (self.rows.len(), self.started, self.failed)
    }

    pub(super) fn capacity(&self) -> usize {
        self.rows.capacity()
    }
}

#[cfg(test)]
pub(super) fn cold<'a>(
    types: Option<&[SemanticTypeDeclV1]>,
    callable: Option<&'a SemanticCallableDeclV1>,
    budget: &mut Budget,
) -> Result<Facts<'a>, ProductionSemanticSsaErrorV1> {
    Facts::classify(types, callable, budget)
}
