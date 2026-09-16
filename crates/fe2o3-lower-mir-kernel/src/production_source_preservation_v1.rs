// Reference-independent, closed source/N rules. Importer locators select
// candidates; only the actual source and N rule checks below establish a row.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourcePreservationRuleV1 {
    Borrow,
    Payload,
    Scalar,
    Discriminant,
    Store,
    Invocation,
    Getter,
    Goto,
    Switch,
    Return,
    Unreachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourcePreservationCoverageV1 {
    source_block: u32,
    source_statement: Option<u32>,
    neutral_block: u32,
    first_operation: usize,
    end_operation: usize,
    rule: SourcePreservationRuleV1,
}

/// Exact subject of the conditional source/N rule. These are retained
/// preconditions, not newly discharged allocation, alias or launch claims.
pub struct ProductionSourcePreservationPreconditionsV1 {
    source_argument: u32,
    source_local: SemanticLocalIdV1,
    source_type: SemanticTypeIdV1,
    neutral_argument: u32,
    neutral_slice: ValueId,
    invocation: ValueId,
    extent: ValueId,
    condition: ValueId,
    pointer: ValueId,
    discriminator: ValueId,
}

impl ProductionSourcePreservationPreconditionsV1 {
    /// Original source argument whose valid, exclusively owned Global U32
    /// slice supplies the allocation, length and lifetime. The caller still
    /// owes byte-extent representability and 4-byte alignment of that slice.
    pub const fn source_argument(&self) -> u32 {
        self.source_argument
    }
    /// Exact original-N slice parameter, not an output allocation identity.
    pub const fn neutral_argument(&self) -> u32 {
        self.neutral_argument
    }
    /// Exact admitted source local and type, retained with the source owner.
    pub const fn source_subject(&self) -> (SemanticLocalIdV1, SemanticTypeIdV1) {
        (self.source_local, self.source_type)
    }
    /// Exact N slice, Global-X value, extent and active-lane predicate.
    pub const fn neutral_subject(&self) -> (ValueId, ValueId, ValueId, ValueId) {
        (
            self.neutral_slice,
            self.invocation,
            self.extent,
            self.condition,
        )
    }
    /// This subset accesses memory only when Global-X INDEX is less than the
    /// exact slice length. It does not establish a concrete launch allocation.
    pub const fn requires_valid_exclusive_global_u32_slice(&self) -> bool {
        true
    }
    /// The caller must discharge representability of base + 4 * Global-X,
    /// including inactive lanes where N computes but does not dereference it.
    pub const fn requires_representable_invocation_address(&self) -> bool {
        true
    }
}

/// One fully covered selected source body and its original N function. Private
/// rule/index storage remains retained for the entire enclosing callback.
pub struct ProductionSourcePreservationRootV1 {
    selected_root: SemanticFunctionIdV1,
    selected_body: SemanticFunctionIdV1,
    neutral_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    preconditions: ProductionSourcePreservationPreconditionsV1,
    coverage: Vec<SourcePreservationCoverageV1>,
    source_index: SourceOutputInvocationSourceIndexV1,
    source_facts: SourceOutputIdentitySourceV1,
    source_blocks: Vec<u32>,
    neutral_blocks: Vec<bool>,
    incoming: Vec<usize>,
    ready: Vec<usize>,
    budget_identity: *const (),
    live_floor: usize,
}

impl ProductionSourcePreservationRootV1 {
    /// Exact admitted selected-root identity, not an ordinal in O.
    pub const fn selected_root(&self) -> SemanticFunctionIdV1 {
        self.selected_root
    }
    /// Body selected by the admitted source entry association.
    pub const fn selected_body(&self) -> SemanticFunctionIdV1 {
        self.selected_body
    }
    /// Actual original-N function coordinate.
    pub const fn neutral_function(&self) -> fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1 {
        self.neutral_function
    }
    /// Conditional rule premises, not newly discharged runtime obligations.
    pub const fn preconditions(&self) -> &ProductionSourcePreservationPreconditionsV1 {
        &self.preconditions
    }
    /// Count of retained, individually checked source statement/terminator rows.
    pub fn checked_rule_count(&self) -> usize {
        self.coverage.len()
    }
    /// One immutable numeric coverage row. Traversal is explicitly paid on the
    /// caller's shared ledger; returned coordinates confer no independent owner.
    pub fn checked_rule_v1(
        &self,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        Option<(u32, Option<u32>, u32, std::ops::Range<usize>, &'static str)>,
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(8)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        if self.budget_identity != std::ptr::from_ref(budget).cast::<()>()
            || budget.storage() < self.live_floor
        {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        Ok(self.coverage.get(ordinal).map(|row| {
            (
                row.source_block,
                row.source_statement,
                row.neutral_block,
                row.first_operation..row.end_operation,
                match row.rule {
                    SourcePreservationRuleV1::Borrow => "borrow",
                    SourcePreservationRuleV1::Payload => "payload",
                    SourcePreservationRuleV1::Scalar => "scalar",
                    SourcePreservationRuleV1::Discriminant => "discriminant",
                    SourcePreservationRuleV1::Store => "store",
                    SourcePreservationRuleV1::Invocation => "invocation",
                    SourcePreservationRuleV1::Getter => "getter",
                    SourcePreservationRuleV1::Goto => "goto",
                    SourcePreservationRuleV1::Switch => "switch",
                    SourcePreservationRuleV1::Return => "return",
                    SourcePreservationRuleV1::Unreachable => "unreachable",
                },
            )
        }))
    }
}

/// Callback-only conjunction of explicit source/N rules for every original
/// R1 root. This is neither optional user-reference verification nor aggregate
/// execution, optimized-output preservation, signed or artifact authority.
/// Budget address/floor checks are not replacement-resistant ledger identity:
/// callers must not replace a budget in place or release live owner receipts.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionScopedSourcePreservationV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionScopedSourcePreservationV1<'static, 'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionBorrowedRankedCorrespondenceV1,
///     ProductionSourcePreservationRootV1, ProductionSourceOutputErrorV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'a>(original: &'a ProductionBorrowedRankedCorrespondenceV1<'a>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<&'a [ProductionSourcePreservationRootV1], ProductionSourceOutputErrorV1>
/// {
///     original.with_source_preservation_v1(budget, |checked, _| Ok(checked.roots()))
/// }
/// ```
pub struct ProductionScopedSourcePreservationV1<'proof, 'scope> {
    original: &'proof ProductionBorrowedRankedCorrespondenceV1<'scope>,
    roots: &'proof [ProductionSourcePreservationRootV1],
    budget_identity: *const (),
    floor: usize,
}

impl ProductionScopedSourcePreservationV1<'_, '_> {
    /// Entire original ordered root roster; this borrow cannot escape its scope.
    pub fn roots(&self) -> &[ProductionSourcePreservationRootV1] {
        self.roots
    }
    /// Rechecks exact live R1 owner and the retained relation storage floor.
    pub fn require_original_v1(
        &self,
        original: &ProductionBorrowedRankedCorrespondenceV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        budget
            .charge_work(4)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        if self.budget_identity != std::ptr::from_ref(budget).cast::<()>()
            || budget.storage() < self.floor
        {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        if !std::ptr::eq(self.original, original) || self.roots.len() != original.root_count() {
            return Err(ProductionSourceOutputErrorV1::Invalid(
                "source preservation original roster differs",
            ));
        }
        Ok(())
    }
}

impl<'scope> ProductionBorrowedRankedCorrespondenceV1<'scope> {
    /// Independently checks the closed source/N subset using this exact live
    /// source and original N. No importer replay, optimizer, O facts, external
    /// reference or caller-selected expected graph supplies rule acceptance.
    /// All scratch drops before floor restoration, including callback unwind.
    /// The callback's full retained floor is checked before those owners drop.
    /// Accounting takes precedence over callback Err or panic; otherwise the
    /// exact callback error or panic payload is preserved after cleanup.
    pub fn with_source_preservation_v1<T>(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
        next: impl for<'proof> FnOnce(
            &ProductionScopedSourcePreservationV1<'proof, 'scope>,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<T, ProductionSourceOutputErrorV1>,
    ) -> Result<T, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        // Original entry/scope eight plus four for callback catch/postflight.
        budget.charge_work(12).map_err(Error::Resource)?;
        let source = self.materialized();
        let capture = source
            .semantic_ssa()
            .occurrence_storage()
            .ok_or(Error::Invalid("source preservation capture absent"))?;
        let retained = source
            .executable_storage()
            .retained_storage()
            .checked_add(source.assert_origin_storage().payload_storage())
            .and_then(|bytes| bytes.checked_add(capture.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < retained {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        if self.roots.is_empty()
            || self.roots.len() != source.semantic_ssa().source_semantic().roots().len()
            || self.roots.len() != self.reports.len()
        {
            return Err(Error::Invalid(
                "source preservation all-root roster incomplete",
            ));
        }
        let floor = budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget
                .reserve_storage(
                    std::mem::size_of::<Vec<ProductionSourcePreservationRootV1>>()
                        .checked_add(std::mem::size_of::<
                            ProductionScopedSourcePreservationV1<'_, '_>,
                        >())
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                )
                .map_err(Error::Resource)?;
            let mut rows = Vec::new();
            for (ordinal, root) in self.roots.iter().enumerate() {
                budget.charge_work(6).map_err(Error::Resource)?;
                if source.semantic_ssa().source_semantic().roots().get(ordinal)
                    != Some(&root.selected_root)
                    || self.reports[ordinal].semantic_sha256()
                        != source
                            .semantic_ssa()
                            .source_semantic()
                            .semantic_sha256()
                            .as_bytes()
                {
                    return Err(Error::Invalid("source preservation R1/source root differs"));
                }
                let row = source_preservation_root_v1(
                    source,
                    source.executable().module(),
                    root.selected_root,
                    budget,
                )?;
                assert_origin_push_v1(&mut rows, row, budget).map_err(Error::SourceOrigin)?;
                // The row header moved into the separately paid actual Vec slot.
                budget
                    .release_storage(std::mem::size_of::<ProductionSourcePreservationRootV1>())
                    .map_err(Error::Resource)?;
            }
            let budget_identity = std::ptr::from_ref(budget).cast::<()>();
            let live_floor = budget.storage();
            for row in &mut rows {
                budget.charge_work(2).map_err(Error::Resource)?;
                row.budget_identity = budget_identity;
                row.live_floor = live_floor;
            }
            let checked = ProductionScopedSourcePreservationV1 {
                original: self,
                roots: &rows,
                budget_identity,
                floor: live_floor,
            };
            let callback =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(&checked, budget)));
            // Rows, their indexes, and the checked view are still live here.
            // This fixed postflight was prepaid at scope entry, so exhausted
            // Work cannot mask a storage violation or replace the body error.
            if checked.budget_identity != std::ptr::from_ref(budget).cast::<()>()
                || budget.storage() < checked.floor
            {
                return Err(Error::Resource(AssertOriginResourceV1::Accounting));
            }
            match callback {
                Ok(value) => value,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }));
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        budget.release_storage(release).map_err(Error::Resource)?;
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn source_preservation_u32_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    )
}

fn source_preservation_root_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    candidate: &Module,
    selected_root: SemanticFunctionIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionSourcePreservationRootV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(8).map_err(Error::Resource)?;
    let semantic = materialized.semantic_ssa().source_semantic();
    let capture = materialized
        .semantic_ssa()
        .occurrence_storage()
        .ok_or(Error::Invalid("source preservation capture absent"))?;
    let retained = materialized
        .executable_storage()
        .retained_storage()
        .checked_add(materialized.assert_origin_storage().payload_storage())
        .and_then(|bytes| bytes.checked_add(capture.retained_storage()))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    if budget.storage() < retained {
        return Err(Error::Resource(AssertOriginResourceV1::Accounting));
    }
    // Entry selection is an admitted association, not a wrapper-semantics
    // theorem. This first rule family covers direct Unit bodies only.
    budget
        .charge_work(
            semantic
                .roots()
                .len()
                .checked_add(8)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        )
        .map_err(Error::Resource)?;
    let selected = semantic
        .functions()
        .get(selected_root.index() as usize)
        .ok_or(Error::Invalid("source preservation selected root absent"))?;
    let entry = selected
        .blocks()
        .get(selected.entry().index() as usize)
        .ok_or(Error::Invalid(
            "source preservation selected root entry absent",
        ))?;
    if let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind()
        && matches!(
            semantic.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { .. })
        )
    {
        return Err(Error::Invalid(
            "source preservation unsupported live entry wrapper behavior",
        ));
    }
    let selection = semantic
        .select_kernel_body_for_root_v1(selected_root)
        .ok_or(Error::Invalid("source preservation selected body absent"))?;
    let selected_body = selection.body();
    let source = semantic
        .functions()
        .get(selected_body.index() as usize)
        .ok_or(Error::Invalid("source preservation selected body absent"))?;
    let mut correspondence = None;
    for row in materialized.correspondence.lowered_functions() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if row.correspondence_owner() != selected_root {
            continue;
        }
        if row.role() != SemanticKirFunctionRoleV1::KernelEntry
            || row.semantic_function() != selected_body
            || correspondence.replace(row).is_some()
        {
            return Err(Error::Invalid(
                "source preservation unsupported live callable closure",
            ));
        }
    }
    let correspondence =
        correspondence.ok_or(Error::Invalid("source preservation N function absent"))?;
    let mut function = None;
    for (ordinal, row) in candidate.functions.iter().enumerate() {
        budget
            .charge_work(
                3usize
                    .checked_add(row.id.as_str().len())
                    .and_then(|v| v.checked_add(correspondence.kernel_ir_function().as_str().len()))
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
            )
            .map_err(Error::Resource)?;
        if &row.id == correspondence.kernel_ir_function()
            && function.replace((ordinal, row)).is_some()
        {
            return Err(Error::Invalid("source preservation N function duplicated"));
        }
    }
    let (ordinal, function) =
        function.ok_or(Error::Invalid("source preservation N function absent"))?;
    let neutral_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(ordinal).map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
    );
    let body = function
        .body
        .as_ref()
        .ok_or(Error::Invalid("source preservation N body absent"))?;
    let mut witness = None;
    for block in source.blocks() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                ..
            }) = semantic.callables().get(call.callee().index() as usize)
        {
            let Some(operand) = call.arguments().get(1) else {
                return Err(Error::Invalid("source preservation getter witness absent"));
            };
            let place = source_output_identity_plain_place_v1(operand).ok_or(Error::Invalid(
                "source preservation getter witness is not plain",
            ))?;
            if witness.replace(place.local()).is_some() {
                return Err(Error::Invalid("source preservation getter duplicated"));
            }
        }
    }
    let witness = witness.ok_or(Error::Invalid("source preservation getter absent"))?;
    // The existing source index reserves its own header. All remaining row
    // headers, including the source-fact Vec headers, are prepaid here.
    budget
        .reserve_storage(
            std::mem::size_of::<ProductionSourcePreservationRootV1>()
                .checked_sub(std::mem::size_of::<SourceOutputInvocationSourceIndexV1>())
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
        )
        .map_err(Error::Resource)?;
    let index = source_output_invocation_source_index_from_source_v1(
        materialized,
        selected_root,
        selected_body,
        neutral_function,
        budget,
    )?;
    let facts =
        source_output_identity_source_from_source_v1(materialized, &index, witness, budget)?;
    let mut row = ProductionSourcePreservationRootV1 {
        selected_root,
        selected_body,
        neutral_function,
        preconditions: source_preservation_parameter_v1(
            materialized,
            selected_root,
            selected_body,
            function,
            &facts,
            budget,
        )?,
        coverage: Vec::new(),
        source_index: index,
        source_facts: facts,
        source_blocks: Vec::new(),
        neutral_blocks: Vec::new(),
        incoming: Vec::new(),
        ready: Vec::new(),
        budget_identity: std::ptr::from_ref(budget).cast::<()>(),
        live_floor: 0,
    };
    source_preservation_blocks_v1(materialized, body, &mut row, budget)?;
    source_preservation_intrinsics_v1(materialized, body, &mut row, budget)?;
    source_preservation_rules_v1(materialized, function, &mut row, budget)?;
    source_preservation_acyclic_v1(materialized, &mut row, budget)?;
    budget.charge_work(2).map_err(Error::Resource)?;
    row.live_floor = budget.storage();
    Ok(row)
}

fn source_preservation_parameter_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    selected_root: SemanticFunctionIdV1,
    selected_body: SemanticFunctionIdV1,
    neutral: &Function,
    facts: &SourceOutputIdentitySourceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionSourcePreservationPreconditionsV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(12).map_err(Error::Resource)?;
    let semantic = materialized.semantic_ssa().source_semantic();
    let source = &semantic.functions()[selected_body.index() as usize];
    let local = &source.locals()[facts.slice.index() as usize];
    let SemanticLocalRoleV1::Argument(argument) = local.role() else {
        return Err(Error::Invalid("source preservation Slice is not a formal"));
    };
    let body = neutral
        .body
        .as_ref()
        .ok_or(Error::Invalid("source preservation N body absent"))?;
    if source.abi().source_input_types().len() != neutral.signature.parameters.len()
        || body.parameters.len() != neutral.signature.parameters.len()
        || !neutral.signature.results.is_empty()
        || !matches!(
            semantic.types()[source.abi().source_output_type().index() as usize].shape(),
            SemanticTypeShapeV1::Unit
        )
    {
        return Err(Error::Invalid(
            "source preservation parameter/result census differs",
        ));
    }
    let mut slice = None;
    for (position, (&ty, actual)) in source
        .abi()
        .source_input_types()
        .iter()
        .zip(&neutral.signature.parameters)
        .enumerate()
    {
        budget.charge_work(8).map_err(Error::Resource)?;
        if position == argument as usize {
            if ty != local.ty()
                || !matches!(actual, Type::Slice(slice)
                if slice.element.as_ref() == &Type::Scalar(ScalarType::U32)
                    && slice.address_space == AddressSpace::Global
                    && slice.access == AccessMode::ReadWrite)
            {
                return Err(Error::Invalid(
                    "source preservation Slice representation differs",
                ));
            }
            let fields = match semantic.types()[ty.index() as usize].shape() {
                SemanticTypeShapeV1::Aggregate(value) => value.fields().len(),
                _ => {
                    return Err(Error::Invalid(
                        "source preservation source Slice is not authenticated aggregate",
                    ));
                }
            };
            // Prepay the descriptor and layout scans performed by the existing
            // source ABI recognizer, plus its one scalar Box and Type header.
            let work = semantic
                .callables()
                .len()
                .checked_mul(12)
                .and_then(|n| fields.checked_mul(20).and_then(|m| n.checked_add(m)))
                .and_then(|n| n.checked_add(32))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.charge_work(work).map_err(Error::Resource)?;
            let bytes = std::mem::size_of::<Option<Type>>()
                .checked_add(std::mem::size_of::<Type>())
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.reserve_storage(bytes).map_err(Error::Resource)?;
            let matches = {
                let expected = authenticated_disjoint_slice_parameter(
                    semantic.types(),
                    semantic.callables(),
                    source,
                    argument,
                    ty,
                );
                expected.as_ref() == Some(actual)
            };
            budget.release_storage(bytes).map_err(Error::Resource)?;
            if !matches {
                return Err(Error::Invalid(
                    "source preservation source Slice ABI differs",
                ));
            }
            slice = body.parameters.get(position).copied();
        } else if !source_preservation_u32_v1(semantic.types(), ty)
            || actual != &Type::Scalar(ScalarType::U32)
            || source.abi().source_argument_ownership().get(position)
                != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
            || !matches!(
                source
                    .abi()
                    .adjusted_arguments()
                    .get(position)
                    .map(|arg| arg.mode()),
                Some(SemanticAbiPassModeV1::Direct(_))
            )
            || source
                .abi()
                .adjusted_arguments()
                .get(position)
                .is_none_or(|arg| arg.ty() != ty || arg.value().adjusted().is_some())
        {
            return Err(Error::Invalid(
                "source preservation unsupported scalar formal",
            ));
        }
    }
    let slice = slice.ok_or(Error::Invalid("source preservation Slice parameter absent"))?;
    let mut matched = 0usize;
    for binding in materialized.correspondence.parameter_bindings() {
        budget.charge_work(6).map_err(Error::Resource)?;
        if binding.correspondence_owner() == selected_root
            && binding.semantic_function() == selected_body
            && binding.semantic_local() == facts.slice
        {
            if binding.kernel_ir_value() != slice {
                return Err(Error::Invalid(
                    "source preservation Slice parameter locator differs",
                ));
            }
            matched = matched
                .checked_add(1)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        }
    }
    if matched != 1 {
        return Err(Error::Invalid(
            "source preservation Slice parameter locator absent or duplicated",
        ));
    }
    Ok(ProductionSourcePreservationPreconditionsV1 {
        source_argument: argument,
        source_local: facts.slice,
        source_type: local.ty(),
        neutral_argument: argument,
        neutral_slice: slice,
        invocation: slice,
        extent: slice,
        condition: slice,
        pointer: slice,
        discriminator: slice,
    })
}

fn source_preservation_blocks_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    body: &fe2o3_kernel_ir::FunctionBody,
    row: &mut ProductionSourcePreservationRootV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(6).map_err(Error::Resource)?;
    let source = &materialized.semantic_ssa().source_semantic().functions()
        [row.selected_body.index() as usize];
    if source.blocks().is_empty() || source.blocks().len() != body.blocks.len() {
        return Err(Error::Invalid(
            "source preservation complete block census differs",
        ));
    }
    for _ in source.blocks() {
        budget.charge_work(2).map_err(Error::Resource)?;
        assert_origin_push_v1(&mut row.source_blocks, u32::MAX, budget)
            .map_err(Error::SourceOrigin)?;
        assert_origin_push_v1(&mut row.neutral_blocks, false, budget)
            .map_err(Error::SourceOrigin)?;
        assert_origin_push_v1(&mut row.incoming, 0, budget).map_err(Error::SourceOrigin)?;
    }
    for binding in materialized.correspondence.blocks() {
        budget.charge_work(7).map_err(Error::Resource)?;
        if binding.correspondence_owner() != row.selected_root
            || binding.semantic_function() != row.selected_body
        {
            continue;
        }
        let source_index = binding.semantic_block().index() as usize;
        let target =
            assert_origin_find_v1(&row.source_index.blocks, budget, |candidate, budget| {
                budget.charge_work(1)?;
                Ok(candidate.0.cmp(&binding.kernel_ir_block()))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("source preservation block locator absent"))?;
        let neutral_index = row.source_index.blocks[target].1 as usize;
        if source_index >= row.source_blocks.len()
            || neutral_index >= body.blocks.len()
            || row.source_blocks[source_index] != u32::MAX
            || row.neutral_blocks[neutral_index]
            || body.blocks[neutral_index].id != binding.kernel_ir_block()
            || binding.source_statement_count() as usize
                != source.blocks()[source_index].statements().len()
            || !body.blocks[neutral_index].parameters.is_empty()
        {
            return Err(Error::Invalid(
                "source preservation block locator or merge differs",
            ));
        }
        row.source_blocks[source_index] = neutral_index as u32;
        row.neutral_blocks[neutral_index] = true;
    }
    for (index, &mapped) in row.source_blocks.iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        if mapped == u32::MAX || !row.neutral_blocks[index] {
            return Err(Error::Invalid(
                "source preservation complete block census differs",
            ));
        }
    }
    if row.source_blocks[source.entry().index() as usize] != 0 {
        return Err(Error::Invalid("source preservation entry differs"));
    }
    Ok(())
}

fn source_preservation_intrinsics_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    body: &fe2o3_kernel_ir::FunctionBody,
    row: &mut ProductionSourcePreservationRootV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let (block, span) = source_output_identity_span_from_source_v1(
        materialized,
        &row.source_index,
        row.source_facts.anchor.producer.source().get(),
        None,
        budget,
    )?;
    budget.charge_work(10).map_err(Error::Resource)?;
    let [producer] = body
        .blocks
        .get(block.block as usize)
        .and_then(|block| block.operations.get(span))
        .ok_or(Error::Invalid(
            "source preservation producer span outside N",
        ))?
    else {
        return Err(Error::Invalid("source preservation Global-X rule differs"));
    };
    if producer.results.len() != 1
        || producer.results[0].ty != Type::INDEX
        || !matches!(&producer.kind, OperationKind::Intrinsic(intrinsic) if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d())
    {
        return Err(Error::Invalid("source preservation Global-X rule differs"));
    }
    let index = producer.results[0].id;
    let (block, span) = source_output_identity_span_from_source_v1(
        materialized,
        &row.source_index,
        row.source_facts.anchor.get.source().get(),
        None,
        budget,
    )?;
    budget.charge_work(30).map_err(Error::Resource)?;
    let [length, compare, data, gep] = body
        .blocks
        .get(block.block as usize)
        .and_then(|block| block.operations.get(span))
        .ok_or(Error::Invalid("source preservation getter span outside N"))?
    else {
        return Err(Error::Invalid("source preservation getter rule differs"));
    };
    let slice = row.preconditions.neutral_slice;
    if [length, compare, data, gep].iter().any(|op| op.results.len() != 1)
        || length.results[0].ty != Type::INDEX || compare.results[0].ty != Type::BOOL
        || !matches!(length.kind, OperationKind::SliceLength { slice: actual } if actual == slice)
        || !matches!(compare.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs } if lhs == index && rhs == length.results[0].id)
        || !matches!(data.kind, OperationKind::SliceData { slice: actual } if actual == slice)
        || !matches!(gep.kind, OperationKind::GetElementPointer { base, offset } if base == data.results[0].id && offset == index)
        || [data, gep].iter().any(|operation| !matches!(&operation.results[0].ty, Type::Pointer(pointer)
            if pointer.pointee.as_ref() == &Type::Scalar(ScalarType::U32)
                && pointer.address_space == AddressSpace::Global && pointer.access == AccessMode::ReadWrite))
    { return Err(Error::Invalid("source preservation getter rule differs")); }
    row.preconditions.invocation = index;
    row.preconditions.extent = length.results[0].id;
    row.preconditions.condition = compare.results[0].id;
    row.preconditions.pointer = gep.results[0].id;
    Ok(())
}

fn source_preservation_constant_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    operand: &SemanticOperandV1,
    operation: &Operation,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ValueId, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(10).map_err(Error::Resource)?;
    let SemanticOperandV1::Constant(constant) = operand else {
        return Err(Error::Invalid("source preservation scalar constant absent"));
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(Error::Invalid(
            "source preservation unsupported scalar constant",
        ));
    };
    if !source_preservation_u32_v1(
        materialized.semantic_ssa().source_semantic().types(),
        constant.ty(),
    ) || value.size_bytes() != 4
        || operation.results.len() != 1
        || operation.results[0].ty != Type::Scalar(ScalarType::U32)
        || !matches!(operation.kind, OperationKind::Constant(Constant::U32(actual)) if u128::from(actual) == value.bits())
    {
        return Err(Error::Invalid(
            "source preservation scalar value rule differs",
        ));
    }
    Ok(operation.results[0].id)
}

fn source_preservation_scalar_operand_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    row: &ProductionSourcePreservationRootV1,
    body: &FunctionBody,
    operand: &SemanticOperandV1,
    site: (u32, u32),
    inline: Option<&Operation>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ValueId, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    source_output_global_scratch_scope_v1(budget, |budget| {
        budget
            .charge_work(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1)
            .map_err(Error::Resource)?;
        budget
            .reserve_storage(std::mem::size_of::<
                [Option<SsaValueV1>; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
            >())
            .map_err(Error::Resource)?;
        let mut visited = [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1];
        let source = &materialized.semantic_ssa().source_semantic().functions()
            [row.selected_body.index() as usize];
        let captured =
            source_output_invocation_capture_v1(materialized, row.selected_body, budget)?;
        let mut operand = operand;
        let mut site = site;
        let mut inline = inline;
        for depth in 0..visited.len() {
            budget.charge_work(depth + 14).map_err(Error::Resource)?;
            if let SemanticOperandV1::Constant(_) = operand {
                return source_preservation_constant_v1(
                    materialized,
                    operand,
                    inline.ok_or(Error::Invalid(
                        "source preservation scalar constant span absent",
                    ))?,
                    budget,
                );
            }
            let place = source_output_identity_plain_place_v1(operand).ok_or(Error::Invalid(
                "source preservation unsupported scalar source operand",
            ))?;
            if inline.is_some()
                || !source_preservation_u32_v1(
                    materialized.semantic_ssa().source_semantic().types(),
                    place.ty(),
                )
            {
                return Err(Error::Invalid(
                    "source preservation scalar copy rule differs",
                ));
            }
            let value = source_output_invocation_use_v1(
                &row.source_index,
                &captured,
                (0, site.0, site.1, 0, 0, 0, 0),
                place.local(),
                budget,
            )?;
            if visited[..depth].contains(&Some(value)) {
                return Err(Error::Invalid(
                    "source preservation scalar definition cycle",
                ));
            }
            visited[depth] = Some(value);
            let plan = materialized
                .semantic_ssa()
                .plan_for_function(row.selected_body)
                .ok_or(Error::Invalid("source preservation source plan absent"))?
                .plan();
            let mut formal = None;
            for entry in plan.entry_definitions() {
                budget.charge_work(4).map_err(Error::Resource)?;
                if entry.variable().get() == place.local().index()
                    && formal.replace(entry.value()).is_some()
                {
                    return Err(Error::Invalid(
                        "source preservation scalar entry duplicated",
                    ));
                }
            }
            if formal == Some(value) {
                let SemanticLocalRoleV1::Argument(argument) =
                    source.locals()[place.local().index() as usize].role()
                else {
                    return Err(Error::Invalid(
                        "source preservation scalar entry is not formal",
                    ));
                };
                return body
                    .parameters
                    .get(argument as usize)
                    .copied()
                    .ok_or(Error::Invalid("source preservation scalar N formal absent"));
            }
            let definition =
                source_output_identity_direct_definition_v1(&row.source_index, value, budget)?;
            let SourceOutputInvocationDefinitionV1::Event(event) = definition else {
                return Err(Error::Invalid(
                    "source preservation unsupported scalar CallReturn",
                ));
            };
            let event = captured
                .events()
                .get(event)
                .ok_or(Error::Invalid("source preservation scalar event absent"))?;
            let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
                event.site()
            else {
                return Err(Error::Invalid(
                    "source preservation scalar definition is not statement",
                ));
            };
            let SemanticStatementKindV1::Assign(assignment) =
                source.blocks()[block.get() as usize].statements()[statement as usize].kind()
            else {
                return Err(Error::Invalid(
                    "source preservation scalar definition is not assignment",
                ));
            };
            if assignment.destination().local() != place.local()
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != place.ty()
            {
                return Err(Error::Invalid(
                    "source preservation scalar definition subject differs",
                ));
            }
            let SemanticRvalueKindV1::Use(next) = assignment.value().kind() else {
                return Err(Error::Invalid(
                    "source preservation unsupported scalar definition",
                ));
            };
            let (coordinate, span) = source_output_identity_span_from_source_v1(
                materialized,
                &row.source_index,
                block.get(),
                Some(statement),
                budget,
            )?;
            let operations = body
                .blocks
                .get(coordinate.block as usize)
                .and_then(|block| block.operations.get(span))
                .ok_or(Error::Invalid(
                    "source preservation scalar definition span outside N",
                ))?;
            inline = match (next, operations) {
                (SemanticOperandV1::Constant(_), [operation]) => Some(operation),
                (SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_), []) => None,
                _ => {
                    return Err(Error::Invalid(
                        "source preservation scalar definition span differs",
                    ));
                }
            };
            operand = next;
            site = (block.get(), statement);
        }
        Err(Error::Invalid(
            "source preservation scalar definition depth exceeded",
        ))
    })
}

fn source_preservation_rules_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    function: &Function,
    row: &mut ProductionSourcePreservationRootV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let source = &materialized.semantic_ssa().source_semantic().functions()
        [row.selected_body.index() as usize];
    let body = function
        .body
        .as_ref()
        .ok_or(Error::Invalid("source preservation N body absent"))?;
    // Check the discriminator before following source block order: block
    // numbering is not dominance order and is not a semantic premise.
    let (coordinate, span) = source_output_identity_span_from_source_v1(
        materialized,
        &row.source_index,
        row.source_facts.discriminator_site.0,
        Some(row.source_facts.discriminator_site.1),
        budget,
    )?;
    budget.charge_work(14).map_err(Error::Resource)?;
    let [cast] = body
        .blocks
        .get(coordinate.block as usize)
        .and_then(|block| block.operations.get(span))
        .ok_or(Error::Invalid(
            "source preservation discriminator span outside N",
        ))?
    else {
        return Err(Error::Invalid(
            "source preservation Option discriminator rule differs",
        ));
    };
    let ty = source.locals()[row.source_facts.discriminator.index() as usize].ty();
    let expected =
        match materialized.semantic_ssa().source_semantic().types()[ty.index() as usize].shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }) => Type::Scalar(ScalarType::U64),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 64,
            }) => Type::Scalar(ScalarType::I64),
            _ => {
                return Err(Error::Invalid(
                    "source preservation unsupported Option discriminator type",
                ));
            }
        };
    if cast.results.len() != 1
        || cast.results[0].ty != expected
        || !matches!(&cast.kind, OperationKind::Cast { kind: CastKind::ZeroExtend, value, to }
            if *value == row.preconditions.condition && to == &expected)
    {
        return Err(Error::Invalid(
            "source preservation Option discriminator rule differs",
        ));
    }
    row.preconditions.discriminator = cast.results[0].id;
    for (source_block, block) in source.blocks().iter().enumerate() {
        budget.charge_work(4).map_err(Error::Resource)?;
        let neutral_block = row.source_blocks[source_block];
        let neutral = &body.blocks[neutral_block as usize];
        let mut cursor = 0;
        for (statement, source_statement) in block.statements().iter().enumerate() {
            let site = (source_block as u32, statement as u32);
            let (coordinate, span) = source_output_identity_span_from_source_v1(
                materialized,
                &row.source_index,
                site.0,
                Some(site.1),
                budget,
            )?;
            budget.charge_work(8).map_err(Error::Resource)?;
            if coordinate.block != neutral_block
                || span.start != cursor
                || span.end > neutral.operations.len()
            {
                return Err(Error::Invalid(
                    "source preservation complete operation census differs",
                ));
            }
            let operations = &neutral.operations[span.clone()];
            let rule = if site == row.source_facts.facts.receiver_site {
                if !operations.is_empty() {
                    return Err(Error::Invalid(
                        "source preservation Borrow alias rule differs",
                    ));
                }
                SourcePreservationRuleV1::Borrow
            } else if site == row.source_facts.facts.payload_site {
                if !operations.is_empty() {
                    return Err(Error::Invalid(
                        "source preservation payload alias rule differs",
                    ));
                }
                SourcePreservationRuleV1::Payload
            } else if site == row.source_facts.discriminator_site {
                SourcePreservationRuleV1::Discriminant
            } else {
                source_preservation_statement_v1(
                    materialized,
                    row,
                    body,
                    source_statement.kind(),
                    site,
                    operations,
                    budget,
                )?
            };
            cursor = span.end;
            assert_origin_push_v1(
                &mut row.coverage,
                SourcePreservationCoverageV1 {
                    source_block: site.0,
                    source_statement: Some(site.1),
                    neutral_block,
                    first_operation: span.start,
                    end_operation: span.end,
                    rule,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
        let (coordinate, span) = source_output_identity_span_from_source_v1(
            materialized,
            &row.source_index,
            source_block as u32,
            None,
            budget,
        )?;
        budget.charge_work(6).map_err(Error::Resource)?;
        if coordinate.block != neutral_block
            || span.start != cursor
            || span.end != neutral.operations.len()
        {
            return Err(Error::Invalid(
                "source preservation complete operation census differs",
            ));
        }
        let rule = source_preservation_terminator_v1(
            row,
            body,
            source_block as u32,
            block.terminator().kind(),
            &neutral.operations[span.clone()],
            neutral.terminator.as_ref(),
            budget,
        )?;
        assert_origin_push_v1(
            &mut row.coverage,
            SourcePreservationCoverageV1 {
                source_block: source_block as u32,
                source_statement: None,
                neutral_block,
                first_operation: span.start,
                end_operation: span.end,
                rule,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    Ok(())
}

fn source_preservation_statement_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    row: &ProductionSourcePreservationRootV1,
    body: &FunctionBody,
    statement: &SemanticStatementKindV1,
    site: (u32, u32),
    operations: &[Operation],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourcePreservationRuleV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(12).map_err(Error::Resource)?;
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Err(Error::Invalid(
            "source preservation unsupported source statement",
        ));
    };
    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
        return Err(Error::Invalid(
            "source preservation unsupported source rvalue",
        ));
    };
    if !source_preservation_u32_v1(
        materialized.semantic_ssa().source_semantic().types(),
        assignment.destination().ty(),
    ) || assignment.value().result_type() != assignment.destination().ty()
    {
        return Err(Error::Invalid(
            "source preservation unsupported assignment type",
        ));
    }
    let store = match assignment.destination().projections() {
        [] => false,
        [projection]
            if assignment.destination().local() == row.source_facts.facts.payload
                && matches!(projection.kind(), SemanticProjectionKindV1::Dereference) =>
        {
            true
        }
        _ => {
            return Err(Error::Invalid(
                "source preservation unsupported assignment destination",
            ));
        }
    };
    let (inline, store_operation) = match (operand, store, operations) {
        (SemanticOperandV1::Constant(_), false, [constant]) => (Some(constant), None),
        (SemanticOperandV1::Constant(_), true, [constant, store]) => (Some(constant), Some(store)),
        (SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_), false, []) => (None, None),
        (SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_), true, [store]) => {
            (None, Some(store))
        }
        _ => {
            return Err(Error::Invalid(
                "source preservation scalar/Store span rule differs",
            ));
        }
    };
    let value = source_preservation_scalar_operand_v1(
        materialized,
        row,
        body,
        operand,
        site,
        inline,
        budget,
    )?;
    if let Some(operation) = store_operation {
        // The unchanged source helper emits exactly one row per own Store in
        // canonical (block, statement) order and already searches that roster.
        let found =
            assert_origin_find_v1(&row.source_facts.stores, budget, |source_store, budget| {
                budget.charge_work(2)?;
                Ok(source_store.site.cmp(&site))
            })
            .map_err(Error::SourceOrigin)?;
        budget.charge_work(10).map_err(Error::Resource)?;
        if found.is_none_or(|ordinal| row.source_facts.stores[ordinal].site != site)
            || !operation.results.is_empty()
            || !matches!(operation.kind, OperationKind::Store { pointer, value: actual, access }
                if pointer == row.preconditions.pointer && actual == value
                    && access.address_space == AddressSpace::Global && access.alignment == 4 && !access.volatile)
        {
            return Err(Error::Invalid(
                "source preservation own Store value/address rule differs",
            ));
        }
        Ok(SourcePreservationRuleV1::Store)
    } else {
        Ok(SourcePreservationRuleV1::Scalar)
    }
}

fn source_preservation_target_v1(
    row: &ProductionSourcePreservationRootV1,
    body: &FunctionBody,
    target: SemanticBlockIdV1,
) -> Option<BlockId> {
    body.blocks
        .get(*row.source_blocks.get(target.index() as usize)? as usize)
        .map(|block| block.id)
}

fn source_preservation_terminator_v1(
    row: &ProductionSourcePreservationRootV1,
    body: &FunctionBody,
    source_block: u32,
    source: &SemanticTerminatorKindV1,
    operations: &[Operation],
    neutral: Option<&Terminator>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourcePreservationRuleV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(12).map_err(Error::Resource)?;
    let branch = |target| {
        matches!(neutral, Some(Terminator::Branch { target: actual, arguments })
        if Some(*actual) == source_preservation_target_v1(row, body, target) && arguments.is_empty())
    };
    match source {
        SemanticTerminatorKindV1::Call(call) => {
            let rule = if source_block == row.source_facts.anchor.producer.source().get() {
                if operations.len() != 1 {
                    return Err(Error::Invalid(
                        "source preservation producer census differs",
                    ));
                }
                SourcePreservationRuleV1::Invocation
            } else if source_block == row.source_facts.anchor.get.source().get() {
                if operations.len() != 4 {
                    return Err(Error::Invalid("source preservation getter census differs"));
                }
                SourcePreservationRuleV1::Getter
            } else {
                return Err(Error::Invalid(
                    "source preservation unsupported live callable behavior",
                ));
            };
            if !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
                || call
                    .destination()
                    .is_none_or(|to| !branch(to.edge().target()))
            {
                return Err(Error::Invalid(
                    "source preservation intrinsic continuation rule differs",
                ));
            }
            Ok(rule)
        }
        SemanticTerminatorKindV1::Goto(edge) if operations.is_empty() && branch(edge.target()) => {
            Ok(SourcePreservationRuleV1::Goto)
        }
        SemanticTerminatorKindV1::SwitchInt { targets, .. }
            if source_block == row.source_facts.switch.index() && operations.is_empty() =>
        {
            budget.charge_work(8).map_err(Error::Resource)?;
            let (actual_len, default_target, default_arguments) = match neutral {
                Some(Terminator::Switch {
                    selector,
                    cases,
                    default_target,
                    default_arguments,
                }) if *selector == row.preconditions.discriminator => {
                    (cases.len(), *default_target, default_arguments)
                }
                Some(Terminator::IntegerSwitch {
                    selector,
                    cases,
                    default_target,
                    default_arguments,
                }) if *selector == row.preconditions.discriminator => {
                    (cases.len(), *default_target, default_arguments)
                }
                _ => {
                    return Err(Error::Invalid(
                        "source preservation Option branch rule differs",
                    ));
                }
            };
            if actual_len != targets.values().len()
                || !default_arguments.is_empty()
                || Some(default_target)
                    != source_preservation_target_v1(row, body, targets.otherwise().target())
            {
                return Err(Error::Invalid(
                    "source preservation Option default rule differs",
                ));
            }
            for (ordinal, source_case) in targets.values().iter().enumerate() {
                budget.charge_work(8).map_err(Error::Resource)?;
                let (value, target, arguments) = match neutral {
                    Some(Terminator::Switch { cases, .. }) => (
                        u128::from(cases[ordinal].value),
                        cases[ordinal].target,
                        &cases[ordinal].arguments,
                    ),
                    Some(Terminator::IntegerSwitch { cases, .. }) => {
                        let value = match cases[ordinal].value {
                            Constant::I64(value) if value >= 0 => value as u128,
                            Constant::U64(value) => u128::from(value),
                            _ => {
                                return Err(Error::Invalid(
                                    "source preservation Option case type differs",
                                ));
                            }
                        };
                        (value, cases[ordinal].target, &cases[ordinal].arguments)
                    }
                    _ => unreachable!(),
                };
                if value != source_case.value()
                    || !arguments.is_empty()
                    || Some(target)
                        != source_preservation_target_v1(row, body, source_case.edge().target())
                {
                    return Err(Error::Invalid(
                        "source preservation Option polarity rule differs",
                    ));
                }
            }
            Ok(SourcePreservationRuleV1::Switch)
        }
        SemanticTerminatorKindV1::Return
            if operations.is_empty()
                && matches!(neutral, Some(Terminator::Return { values }) if values.is_empty()) =>
        {
            Ok(SourcePreservationRuleV1::Return)
        }
        SemanticTerminatorKindV1::Unreachable
            if operations.is_empty()
                && row
                    .source_facts
                    .fallback
                    .is_some_and(|block| block.index() == source_block)
                && matches!(neutral, Some(Terminator::Unreachable)) =>
        {
            Ok(SourcePreservationRuleV1::Unreachable)
        }
        _ => Err(Error::Invalid(
            "source preservation unsupported or different control rule",
        )),
    }
}

fn source_preservation_acyclic_v1(
    materialized: &ProductionPreRankedKirOwnerV1,
    row: &mut ProductionSourcePreservationRootV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let source = &materialized.semantic_ssa().source_semantic().functions()
        [row.selected_body.index() as usize];
    for (ordinal, block) in source.blocks().iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        block.terminator().kind().try_for_each_edge(|edge| {
            budget.charge_work(7).map_err(Error::Resource)?;
            if row.source_facts.fallback == Some(edge.target())
                && (ordinal != row.source_facts.switch.index() as usize
                    || edge.role()
                        != fe2o3_mir_model::semantic_mir_v1::SemanticEdgeRoleV1::SwitchOtherwise)
            {
                return Err(Error::Invalid(
                    "source preservation default has another incoming edge",
                ));
            }
            let count =
                row.incoming
                    .get_mut(edge.target().index() as usize)
                    .ok_or(Error::Invalid(
                        "source preservation control target outside body",
                    ))?;
            *count = count
                .checked_add(1)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            Ok(())
        })?;
    }
    for (block, &incoming) in row.incoming.iter().enumerate() {
        budget.charge_work(3).map_err(Error::Resource)?;
        if incoming == 0 {
            assert_origin_push_v1(&mut row.ready, block, budget).map_err(Error::SourceOrigin)?;
        }
    }
    let mut cursor = 0;
    while let Some(&block) = row.ready.get(cursor) {
        budget.charge_work(4).map_err(Error::Resource)?;
        cursor += 1;
        source.blocks()[block]
            .terminator()
            .kind()
            .try_for_each_edge(|edge| {
                budget.charge_work(5).map_err(Error::Resource)?;
                let target = edge.target().index() as usize;
                row.incoming[target] = row.incoming[target].checked_sub(1).ok_or(
                    Error::Invalid("source preservation control indegree differs"),
                )?;
                if row.incoming[target] == 0 {
                    assert_origin_push_v1(&mut row.ready, target, budget)
                        .map_err(Error::SourceOrigin)?;
                }
                Ok(())
            })?;
    }
    if cursor != source.blocks().len() {
        return Err(Error::Invalid(
            "source preservation unsupported control cycle",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_source_preservation_v1_tests.rs"]
mod source_preservation_tests;
