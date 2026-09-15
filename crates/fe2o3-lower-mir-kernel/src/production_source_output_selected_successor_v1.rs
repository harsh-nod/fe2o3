/// One checked Boolean successor occurrence mapped back to the replayed source.
/// This is control placement, not effect omission or assertion-discharge authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceOutputSelectedSuccessorV1 {
    input: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    semantic_ordinal: u32,
    semantic_target: SemanticBlockIdV1,
}

impl ProductionSourceOutputSelectedSuccessorV1 {
    /// Exact selected input edge in coordinate-preserved N/B, never an O edge.
    pub const fn input(self) -> fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
        self.input
    }

    /// Original source successor occurrence: explicit target zero or otherwise one.
    /// Equal target block IDs do not collapse these distinct edge occurrences.
    pub const fn semantic_ordinal(self) -> u32 {
        self.semantic_ordinal
    }

    /// Original semantic target block authenticated by the actual N terminator.
    pub const fn semantic_target(self) -> SemanticBlockIdV1 {
        self.semantic_target
    }
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Maps a checked selected Boolean edge to its exact source occurrence.
    ///
    /// `None` means the checker supplied no selection; it does not prove every
    /// source successor executable. A selected edge requires an executable
    /// original source block and the exact existing one-explicit-target Boolean
    /// switch lowering. Assertions, generated control splits, and other selected
    /// terminators are refused; assertions retain their separate outcome query.
    /// No source effect may be discarded merely because this query succeeds.
    ///
    /// Queries reuse the source/block indexes and allocate no storage. Each query
    /// pays its own lookups and fixed descriptor work on the caller's ledger.
    /// The source executable, origin payload, checked output, and view must remain
    /// reserved; B and any separate source C receipt remain caller obligations.
    pub fn selected_successor(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionSourceOutputSelectedSuccessorV1>, ProductionSourceOutputErrorV1>
    {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        if !self
            .source
            .assert_origins()
            .is_materialized_block(owner, function, block, budget)
            .map_err(Error::SourceOrigin)?
        {
            return Err(Error::Invalid(
                "selected successor source block is not materialized",
            ));
        }
        let site = SemanticKirAssertSiteV1::new(owner, function, block);
        let ordinal = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            source_output_site_cmp_v1(row.source, site, budget)
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid(
            "selected successor source block row is absent",
        ))?;
        budget.charge_work(2).map_err(Error::Resource)?;
        let row = &self.blocks[ordinal];
        let Some(selected) = row.control.selected_successor else {
            return Ok(None);
        };
        budget.charge_work(3).map_err(Error::Resource)?;
        if !row.control.reachable || selected.source != row.original {
            return Err(Error::Invalid(
                "selected successor input block is not executable or exact",
            ));
        }
        budget.charge_work(4).map_err(Error::Resource)?;
        let original_function = self
            .source
            .executable()
            .module()
            .functions
            .get(row.original.function.0 as usize)
            .ok_or(Error::Invalid(
                "selected successor original function is absent",
            ))?;
        let original_body = original_function
            .body
            .as_ref()
            .ok_or(Error::Invalid("selected successor original body is absent"))?;
        let original_block = original_body
            .blocks
            .get(row.original.block as usize)
            .ok_or(Error::Invalid(
                "selected successor original block is absent",
            ))?;
        let semantic = self.source.semantic_ssa().source_semantic();
        let source_function =
            semantic
                .functions()
                .get(function.index() as usize)
                .ok_or(Error::Invalid(
                    "selected successor semantic function is absent",
                ))?;
        source_output_selected_semantic_edge_v1(
            semantic.types(),
            source_function,
            block,
            original_block,
            selected,
            budget,
        )
        .map(Some)
    }
}

fn source_output_selected_semantic_edge_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
    original: &BasicBlock,
    selected: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionSourceOutputSelectedSuccessorV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    // Source block lookup, exact N block ID, both terminator tags, edge ordinal.
    budget.charge_work(5).map_err(Error::Resource)?;
    let source = function
        .blocks()
        .get(block.index() as usize)
        .ok_or(Error::Invalid(
            "selected successor semantic block is absent",
        ))?;
    if original.id != BlockId(block.index()) || selected.successor > 1 {
        return Err(Error::Invalid(
            "selected successor block or edge ordinal differs",
        ));
    }
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = source.terminator().kind()
    else {
        return Err(Error::Invalid(
            "selected successor requires an ordinary Boolean source switch",
        ));
    };
    let Some(Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    }) = &original.terminator
    else {
        return Err(Error::Invalid(
            "selected successor requires the exact N conditional branch",
        ));
    };
    // Operand-type dispatch, type lookup, Boolean shape, singleton target shape.
    budget.charge_work(4).map_err(Error::Resource)?;
    let declaration = types
        .get(semantic_operand_type(discriminant).index() as usize)
        .ok_or(Error::Invalid(
            "selected successor source selector type is absent",
        ))?;
    if !matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ) {
        return Err(Error::Invalid(
            "selected successor source selector is not Boolean",
        ));
    }
    let [explicit] = targets.values() else {
        return Err(Error::Invalid(
            "selected successor source switch has multiple explicit targets",
        ));
    };
    // Match the existing lower_terminator Boolean polarity and both targets,
    // not just the selected block ID: duplicate-target occurrences stay distinct.
    budget.charge_work(5).map_err(Error::Resource)?;
    if explicit.value() > 1 {
        return Err(Error::Invalid(
            "selected successor source switch value is not Boolean",
        ));
    }
    let explicit_is_true = explicit.value() == 1;
    let (expected_then, expected_else) = if explicit_is_true {
        (explicit.edge().target(), targets.otherwise().target())
    } else {
        (targets.otherwise().target(), explicit.edge().target())
    };
    if *then_target != BlockId(expected_then.index())
        || *else_target != BlockId(expected_else.index())
    {
        return Err(Error::Invalid(
            "selected successor original target occurrences differ from source",
        ));
    }
    let semantic_ordinal = u32::from((selected.successor == 0) != explicit_is_true);
    budget.charge_work(2).map_err(Error::Resource)?;
    let semantic_target = if semantic_ordinal == 0 {
        explicit.edge().target()
    } else {
        targets.otherwise().target()
    };
    Ok(ProductionSourceOutputSelectedSuccessorV1 {
        input: selected,
        semantic_ordinal,
        semantic_target,
    })
}
