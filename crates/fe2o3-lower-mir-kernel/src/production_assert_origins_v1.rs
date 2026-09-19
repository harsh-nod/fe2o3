// Emission custody only: these locators do not prove source/KIR equivalence.

type AssertOriginBudgetV1<'a> = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'a>;
type AssertOriginResourceV1 = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1;
type AssertOriginResultV1<T> = Result<T, SemanticKirAssertOriginErrorV1>;

/// Inert source-site locator for the closed AssertCondition origin role.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKirAssertSiteV1 {
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
}

impl SemanticKirAssertSiteV1 {
    /// Creates a locator, not an admitted binding.
    pub const fn new(
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_block: SemanticBlockIdV1,
    ) -> Self {
        Self {
            correspondence_owner,
            semantic_function,
            semantic_block,
        }
    }
}

/// Origin admission/query failure, distinct from an unknown graph value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirAssertOriginErrorV1 {
    /// The shared phase ledger rejected work or logical payload.
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    /// A promised source site has no sealed binding.
    MissingBinding {
        /// Exact source-site locator.
        site: SemanticKirAssertSiteV1,
    },
    /// The emission trace or its graph/source association is inconsistent.
    InvalidBinding {
        /// Exact source-site locator, when available.
        site: Option<SemanticKirAssertSiteV1>,
        /// Stable structural rejection reason.
        detail: &'static str,
    },
}

impl fmt::Display for SemanticKirAssertOriginErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::MissingBinding { site } => write!(formatter, "missing assertion origin {site:?}"),
            Self::InvalidBinding { site, detail } => {
                write!(formatter, "invalid assertion origin {site:?}: {detail}")
            }
        }
    }
}
impl std::error::Error for SemanticKirAssertOriginErrorV1 {}
impl From<AssertOriginResourceV1> for SemanticKirAssertOriginErrorV1 {
    fn from(error: AssertOriginResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<SemanticKirAssertOriginErrorV1> for ProductionSemanticKirErrorV1 {
    fn from(error: SemanticKirAssertOriginErrorV1) -> Self {
        Self::AssertOrigin(error)
    }
}

impl From<SemanticKirAssertOriginErrorV1> for ProductionPreRankedKirErrorV1 {
    fn from(error: SemanticKirAssertOriginErrorV1) -> Self {
        Self::Lowering(ProductionSemanticKirErrorV1::AssertOrigin(error))
    }
}

/// Actual executable occurrence, or an explicitly absent condition use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirAssertConditionOutcomeV1 {
    /// The emitted Boolean condition and both ordered successor occurrences.
    Emitted {
        /// Exact Boolean terminator operand occurrence.
        condition_use: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
        /// Exact graph definition consumed by that occurrence.
        definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        /// Successful successor occurrence, respecting expected=false.
        success_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
        /// Synthetic failure successor occurrence.
        failure_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    },
    /// Existing source-rule elision, not a fabricated graph Boolean or proof.
    ElidedByExistingRule {
        /// Actual emitted unconditional success occurrence.
        success_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    },
}

/// Sealed physical assertion attachment. Source/root aliases share this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirAssertConditionBindingV1 {
    expected: bool,
    semantic_success: SemanticBlockIdV1,
    outcome: SemanticKirAssertConditionOutcomeV1,
}
impl SemanticKirAssertConditionBindingV1 {
    /// Returns the source assertion polarity retained at emission.
    pub const fn expected(self) -> bool {
        self.expected
    }
    /// Returns the source success block, whose edge arguments were checked exactly.
    pub const fn semantic_success(self) -> SemanticBlockIdV1 {
        self.semantic_success
    }
    /// Returns the actual executable occurrence or explicit existing-rule elision.
    pub const fn outcome(self) -> SemanticKirAssertConditionOutcomeV1 {
        self.outcome
    }
    fn block(self) -> fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
        match self.outcome {
            SemanticKirAssertConditionOutcomeV1::Emitted { success_edge, .. }
            | SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
                success_edge.source
            }
        }
    }
}

/// Logical retained-origin payload transferred with its owning executable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirAssertOriginStorageV1 {
    payload_storage: usize,
}
impl SemanticKirAssertOriginStorageV1 {
    /// Reserve this floor in addition to the connected executable receipt.
    pub const fn payload_storage(self) -> usize {
        self.payload_storage
    }
}

#[derive(Clone, Copy, Debug)]
struct AssertOriginAliasV1 {
    site: SemanticKirAssertSiteV1,
    binding: usize,
}
#[derive(Debug)]
struct SealedAssertOriginsV1 {
    aliases: Vec<AssertOriginAliasV1>,
    functions: Vec<AssertSourceFunctionV1>,
    bindings: Vec<SemanticKirAssertConditionBindingV1>,
    storage: SemanticKirAssertOriginStorageV1,
}

/// Borrowed assertion attachments tied to the owner's exact immutable graph.
/// No independent constructor, graph clone, source evaluator or analysis exists here.
#[derive(Clone, Copy, Debug)]
pub struct SemanticKirAssertOriginsV1<'a> {
    executable: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    semantic_ssa: &'a ProductionSemanticSsaOwnerV1,
    origins: &'a SealedAssertOriginsV1,
}
impl<'a> SemanticKirAssertOriginsV1<'a> {
    /// Borrows the exact graph against which these origins were sealed.
    pub const fn executable(self) -> &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.executable
    }
    /// Number of admitted source/root associations, including shared-helper aliases.
    pub fn source_site_count(self) -> usize {
        self.origins.aliases.len()
    }
    /// Number of distinct physical assertion occurrences in the executable.
    pub fn binding_count(self) -> usize {
        self.origins.bindings.len()
    }
    /// Checks applicability against the sealed function roster and exact retained SSA plan.
    /// Invalid owner/function/block coordinates are errors; false means an in-range
    /// block absent from the actual materialization roster, never an elided condition.
    /// Work is four fixed checks plus one per function-roster search comparison.
    pub fn is_materialized_block(
        self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<bool> {
        budget.charge_work(4)?;
        let site =
            SemanticKirAssertSiteV1::new(correspondence_owner, semantic_function, semantic_block);
        let bad = || assert_origin_invalid_v1(Some(site), "invalid materialized block locator");
        let index = assert_origin_find_v1(&self.origins.functions, budget, |row, _| {
            Ok((row.owner, row.function).cmp(&(correspondence_owner, semantic_function)))
        })?
        .ok_or_else(bad)?;
        let source = self
            .semantic_ssa
            .source_semantic()
            .functions()
            .get(semantic_function.index() as usize)
            .ok_or_else(bad)?;
        let plan = self
            .semantic_ssa
            .plan_for_function(semantic_function)
            .ok_or_else(bad)?;
        if semantic_block.index() as usize >= source.blocks().len()
            || plan.function_identity() != source.identity()
            || plan.plan().reverse_postorder().len()
                != self.origins.functions[index].reachable_blocks
        {
            return Err(bad());
        }
        Ok(plan
            .plan()
            .is_reachable(fe2o3_mir_model::SsaBlockIdV1::new(semantic_block.index())))
    }

    /// Looks up one promised source site without allocating or inspecting source text.
    /// Each binary-search comparison and final row read is charged before access.
    pub fn assert_condition(
        self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<SemanticKirAssertConditionBindingV1> {
        let site =
            SemanticKirAssertSiteV1::new(correspondence_owner, semantic_function, semantic_block);
        let index = assert_origin_find_v1(&self.origins.aliases, budget, |row, _| {
            Ok(row.site.cmp(&site))
        })?
        .ok_or(SemanticKirAssertOriginErrorV1::MissingBinding { site })?;
        budget.charge_work(1)?;
        let row = self.origins.aliases[index];
        self.origins
            .bindings
            .get(row.binding)
            .copied()
            .ok_or_else(|| {
                assert_origin_invalid_v1(Some(site), "sealed binding ordinal is missing")
            })
    }
}

#[derive(Clone, Copy, Debug)]
enum PendingAssertOutcomeV1 {
    Emitted {
        condition: ValueId,
        failure: BlockId,
    },
    ElidedByExistingRule,
}
#[derive(Debug)]
struct PendingAssertOriginV1 {
    site: SemanticKirAssertSiteV1,
    emitted_function: String,
    block: BlockId,
    first_operation: u32,
    operation_count: u32,
    expected: bool,
    semantic_success: SemanticBlockIdV1,
    physical_success: BlockId,
    argument_start: usize,
    argument_count: usize,
    outcome: PendingAssertOutcomeV1,
}
struct AssertOriginEmissionV1<'a, 'work> {
    budget: &'a mut AssertOriginBudgetV1<'work>,
    records: Vec<PendingAssertOriginV1>,
    arguments: Vec<ValueId>,
}
impl<'a, 'work> AssertOriginEmissionV1<'a, 'work> {
    fn new(budget: &'a mut AssertOriginBudgetV1<'work>) -> Self {
        Self {
            budget,
            records: Vec::new(),
            arguments: Vec::new(),
        }
    }

    fn record(
        &mut self,
        span: SemanticKirTerminatorOperationSpanV1,
        emitted_function: &FunctionId,
        source: &SemanticTerminatorKindV1,
        emitted: &Terminator,
        elided_by_existing_rule: bool,
    ) -> AssertOriginResultV1<()> {
        record_assert_origin_at_v1(
            &mut self.records,
            &mut self.arguments,
            span,
            emitted_function,
            source,
            emitted,
            elided_by_existing_rule,
            SemanticEmissionPlacementV1::default(),
            self.budget,
        )
    }
}

fn assert_origin_copy_name_v1(
    value: &str,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<String> {
    budget.charge_work(value.len())?;
    budget.reserve_storage(value.len())?;
    let mut copy = String::new();
    if copy.try_reserve_exact(value.len()).is_err() {
        drop(copy);
        budget.release_storage(value.len())?;
        return Err(AssertOriginResourceV1::Allocation.into());
    }
    if let Err(error) = budget.reserve_storage(copy.capacity() - value.len()) {
        drop(copy);
        budget.release_storage(value.len())?;
        return Err(error.into());
    }
    copy.push_str(value);
    Ok(copy)
}

fn assert_origin_invalid_v1(
    site: Option<SemanticKirAssertSiteV1>,
    detail: &'static str,
) -> SemanticKirAssertOriginErrorV1 {
    SemanticKirAssertOriginErrorV1::InvalidBinding { site, detail }
}
fn assert_origin_bytes_v1<T>(capacity: usize) -> AssertOriginResultV1<usize> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(AssertOriginResourceV1::Arithmetic.into())
}

// Grow only these origin vectors. The old/new allocations coexist until the move
// completes; each relocation and logical byte reservation precedes its operation.
fn assert_origin_reserve_v1<T>(
    values: &mut Vec<T>,
    required: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<()> {
    if required <= values.capacity() {
        return Ok(());
    }
    let old_capacity = values.capacity();
    let capacity = required
        .max(
            old_capacity
                .checked_mul(2)
                .ok_or(AssertOriginResourceV1::Arithmetic)?,
        )
        .max(4);
    let bytes = assert_origin_bytes_v1::<T>(capacity)?;
    budget.charge_work(
        values
            .len()
            .checked_add(1)
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    budget.reserve_storage(bytes)?;
    let mut next = Vec::new();
    if next.try_reserve_exact(capacity).is_err() {
        drop(next);
        budget.release_storage(bytes)?;
        return Err(AssertOriginResourceV1::Allocation.into());
    }
    let actual_bytes = assert_origin_bytes_v1::<T>(next.capacity())?;
    if let Err(error) = budget.reserve_storage(actual_bytes - bytes) {
        drop(next);
        budget.release_storage(bytes)?;
        return Err(error.into());
    }
    next.append(values);
    let old = std::mem::replace(values, next);
    drop(old);
    budget.release_storage(assert_origin_bytes_v1::<T>(old_capacity)?)?;
    Ok(())
}
fn assert_origin_push_v1<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<()> {
    let required = values
        .len()
        .checked_add(1)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    assert_origin_reserve_v1(values, required, budget)?;
    budget.charge_work(1)?;
    values.push(value);
    Ok(())
}
fn assert_origin_drop_v1<T>(
    values: Vec<T>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<()> {
    let bytes = assert_origin_bytes_v1::<T>(values.capacity())?;
    drop(values);
    budget.release_storage(bytes)?;
    Ok(())
}
fn assert_origin_find_v1<T>(
    values: &[T],
    budget: &mut AssertOriginBudgetV1<'_>,
    mut compare: impl FnMut(
        &T,
        &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<std::cmp::Ordering>,
) -> AssertOriginResultV1<Option<usize>> {
    let (mut first, mut end) = (0, values.len());
    while first < end {
        budget.charge_work(1)?;
        let middle = first + (end - first) / 2;
        match compare(&values[middle], budget)? {
            std::cmp::Ordering::Less => first = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

// As in the graph inventory, a fallible heapsort charges actual comparisons and
// swaps instead of assuming an implementation-specific std sorting bound.
fn assert_origin_sort_v1<T>(
    values: &mut [T],
    budget: &mut AssertOriginBudgetV1<'_>,
    mut compare: impl FnMut(
        &T,
        &T,
        &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<std::cmp::Ordering>,
) -> AssertOriginResultV1<()> {
    fn sift<T>(
        values: &mut [T],
        mut root: usize,
        end: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
        compare: &mut impl FnMut(
            &T,
            &T,
            &mut AssertOriginBudgetV1<'_>,
        ) -> AssertOriginResultV1<std::cmp::Ordering>,
    ) -> AssertOriginResultV1<()> {
        loop {
            budget.charge_work(1)?;
            let child = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            if child >= end {
                return Ok(());
            }
            let next = if child + 1 < end {
                budget.charge_work(1)?;
                if compare(&values[child], &values[child + 1], budget)? == std::cmp::Ordering::Less
                {
                    child + 1
                } else {
                    child
                }
            } else {
                child
            };
            budget.charge_work(1)?;
            if compare(&values[root], &values[next], budget)? != std::cmp::Ordering::Less {
                return Ok(());
            }
            budget.charge_work(1)?;
            values.swap(root, next);
            root = next;
        }
    }
    if values.len() < 2 {
        return Ok(());
    }
    let length = values.len();
    for root in (0..length / 2).rev() {
        sift(values, root, length, budget, &mut compare)?;
    }
    for end in (1..length).rev() {
        budget.charge_work(1)?;
        values.swap(0, end);
        sift(values, 0, end, budget, &mut compare)?;
    }
    Ok(())
}

type AssertFunctionCoordinateV1 = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1;
type AssertBlockCoordinateV1 = fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1;
type AssertDefinitionCoordinateV1 = fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1;

#[derive(Clone, Copy)]
struct AssertBlockIndexV1 {
    function: AssertFunctionCoordinateV1,
    id: BlockId,
    ordinal: u32,
}
#[derive(Clone, Copy)]
struct AssertDefinitionIndexV1 {
    function: AssertFunctionCoordinateV1,
    id: ValueId,
    coordinate: AssertDefinitionCoordinateV1,
    boolean: bool,
}
struct AssertGraphIndexV1<'a> {
    functions: Vec<(&'a str, AssertFunctionCoordinateV1)>,
    blocks: Vec<AssertBlockIndexV1>,
    definitions: Vec<AssertDefinitionIndexV1>,
}
impl<'a> AssertGraphIndexV1<'a> {
    fn build(
        module: &'a Module,
        include_definitions: bool,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<Self> {
        Self::build_functions(&module.functions, include_definitions, budget)
    }

    fn build_functions(
        functions: &'a [Function],
        include_definitions: bool,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<Self> {
        let mut result = Self {
            functions: Vec::new(),
            blocks: Vec::new(),
            definitions: Vec::new(),
        };
        for (function_ordinal, function) in functions.iter().enumerate() {
            budget.charge_work(1)?;
            let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(function_ordinal).map_err(|_| AssertOriginResourceV1::Arithmetic)?,
            );
            assert_origin_push_v1(
                &mut result.functions,
                (function.id.as_str(), coordinate),
                budget,
            )?;
            let Some(body) = &function.body else { continue };
            if body.parameters.len() != function.signature.parameters.len() {
                return Err(assert_origin_invalid_v1(
                    None,
                    "function parameter arity differs",
                ));
            }
            for (argument, (value, ty)) in body
                .parameters
                .iter()
                .zip(&function.signature.parameters)
                .enumerate()
            {
                if !include_definitions {
                    break;
                }
                budget.charge_work(1)?;
                assert_origin_push_v1(
                    &mut result.definitions,
                    AssertDefinitionIndexV1 {
                        function: coordinate,
                        id: *value,
                        boolean: *ty == Type::BOOL,
                        coordinate: AssertDefinitionCoordinateV1::FunctionArgument {
                            function: coordinate,
                            argument: u32::try_from(argument)
                                .map_err(|_| AssertOriginResourceV1::Arithmetic)?,
                        },
                    },
                    budget,
                )?;
            }
            for (block_ordinal, block) in body.blocks.iter().enumerate() {
                budget.charge_work(1)?;
                let block_coordinate = AssertBlockCoordinateV1 {
                    function: coordinate,
                    block: u32::try_from(block_ordinal)
                        .map_err(|_| AssertOriginResourceV1::Arithmetic)?,
                };
                assert_origin_push_v1(
                    &mut result.blocks,
                    AssertBlockIndexV1 {
                        function: coordinate,
                        id: block.id,
                        ordinal: block_coordinate.block,
                    },
                    budget,
                )?;
                if !include_definitions {
                    continue;
                }
                for (argument, value) in block.parameters.iter().enumerate() {
                    budget.charge_work(1)?;
                    assert_origin_push_v1(
                        &mut result.definitions,
                        AssertDefinitionIndexV1 {
                            function: coordinate,
                            id: value.id,
                            boolean: value.ty == Type::BOOL,
                            coordinate: AssertDefinitionCoordinateV1::BlockArgument {
                                block: block_coordinate,
                                argument: u32::try_from(argument)
                                    .map_err(|_| AssertOriginResourceV1::Arithmetic)?,
                            },
                        },
                        budget,
                    )?;
                }
                for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                    budget.charge_work(1)?;
                    let operation_coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                        block: block_coordinate,
                        operation: u32::try_from(operation_ordinal)
                            .map_err(|_| AssertOriginResourceV1::Arithmetic)?,
                    };
                    for (result_ordinal, value) in operation.results.iter().enumerate() {
                        budget.charge_work(1)?;
                        assert_origin_push_v1(
                            &mut result.definitions,
                            AssertDefinitionIndexV1 {
                                function: coordinate,
                                id: value.id,
                                boolean: value.ty == Type::BOOL,
                                coordinate: AssertDefinitionCoordinateV1::Result {
                                    operation: operation_coordinate,
                                    result: u32::try_from(result_ordinal)
                                        .map_err(|_| AssertOriginResourceV1::Arithmetic)?,
                                },
                            },
                            budget,
                        )?;
                    }
                }
            }
        }
        assert_origin_sort_v1(&mut result.functions, budget, |a, b, budget| {
            assert_origin_string_work_v1(a.0, b.0, budget)?;
            Ok(a.0.cmp(b.0))
        })?;
        assert_origin_sort_v1(&mut result.blocks, budget, |a, b, _| {
            Ok((a.function, a.id).cmp(&(b.function, b.id)))
        })?;
        assert_origin_sort_v1(&mut result.definitions, budget, |a, b, _| {
            Ok((a.function, a.id).cmp(&(b.function, b.id)))
        })?;
        for pair in result.functions.windows(2) {
            budget.charge_work(1)?;
            assert_origin_string_work_v1(pair[0].0, pair[1].0, budget)?;
            if pair[0].0 == pair[1].0 {
                return Err(assert_origin_invalid_v1(
                    None,
                    "duplicate executable function",
                ));
            }
        }
        for pair in result.blocks.windows(2) {
            budget.charge_work(1)?;
            if (pair[0].function, pair[0].id) == (pair[1].function, pair[1].id) {
                return Err(assert_origin_invalid_v1(None, "duplicate executable block"));
            }
        }
        for pair in result.definitions.windows(2) {
            budget.charge_work(1)?;
            if (pair[0].function, pair[0].id) == (pair[1].function, pair[1].id) {
                return Err(assert_origin_invalid_v1(
                    None,
                    "duplicate executable definition",
                ));
            }
        }
        Ok(result)
    }
    fn function(
        &self,
        name: &str,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<AssertFunctionCoordinateV1> {
        let index = assert_origin_find_v1(&self.functions, budget, |row, budget| {
            assert_origin_string_work_v1(row.0, name, budget)?;
            Ok(row.0.cmp(name))
        })?
        .ok_or_else(|| assert_origin_invalid_v1(None, "missing executable function"))?;
        budget.charge_work(1)?;
        Ok(self.functions[index].1)
    }
    fn block(
        &self,
        function: AssertFunctionCoordinateV1,
        id: BlockId,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<AssertBlockCoordinateV1> {
        let index = assert_origin_find_v1(&self.blocks, budget, |row, _| {
            Ok((row.function, row.id).cmp(&(function, id)))
        })?
        .ok_or_else(|| assert_origin_invalid_v1(None, "missing executable block"))?;
        budget.charge_work(1)?;
        Ok(AssertBlockCoordinateV1 {
            function,
            block: self.blocks[index].ordinal,
        })
    }
    fn definition(
        &self,
        function: AssertFunctionCoordinateV1,
        id: ValueId,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> AssertOriginResultV1<AssertDefinitionIndexV1> {
        let index = assert_origin_find_v1(&self.definitions, budget, |row, _| {
            Ok((row.function, row.id).cmp(&(function, id)))
        })?
        .ok_or_else(|| assert_origin_invalid_v1(None, "missing executable definition"))?;
        budget.charge_work(1)?;
        Ok(self.definitions[index])
    }
    fn release(self, budget: &mut AssertOriginBudgetV1<'_>) -> AssertOriginResultV1<()> {
        assert_origin_drop_v1(self.functions, budget)?;
        assert_origin_drop_v1(self.blocks, budget)?;
        assert_origin_drop_v1(self.definitions, budget)
    }
}
fn assert_origin_string_work_v1(
    left: &str,
    right: &str,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<()> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    Ok(())
}
fn assert_origin_block_v1(
    module: &Module,
    coordinate: AssertBlockCoordinateV1,
) -> AssertOriginResultV1<&BasicBlock> {
    assert_origin_function_block_v1(&module.functions, coordinate)
}

fn assert_origin_function_block_v1(
    functions: &[Function],
    coordinate: AssertBlockCoordinateV1,
) -> AssertOriginResultV1<&BasicBlock> {
    functions
        .get(coordinate.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(coordinate.block as usize))
        .ok_or_else(|| assert_origin_invalid_v1(None, "invalid canonical block coordinate"))
}

#[derive(Clone, Copy, Debug)]
struct AssertSourceFunctionV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    canonical: AssertFunctionCoordinateV1,
    reachable_blocks: usize,
}
#[derive(Clone, Copy)]
struct ResolvedAssertOriginV1 {
    site: SemanticKirAssertSiteV1,
    binding: SemanticKirAssertConditionBindingV1,
}

impl AssertOriginEmissionV1<'_, '_> {
    // The fixed production caller has already validated the complete root/helper
    // correspondence roster. This extends that custody with exact SSA block and
    // assertion emission coverage; it is not a standalone roster admission API.
    fn seal(
        mut self,
        semantic_ssa: &ProductionSemanticSsaOwnerV1,
        correspondence: &SemanticKirCorrespondenceV1,
        executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    ) -> AssertOriginResultV1<SealedAssertOriginsV1> {
        let budget = &mut *self.budget;
        let semantic = semantic_ssa.source_semantic();
        budget.charge_work(1)?;
        if correspondence.semantic_sha256 != *semantic.semantic_sha256().as_bytes() {
            return Err(assert_origin_invalid_v1(
                None,
                "source identity differs from correspondence",
            ));
        }
        let graph =
            AssertGraphIndexV1::build(executable.module(), !self.records.is_empty(), budget)?;
        let mut roots = Vec::new();
        for root in semantic.roots() {
            budget.charge_work(1)?;
            assert_origin_push_v1(&mut roots, *root, budget)?;
        }
        assert_origin_sort_v1(&mut roots, budget, |a, b, _| Ok(a.cmp(b)))?;
        let mut functions = Vec::new();
        for function in &correspondence.lowered_functions {
            budget.charge_work(1)?;
            if assert_origin_find_v1(&roots, budget, |root, _| {
                Ok(root.cmp(&function.correspondence_owner))
            })?
            .is_none()
                || semantic
                    .functions()
                    .get(function.semantic_function.index() as usize)
                    .is_none()
            {
                return Err(assert_origin_invalid_v1(
                    None,
                    "invalid correspondence function owner",
                ));
            }
            let plan = semantic_ssa
                .plan_for_function(function.semantic_function)
                .ok_or_else(|| assert_origin_invalid_v1(None, "missing source SSA function"))?;
            if plan.function_identity()
                != semantic.functions()[function.semantic_function.index() as usize].identity()
            {
                return Err(assert_origin_invalid_v1(
                    None,
                    "source SSA function identity differs",
                ));
            }
            let canonical = graph.function(function.kernel_ir_function.as_str(), budget)?;
            assert_origin_push_v1(
                &mut functions,
                AssertSourceFunctionV1 {
                    owner: function.correspondence_owner,
                    function: function.semantic_function,
                    canonical,
                    reachable_blocks: plan.plan().reverse_postorder().len(),
                },
                budget,
            )?;
        }
        assert_origin_sort_v1(&mut functions, budget, |a, b, _| {
            Ok((a.owner, a.function).cmp(&(b.owner, b.function)))
        })?;
        for pair in functions.windows(2) {
            budget.charge_work(1)?;
            if (pair[0].owner, pair[0].function) == (pair[1].owner, pair[1].function) {
                return Err(assert_origin_invalid_v1(
                    None,
                    "duplicate correspondence function",
                ));
            }
        }
        assert_origin_sort_v1(&mut self.records, budget, |a, b, _| Ok(a.site.cmp(&b.site)))?;
        for pair in self.records.windows(2) {
            budget.charge_work(1)?;
            if pair[0].site == pair[1].site {
                return Err(assert_origin_invalid_v1(
                    Some(pair[0].site),
                    "duplicate assertion origin",
                ));
            }
        }

        let mut spans = Vec::new();
        for span in &correspondence.terminator_operation_spans {
            budget.charge_work(1)?;
            assert_origin_push_v1(&mut spans, span, budget)?;
        }
        let span_key = |span: &SemanticKirTerminatorOperationSpanV1| {
            SemanticKirAssertSiteV1::new(
                span.correspondence_owner,
                span.semantic_function,
                span.semantic_block,
            )
        };
        assert_origin_sort_v1(&mut spans, budget, |a, b, _| {
            Ok(span_key(a).cmp(&span_key(b)))
        })?;
        for pair in spans.windows(2) {
            budget.charge_work(1)?;
            if span_key(pair[0]) == span_key(pair[1]) {
                return Err(assert_origin_invalid_v1(
                    Some(span_key(pair[0])),
                    "duplicate materialized source block",
                ));
            }
        }
        let mut seen_blocks = Vec::new();
        for _ in &functions {
            budget.charge_work(1)?;
            assert_origin_push_v1(&mut seen_blocks, 0_usize, budget)?;
        }
        let mut resolved = Vec::new();
        for span in &spans {
            budget.charge_work(1)?;
            let site = SemanticKirAssertSiteV1::new(
                span.correspondence_owner,
                span.semantic_function,
                span.semantic_block,
            );
            let source_function = semantic
                .functions()
                .get(span.semantic_function.index() as usize)
                .ok_or_else(|| assert_origin_invalid_v1(Some(site), "missing source function"))?;
            let source_block = source_function
                .blocks()
                .get(span.semantic_block.index() as usize)
                .ok_or_else(|| assert_origin_invalid_v1(Some(site), "missing source block"))?;
            let function_index = assert_origin_find_v1(&functions, budget, |row, _| {
                Ok((row.owner, row.function)
                    .cmp(&(site.correspondence_owner, site.semantic_function)))
            })?
            .ok_or_else(|| assert_origin_invalid_v1(Some(site), "missing function association"))?;
            budget.charge_work(1)?;
            let canonical_function = functions[function_index].canonical;
            let plan = semantic_ssa
                .plan_for_function(site.semantic_function)
                .ok_or_else(|| {
                    assert_origin_invalid_v1(Some(site), "missing source SSA function")
                })?;
            if !plan.plan().is_reachable(fe2o3_mir_model::SsaBlockIdV1::new(
                site.semantic_block.index(),
            )) || span.kernel_ir_block != BlockId(site.semantic_block.index())
            {
                return Err(assert_origin_invalid_v1(
                    Some(site),
                    "block is absent from materialized SSA roster",
                ));
            }
            let canonical_block = graph.block(canonical_function, span.kernel_ir_block, budget)?;
            let emitted_block = assert_origin_block_v1(executable.module(), canonical_block)?;
            let span_end = span
                .first_operation_ordinal
                .checked_add(span.operation_count)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            if span_end as usize != emitted_block.operations.len() {
                return Err(assert_origin_invalid_v1(
                    Some(site),
                    "terminator span differs from materialized block",
                ));
            }
            seen_blocks[function_index] = seen_blocks[function_index]
                .checked_add(1)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            let SemanticTerminatorKindV1::Assert {
                expected, target, ..
            } = source_block.terminator().kind()
            else {
                continue;
            };
            let pending_index =
                assert_origin_find_v1(&self.records, budget, |row, _| Ok(row.site.cmp(&site)))?
                    .ok_or(SemanticKirAssertOriginErrorV1::MissingBinding { site })?;
            budget.charge_work(1)?;
            let pending = &self.records[pending_index];
            if pending.expected != *expected
                || pending.semantic_success != target.target()
                || pending.physical_success != BlockId(target.target().index())
                || pending.block != span.kernel_ir_block
                || pending.first_operation != span.first_operation_ordinal
                || pending.operation_count != span.operation_count
            {
                return Err(assert_origin_invalid_v1(
                    Some(site),
                    "source/span emission mismatch",
                ));
            }

            let actual_name = executable.module().functions[canonical_function.0 as usize]
                .id
                .as_str();
            assert_origin_string_work_v1(&pending.emitted_function, actual_name, budget)?;
            if pending.emitted_function != actual_name {
                return Err(assert_origin_invalid_v1(
                    Some(site),
                    "function emission identity differs",
                ));
            }
            let block = graph.block(canonical_function, pending.block, budget)?;
            let binding = seal_assert_occurrence_v1(
                pending,
                &self.arguments,
                block,
                &graph,
                executable.module(),
                source_function.blocks().len(),
                budget,
            )?;
            assert_origin_push_v1(
                &mut resolved,
                ResolvedAssertOriginV1 { site, binding },
                budget,
            )?;
        }
        for (function, seen) in functions.iter().zip(&seen_blocks) {
            budget.charge_work(1)?;
            if function.reachable_blocks != *seen {
                return Err(assert_origin_invalid_v1(
                    None,
                    "incomplete materialized source block roster",
                ));
            }
        }
        budget.charge_work(1)?;
        if resolved.len() != self.records.len() {
            return Err(assert_origin_invalid_v1(
                None,
                "missing or extra source assertion coverage",
            ));
        }
        // Sort physical occurrences first. Root/source aliases never duplicate facts.
        assert_origin_sort_v1(&mut resolved, budget, |a, b, _| {
            Ok(a.binding.block().cmp(&b.binding.block()))
        })?;
        budget.reserve_storage(std::mem::size_of::<SealedAssertOriginsV1>())?;
        let mut bindings: Vec<SemanticKirAssertConditionBindingV1> = Vec::new();
        let mut aliases = Vec::new();
        for row in &resolved {
            budget.charge_work(1)?;
            let existing = bindings
                .last()
                .filter(|binding| binding.block() == row.binding.block());
            if let Some(existing) = existing {
                if *existing != row.binding {
                    return Err(assert_origin_invalid_v1(
                        Some(row.site),
                        "shared assertion occurrence differs",
                    ));
                }
            } else {
                assert_origin_push_v1(&mut bindings, row.binding, budget)?;
            }
            let binding = bindings
                .len()
                .checked_sub(1)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            assert_origin_push_v1(
                &mut aliases,
                AssertOriginAliasV1 {
                    site: row.site,
                    binding,
                },
                budget,
            )?;
        }
        assert_origin_sort_v1(&mut aliases, budget, |a, b, _| Ok(a.site.cmp(&b.site)))?;
        for pair in aliases.windows(2) {
            budget.charge_work(1)?;
            if pair[0].site == pair[1].site {
                return Err(assert_origin_invalid_v1(
                    Some(pair[0].site),
                    "duplicate source assertion coverage",
                ));
            }
        }
        let payload_storage = std::mem::size_of::<SealedAssertOriginsV1>()
            .checked_add(assert_origin_bytes_v1::<AssertOriginAliasV1>(
                aliases.capacity(),
            )?)
            .and_then(|n| {
                n.checked_add(
                    functions
                        .capacity()
                        .checked_mul(std::mem::size_of::<AssertSourceFunctionV1>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    bindings
                        .capacity()
                        .checked_mul(std::mem::size_of::<SemanticKirAssertConditionBindingV1>())?,
                )
            })
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        assert_origin_drop_v1(resolved, budget)?;
        assert_origin_drop_v1(spans, budget)?;
        assert_origin_drop_v1(seen_blocks, budget)?;
        assert_origin_drop_v1(roots, budget)?;
        graph.release(budget)?;
        let mut name_bytes = 0_usize;
        for row in &self.records {
            budget.charge_work(1)?;
            name_bytes = name_bytes
                .checked_add(row.emitted_function.capacity())
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
        }
        assert_origin_drop_v1(self.records, budget)?;
        budget.release_storage(name_bytes)?;
        assert_origin_drop_v1(self.arguments, budget)?;
        Ok(SealedAssertOriginsV1 {
            aliases,
            functions,
            bindings,
            storage: SemanticKirAssertOriginStorageV1 { payload_storage },
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn seal_assert_occurrence_v1(
    pending: &PendingAssertOriginV1,
    arguments: &[ValueId],
    coordinate: AssertBlockCoordinateV1,
    graph: &AssertGraphIndexV1<'_>,
    module: &Module,
    source_block_count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<SemanticKirAssertConditionBindingV1> {
    let failure =
        BlockId(u32::try_from(source_block_count).map_err(|_| AssertOriginResourceV1::Arithmetic)?);
    seal_assert_occurrence_in_functions_v1(
        pending,
        arguments,
        coordinate,
        graph,
        &module.functions,
        failure,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn seal_assert_occurrence_in_functions_v1(
    pending: &PendingAssertOriginV1,
    arguments: &[ValueId],
    coordinate: AssertBlockCoordinateV1,
    graph: &AssertGraphIndexV1<'_>,
    functions: &[Function],
    expected_failure: BlockId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<SemanticKirAssertConditionBindingV1> {
    seal_assert_occurrence_in_functions_at_v1(
        pending,
        pending.first_operation,
        arguments,
        coordinate,
        graph,
        functions,
        expected_failure,
        budget,
    )
}

// The scoped caller checks its retained relocation witness before supplying
// this physical offset; the original source capture remains immutable.
#[allow(clippy::too_many_arguments)]
fn seal_assert_occurrence_in_functions_at_v1(
    pending: &PendingAssertOriginV1,
    first_operation: u32,
    arguments: &[ValueId],
    coordinate: AssertBlockCoordinateV1,
    graph: &AssertGraphIndexV1<'_>,
    functions: &[Function],
    expected_failure: BlockId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<SemanticKirAssertConditionBindingV1> {
    let bad = |detail| assert_origin_invalid_v1(Some(pending.site), detail);
    budget.charge_work(1)?;
    let block = assert_origin_function_block_v1(functions, coordinate)?;
    let end = first_operation
        .checked_add(pending.operation_count)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    if block.id != pending.block || end as usize != block.operations.len() {
        return Err(bad("terminator emission span is not exact"));
    }
    let argument_end = pending
        .argument_start
        .checked_add(pending.argument_count)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    let expected_arguments = arguments
        .get(pending.argument_start..argument_end)
        .ok_or_else(|| bad("pending successor arguments are missing"))?;
    let terminator = block
        .terminator
        .as_ref()
        .ok_or_else(|| bad("missing emitted terminator"))?;
    let success_id = pending.physical_success;
    // Resolve the exact target as well as the occurrence. SSA verification already
    // checked edge argument types and dominance; compare the captured slice here.
    graph.block(coordinate.function, success_id, budget)?;
    let outcome = match (pending.outcome, terminator) {
        (
            PendingAssertOutcomeV1::ElidedByExistingRule,
            Terminator::Branch { target, arguments },
        ) => {
            budget.charge_work(
                arguments
                    .len()
                    .checked_add(1)
                    .ok_or(AssertOriginResourceV1::Arithmetic)?,
            )?;
            if *target != success_id || arguments.as_slice() != expected_arguments {
                return Err(bad("elided assertion success occurrence differs"));
            }
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule {
                success_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                    source: coordinate,
                    successor: 0,
                },
            }
        }
        (
            PendingAssertOutcomeV1::Emitted {
                condition: expected_condition,
                failure,
            },
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            },
        ) => {
            let (success, success_arguments, actual_failure, failure_arguments, successor) =
                if pending.expected {
                    (
                        *then_target,
                        then_arguments,
                        *else_target,
                        else_arguments,
                        0,
                    )
                } else {
                    (
                        *else_target,
                        else_arguments,
                        *then_target,
                        then_arguments,
                        1,
                    )
                };
            budget.charge_work(
                success_arguments
                    .len()
                    .checked_add(1)
                    .ok_or(AssertOriginResourceV1::Arithmetic)?,
            )?;
            if *condition != expected_condition
                || success != success_id
                || success_arguments.as_slice() != expected_arguments
                || !failure_arguments.is_empty()
                || actual_failure != failure
                || failure != expected_failure
            {
                return Err(bad("assertion condition or successor occurrence differs"));
            }
            let definition = graph.definition(coordinate.function, *condition, budget)?;
            if !definition.boolean {
                return Err(bad("assertion condition definition is not Boolean"));
            }
            let failure_coordinate = graph.block(coordinate.function, failure, budget)?;
            budget.charge_work(1)?;
            let failure_block = assert_origin_function_block_v1(functions, failure_coordinate)?;
            if !failure_block.parameters.is_empty()
                || !matches!(failure_block.terminator, Some(Terminator::Unreachable))
                || failure_block.operations.len() != 1
            {
                return Err(bad("assertion failure is not the synthetic trap block"));
            }
            let operation = &failure_block.operations[0];
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                return Err(bad("assertion failure has no synthetic trap"));
            };
            budget.charge_work(
                callee
                    .as_str()
                    .len()
                    .checked_add(arguments.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(AssertOriginResourceV1::Arithmetic)?,
            )?;
            if !operation.results.is_empty()
                || AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                    != Some(AmdGpuDiagnosticOperation::Trap)
            {
                return Err(bad("assertion failure operation differs"));
            }
            SemanticKirAssertConditionOutcomeV1::Emitted {
                condition_use: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                    block: coordinate,
                    operand: 0,
                },
                definition: definition.coordinate,
                success_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                    source: coordinate,
                    successor,
                },
                failure_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                    source: coordinate,
                    successor: 1 - successor,
                },
            }
        }
        _ => return Err(bad("assertion emission rule and terminator differ")),
    };
    Ok(SemanticKirAssertConditionBindingV1 {
        expected: pending.expected,
        semantic_success: pending.semantic_success,
        outcome,
    })
}

#[cfg(test)]
#[path = "production_assert_origins_v1_tests.rs"]
mod assert_origins_v1_tests;

include!("production_instance_assert_origins_v1.rs");
