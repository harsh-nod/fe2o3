/// Failure from fixed, same-owner private-array effect observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirPrivateArrayQueryErrorV1 {
    /// The caller's existing canonical ledger cannot admit the query.
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    /// The requested source coordinate is not a supported actual occurrence.
    InvalidSource(&'static str),
    /// The retained occurrence has no supported complete address relation.
    Incomplete(&'static str),
    /// A retained record contradicts its actual source or executable operation.
    Mismatch(&'static str),
}

impl fmt::Display for SemanticKirPrivateArrayQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(formatter, "private array query resource: {error}"),
            Self::InvalidSource(detail) => write!(formatter, "private array source: {detail}"),
            Self::Incomplete(detail) => {
                write!(formatter, "private array relation incomplete: {detail}")
            }
            Self::Mismatch(detail) => {
                write!(formatter, "private array relation mismatch: {detail}")
            }
        }
    }
}

impl std::error::Error for SemanticKirPrivateArrayQueryErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::InvalidSource(_) | Self::Incomplete(_) | Self::Mismatch(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateArrayPhysicalLocationV1 {
    block_ordinal: usize,
    block: BlockId,
    operation: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateArrayAccessV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateArrayInstanceV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    lowered_function_ordinal: usize,
    module_function_ordinal: usize,
    slot_start: usize,
    slot_end: usize,
    effect_start: usize,
    effect_end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateArraySlotV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    local: u32,
    semantic_type: SemanticTypeIdV1,
    count_location: PrivateArrayPhysicalLocationV1,
    alloca_location: PrivateArrayPhysicalLocationV1,
    pointer: ValueId,
    count: ValueId,
    length: u64,
    element_type: SemanticTypeIdV1,
    element_facts: PrivateRetainedSlotFactsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateArrayIndexV1 {
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Local {
        local: u32,
        semantic_type: SemanticTypeIdV1,
        original: ValueId,
        physical_type: ScalarType,
        direct_definition: Option<PrivateArrayPhysicalLocationV1>,
    },
    InitializerElement {
        component: u32,
        value: PrivateArrayInitializerValueV1,
    },
}

// A component's written-value relation is distinct from its source occurrence
// and address. Additional operand recipes require independent value checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateArrayInitializerValueV1 {
    LiteralScalar {
        value: ValueId,
        definition: PrivateArrayPhysicalLocationV1,
    },
}

impl PrivateArrayIndexV1 {
    fn component(self) -> u32 {
        match self {
            Self::InitializerElement { component, .. } => component,
            Self::ConstantIndex { .. } | Self::Local { .. } => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateArrayEffectV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    semantic_block: u32,
    semantic_statement: u32,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    local: u32,
    semantic_type: SemanticTypeIdV1,
    source_first_operation: usize,
    source_end_operation: usize,
    original_index: PrivateArrayIndexV1,
    offset_location: Option<PrivateArrayPhysicalLocationV1>,
    gep_location: PrivateArrayPhysicalLocationV1,
    memory_location: PrivateArrayPhysicalLocationV1,
    offset: ValueId,
    gep: ValueId,
    access: PrivateArrayAccessV1,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PrivateArrayCorrespondenceV1 {
    active: bool,
    recorded_instance_operations: usize,
    instances: Vec<PrivateArrayInstanceV1>,
    slots: Vec<PrivateArraySlotV1>,
    effects: Vec<PrivateArrayEffectV1>,
}

#[derive(Clone, Copy)]
struct PrivateArrayExpectedPlaceV1<'s> {
    place: &'s SemanticPlaceV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    initializer_component: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateArrayDirectUnsignedDefinitionV1 {
    value: ValueId,
    scalar: ScalarType,
    constant: u64,
    location: PrivateArrayPhysicalLocationV1,
}

struct PrivateArrayLazyBudgetV1 {
    roots: usize,
    operations: usize,
    active: Option<PrivateArrayRecorderBudgetV1>,
}

impl PrivateArrayLazyBudgetV1 {
    fn new(roots: usize, operations: usize) -> Self {
        Self {
            roots,
            operations,
            active: None,
        }
    }

    fn activate(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.active.is_none() {
            self.active = Some(PrivateArrayRecorderBudgetV1::new(
                self.roots,
                self.operations,
            )?);
        }
        Ok(())
    }

    fn active_limit(&self) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.active
            .as_ref()
            .map(|work| work.work.limit())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }
}

impl PrivateArrayChargeV1 for PrivateArrayLazyBudgetV1 {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.active
            .as_mut()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .charge_private_array_work(amount)
    }
}

struct PrivateArrayRecorderBudgetV1 {
    work: fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1,
    first_denial: Option<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>,
}

impl PrivateArrayRecorderBudgetV1 {
    fn new(roots: usize, operations: usize) -> Result<Self, ProductionSemanticKirErrorV1> {
        let limit = roots
            .checked_mul(operations)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        Ok(Self {
            work: fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit),
            first_denial: None,
        })
    }

    fn work_error(
        error: fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1,
    ) -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual: error.actual(),
            limit: error.limit(),
        }
    }
}

impl PrivateArrayChargeV1 for PrivateArrayRecorderBudgetV1 {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        if let Some(error) = self.first_denial {
            return Err(Self::work_error(error));
        }
        if let Err(error) = self.work.charge_work(amount) {
            self.first_denial = Some(error);
            return Err(Self::work_error(error));
        }
        Ok(())
    }
}

enum PrivateArrayRecorderWorkV1<'a> {
    Shared(&'a mut PrivateArrayLazyBudgetV1),
    #[cfg(test)]
    Owned(PrivateArrayLazyBudgetV1),
}

impl PrivateArrayRecorderWorkV1<'_> {
    fn activate(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
        match self {
            Self::Shared(work) => work.activate(),
            #[cfg(test)]
            Self::Owned(work) => work.activate(),
        }
    }
}

impl PrivateArrayChargeV1 for PrivateArrayRecorderWorkV1<'_> {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        match self {
            Self::Shared(work) => work.charge_private_array_work(amount),
            #[cfg(test)]
            Self::Owned(work) => work.charge_private_array_work(amount),
        }
    }
}

struct PrivateArrayQueryWorkV1<'b, 'w> {
    budget: &'b mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
}

impl PrivateArrayChargeV1 for PrivateArrayQueryWorkV1<'_, '_> {
    type Error = SemanticKirPrivateArrayQueryErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.budget
            .charge_work(amount)
            .map_err(SemanticKirPrivateArrayQueryErrorV1::Resource)
    }
}

struct PrivateArrayCorrelationWorkV1<'a> {
    budget: &'a mut UnsupportedIndexCorrelationBudgetV1,
}

impl PrivateArrayChargeV1 for PrivateArrayCorrelationWorkV1<'_> {
    type Error = ProductionMirPlironTranslationErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        // One atomic fixed-batch admission preserves the shared remaining-work ceiling.
        self.budget.remaining = self
            .budget
            .remaining
            .checked_sub(amount)
            .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        Ok(())
    }
}

struct PrivateArrayBufferV1<T> {
    rows: Vec<T>,
    logical_capacity: usize,
    admitted_end: usize,
    capacity_limit: usize,
}

impl<T> PrivateArrayBufferV1<T> {
    fn new(capacity_limit: usize) -> Self {
        Self {
            rows: Vec::new(),
            logical_capacity: 0,
            admitted_end: 0,
            capacity_limit,
        }
    }
}

impl<T> PrivateArrayBufferV1<T> {
    fn reserve<W: PrivateArrayChargeV1<Error = ProductionSemanticKirErrorV1>>(
        &mut self,
        additional: usize,
        occupied_limit: usize,
        work: &mut W,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let capacity_limit = self.capacity_limit;
        work.charge_private_array_work(1)?;
        let needed = self
            .rows
            .len()
            .checked_add(additional)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        // Existing length/capacity, capacity ceiling, occupied ceiling, and next count.
        work.charge_private_array_work(6)?;
        if self.rows.len() > self.admitted_end
            || self.admitted_end > self.logical_capacity
            || self.rows.len() > self.logical_capacity
            || self.logical_capacity > capacity_limit
            || occupied_limit > capacity_limit
            || needed > occupied_limit
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        work.charge_private_array_work(1)?;
        if needed <= self.logical_capacity {
            // Logical reserve intent plus occupied-frontier bookkeeping.
            work.charge_private_array_work(2)?;
            self.admitted_end = needed;
            return Ok(());
        }

        work.charge_private_array_work(1)?;
        let remaining = capacity_limit
            .checked_sub(self.logical_capacity)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        work.charge_private_array_work(2)?;
        let growth = self.logical_capacity.max(1).min(remaining);
        work.charge_private_array_work(1)?;
        let grown = self
            .logical_capacity
            .checked_add(growth)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        work.charge_private_array_work(1)?;
        let next_capacity = needed.max(grown);
        work.charge_private_array_work(1)?;
        next_capacity
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        work.charge_private_array_work(1)?;
        let reserve = next_capacity
            .checked_sub(self.rows.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        // Logical reserve intent plus capacity/occupied-frontier bookkeeping.
        work.charge_private_array_work(2)?;
        self.rows.try_reserve_exact(reserve).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            }
        })?;
        self.logical_capacity = next_capacity;
        self.admitted_end = needed;
        Ok(())
    }

    fn push<W: PrivateArrayChargeV1<Error = ProductionSemanticKirErrorV1>>(
        &mut self,
        row: T,
        work: &mut W,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        work.charge_private_array_work(2)?;
        if self.rows.len() >= self.logical_capacity || self.rows.len() >= self.admitted_end {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        work.charge_private_array_work(1)?;
        self.rows.push(row);
        Ok(())
    }

    fn truncate(&mut self, length: usize) {
        self.rows.truncate(length);
        self.admitted_end = self.rows.len();
    }

    fn into_rows(self) -> Vec<T> {
        self.rows
    }
}
