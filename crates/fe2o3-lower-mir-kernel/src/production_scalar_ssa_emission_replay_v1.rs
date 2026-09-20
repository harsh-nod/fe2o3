use super::*;

impl Sealed {
    pub(super) fn seal(
        original: &ProductionPreRankedKirOwnerV1,
        mut capture: Recorder,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        let (inventory, receipt) = Inventory::derive(original.executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let mut aliases = Vec::new();
        let mut function_index = Vec::new();
        for (ordinal, function) in capture.functions.iter_mut().enumerate() {
            budget.charge_work(2)?;
            let actual = inventory
                .function_for_name(&function.name, budget)?
                .ok_or(Error::Mismatch("captured physical function"))?;
            function.coordinate = Some(actual.coordinate);
            append(&mut function_index, (actual.coordinate, ordinal), budget)?;
            if function.unsupported_placement {
                continue;
            }
            for definition in &mut capture.definitions[function.definitions.clone()] {
                budget.charge_work(2)?;
                let expected = capture
                    .expected
                    .get(definition.expected)
                    .ok_or(Error::Mismatch("expected definition index"))?;
                for component in 0..expected.shape.width() {
                    let actual = inventory
                        .definition_for_value(
                            actual.coordinate,
                            definition.values[component],
                            budget,
                        )?
                        .ok_or(Error::Mismatch("emitted value definition"))?;
                    if *actual.ty != Type::Scalar(expected.shape.scalar(component)) {
                        return Err(Error::Mismatch("emitted definition scalar type"));
                    }
                    definition.definitions[component] = Some(actual.coordinate);
                }
            }
            for edge in &mut capture.edges[function.edges.clone()] {
                budget.charge_work(2)?;
                let block = inventory
                    .block_for_id(actual.coordinate, edge.raw_source, budget)?
                    .ok_or(Error::Mismatch("emitted source block"))?;
                let [actual] = inventory
                    .edges()
                    .get(block.edges.clone())
                    .ok_or(Error::Mismatch("edge range"))?
                else {
                    return Err(Error::Mismatch("emitted Goto has one successor"));
                };
                if actual.target_id != edge.raw_target
                    || actual.arguments.get(edge.argument as usize) != Some(&edge.value)
                {
                    return Err(Error::Mismatch("emitted successor argument"));
                }
                edge.edge = Some(actual.coordinate);
            }
        }
        resources::sort_work(function_index.len(), budget)?;
        function_index.sort_unstable_by_key(|row| row.0);
        budget.charge_work(function_index.len())?;
        if function_index.windows(2).any(|rows| rows[0].0 == rows[1].0) {
            return Err(Error::Mismatch("duplicate emitted physical function"));
        }
        for source in original.correspondence.lowered_functions() {
            budget.charge_work(3)?;
            let actual = inventory
                .function_for_name(source.kernel_ir_function().as_str(), budget)?
                .ok_or(Error::Mismatch("correspondence physical function"))?;
            charge_lookup(function_index.len(), budget)?;
            let position = function_index
                .binary_search_by_key(&actual.coordinate, |row| row.0)
                .map_err(|_| Error::Mismatch("missing genuine function capture"))?;
            let emitted = function_index[position].1;
            if capture.functions[emitted].source != source.semantic_function() {
                return Err(Error::Mismatch("physical/source function binding"));
            }
            append(
                &mut aliases,
                Alias {
                    root: source.correspondence_owner(),
                    source: source.semantic_function(),
                    emitted,
                },
                budget,
            )?;
        }
        resources::sort_work(aliases.len(), budget)?;
        aliases.sort_unstable_by_key(|row| row.key());
        let mut statements = Vec::new();
        for (ordinal, span) in original
            .correspondence
            .statement_operation_spans()
            .iter()
            .enumerate()
        {
            append(
                &mut statements,
                StatementIndex {
                    key: (
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        span.semantic_block().index(),
                        span.statement_ordinal(),
                    ),
                    ordinal,
                },
                budget,
            )?;
        }
        resources::sort_work(statements.len(), budget)?;
        statements.sort_unstable_by_key(|row| row.key);
        let mut sites = Vec::new();
        for (expected, row) in capture.expected.iter().enumerate() {
            append(
                &mut sites,
                DefinitionSiteIndex {
                    key: row.site_key(),
                    expected,
                },
                budget,
            )?;
        }
        resources::sort_work(sites.len(), budget)?;
        sites.sort_unstable_by_key(|row| row.key);
        let mut first_operands = Vec::new();
        let occurrences = original
            .semantic_ssa()
            .occurrences_v1()
            .ok_or(Error::Mismatch("actual source occurrences"))?;
        for plan in original.semantic_ssa().plans() {
            budget.charge_work(1)?;
            let rows = occurrences
                .function(plan.function())
                .ok_or(Error::Mismatch("actual source function occurrences"))?;
            for (ordinal, row) in rows.events().iter().enumerate() {
                budget.charge_work(1)?;
                if let Some(key) = first_operand_key(plan.function(), row) {
                    append(
                        &mut first_operands,
                        FirstOperandIndex { key, ordinal },
                        budget,
                    )?;
                }
            }
        }
        resources::sort_work(first_operands.len(), budget)?;
        first_operands.sort_unstable_by_key(|row| row.key);
        let index_bytes = table_bytes::<(FunctionCoordinate, usize)>(function_index.capacity())?;
        drop(function_index);
        budget.release_storage(index_bytes)?;
        let retained = capture
            .capacity_bytes(budget)?
            .checked_add(table_bytes::<Alias>(aliases.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<StatementIndex>(statements.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<DefinitionSiteIndex>(sites.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<FirstOperandIndex>(first_operands.capacity())?)
            .ok_or(Resource::Arithmetic)?;
        let sealed = Self {
            capture,
            aliases,
            statements,
            sites,
            first_operands,
            retained,
        };
        sealed.replay(original, &inventory, budget)?;
        drop(inventory);
        budget.release_storage(receipt.retained_storage())?;
        Ok(sealed)
    }

    /// Independent complete coverage check. Source rows are enumerated from the
    /// actual occurrence/SSA owner, not from the producer's chosen record list.
    pub(super) fn replay(
        &self,
        original: &ProductionPreRankedKirOwnerV1,
        inventory: &Inventory<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(4)?;
        if !inventory.belongs_to(original.executable()) {
            return Err(Error::Mismatch("foreign inventory owner"));
        }
        self.check_source_coverage(original.semantic_ssa(), budget)?;
        if self
            .capture
            .capacity_bytes(budget)?
            .checked_add(table_bytes::<Alias>(self.aliases.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<StatementIndex>(self.statements.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<DefinitionSiteIndex>(self.sites.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<FirstOperandIndex>(
                self.first_operands.capacity(),
            )?)
            .ok_or(Resource::Arithmetic)?
            != self.retained
        {
            return Err(Error::Mismatch("attachment capacity receipt"));
        }
        budget.charge_work(self.aliases.len())?;
        if self
            .aliases
            .windows(2)
            .any(|rows| rows[0].key() >= rows[1].key())
        {
            return Err(Error::Mismatch("alias index order or duplicates"));
        }
        if self.aliases.len() != original.correspondence.lowered_functions().len() {
            return Err(Error::Mismatch("complete source alias roster"));
        }
        budget.charge_work(self.statements.len())?;
        if self.statements.len() != original.correspondence.statement_operation_spans().len()
            || self
                .statements
                .windows(2)
                .any(|rows| rows[0].key >= rows[1].key)
        {
            return Err(Error::Mismatch("statement index coverage"));
        }
        for row in &self.statements {
            budget.charge_work(1)?;
            let span = original
                .correspondence
                .statement_operation_spans()
                .get(row.ordinal)
                .ok_or(Error::Mismatch("statement index ordinal"))?;
            if row.key
                != (
                    span.correspondence_owner().index(),
                    span.semantic_function().index(),
                    span.semantic_block().index(),
                    span.statement_ordinal(),
                )
            {
                return Err(Error::Mismatch("statement index source occurrence"));
            }
        }
        budget.charge_work(self.sites.len())?;
        if self.sites.len() != self.capture.expected.len()
            || self.sites.windows(2).any(|rows| rows[0].key >= rows[1].key)
        {
            return Err(Error::Mismatch("definition site index coverage"));
        }
        for row in &self.sites {
            budget.charge_work(1)?;
            if self
                .capture
                .expected
                .get(row.expected)
                .map(|row| row.site_key())
                != Some(row.key)
            {
                return Err(Error::Mismatch("definition site index binding"));
            }
        }
        self.check_first_operand_index(original.semantic_ssa(), budget)?;
        let mut used = Vec::new();
        budget.charge_work(self.capture.functions.len())?;
        reserve(&mut used, self.capture.functions.len(), budget)?;
        used.resize(self.capture.functions.len(), false);
        let checked = (|| {
            #[cfg(test)]
            super::tests::after_replay_scratch_reserved_v1(budget, used.capacity())?;
            for source in original.correspondence.lowered_functions() {
                budget.charge_work(2)?;
                charge_lookup(self.aliases.len(), budget)?;
                let index = self
                    .aliases
                    .binary_search_by_key(
                        &(
                            source.correspondence_owner().index(),
                            source.semantic_function().index(),
                        ),
                        |row| row.key(),
                    )
                    .map_err(|_| Error::Mismatch("missing source alias"))?;
                let alias = self.aliases[index];
                let function = self
                    .capture
                    .functions
                    .get(alias.emitted)
                    .ok_or(Error::Mismatch("alias target"))?;
                let actual = inventory
                    .function_for_name(source.kernel_ir_function().as_str(), budget)?
                    .ok_or(Error::Mismatch("actual source function"))?;
                budget.charge_work(source.kernel_ir_function().as_str().len())?;
                if function.source != source.semantic_function()
                    || function.coordinate != Some(actual.coordinate)
                    || function.name != source.kernel_ir_function().as_str()
                {
                    return Err(Error::Mismatch("source alias custody"));
                }
                used[alias.emitted] = true;
            }
            budget.charge_work(used.len())?;
            if used.iter().any(|value| !*value) {
                return Err(Error::Mismatch("extra captured physical function"));
            }
            let mut definition_end = 0;
            let mut edge_end = 0;
            for function in &self.capture.functions {
                budget.charge_work(5)?;
                if function.definitions.start != definition_end
                    || function.edges.start != edge_end
                    || function.definitions.end < definition_end
                    || function.edges.end < edge_end
                    || function.definitions.end > self.capture.definitions.len()
                    || function.edges.end > self.capture.edges.len()
                {
                    return Err(Error::Mismatch("complete emission range partition"));
                }
                definition_end = function.definitions.end;
                edge_end = function.edges.end;
                charge_lookup(self.aliases.len(), budget)?;
                let alias = self
                    .aliases
                    .binary_search_by_key(
                        &(function.root.index(), function.source.index()),
                        |row| row.key(),
                    )
                    .map_err(|_| Error::Mismatch("actual emission root alias"))?;
                if !std::ptr::eq(
                    &self.capture.functions[self.aliases[alias].emitted],
                    function,
                ) {
                    return Err(Error::Mismatch("actual emission root owner"));
                }
                if function.unsupported_placement {
                    if !function.definitions.is_empty() || !function.edges.is_empty() {
                        return Err(Error::Mismatch("unsupported placement has claimed rows"));
                    }
                    continue;
                }
                self.check_function(original, inventory, function, budget)?;
            }
            if definition_end != self.capture.definitions.len()
                || edge_end != self.capture.edges.len()
            {
                return Err(Error::Mismatch("unused emission row tail"));
            }
            Ok(())
        })();
        let bytes = table_bytes::<bool>(used.capacity())?;
        drop(used);
        budget.release_storage(bytes)?;
        checked
    }

    fn check_source_coverage(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(self.capture.expected.len())?;
        if self
            .capture
            .expected
            .windows(2)
            .any(|rows| rows[0].key() >= rows[1].key())
        {
            return Err(Error::Mismatch("source definition index order"));
        }
        let occurrences = owner
            .occurrences_v1()
            .ok_or(Error::Mismatch("source occurrence attachment"))?;
        let source = owner.source_semantic();
        let mut count = 0usize;
        let mut check = |function: SemanticFunctionIdV1,
                         variable,
                         value,
                         site,
                         budget: &mut Budget<'_>|
         -> Result<()> {
            budget.charge_work(3)?;
            let declaration = source
                .functions()
                .get(function.index() as usize)
                .ok_or(Error::Mismatch("source function"))?;
            let Some(shape) = capture::source_shape(source.types(), declaration, variable, site)?
            else {
                return Ok(());
            };
            charge_lookup(self.capture.expected.len(), budget)?;
            let index = self
                .capture
                .expected
                .binary_search_by_key(&(function.index(), value), |row| row.key())
                .map_err(|_| Error::Mismatch("missing supported source row"))?;
            if self.capture.expected[index]
                != (Expected {
                    function,
                    value,
                    variable,
                    site,
                    shape,
                })
            {
                return Err(Error::Mismatch("changed source row"));
            }
            count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            Ok(())
        };
        for plan in owner.plans() {
            budget.charge_work(2)?;
            let function = plan.function();
            let rows = occurrences
                .function(function)
                .ok_or(Error::Mismatch("actual source occurrence function"))?;
            if !std::ptr::eq(rows.owner(), owner) {
                return Err(Error::Mismatch("foreign source occurrence owner"));
            }
            // Deliberately a distinct enumeration from Recorder::prepare.
            for row in rows.events() {
                budget.charge_work(1)?;
                if row.is_reachable()
                    && row.is_promoted()
                    && let Some(SsaResolvedEventV1::Define { variable, value }) = row.resolved()
                {
                    check(
                        function,
                        variable,
                        value,
                        SourceSite::Event(row.site()),
                        budget,
                    )?;
                }
            }
            for row in rows.edge_definitions() {
                budget.charge_work(1)?;
                if row.is_reachable() && row.is_promoted() {
                    check(
                        function,
                        row.variable(),
                        row.value()
                            .ok_or(Error::Mismatch("source edge resolution"))?,
                        SourceSite::Edge(row.edge()),
                        budget,
                    )?;
                }
            }
            for block in plan.plan().reverse_postorder() {
                budget.charge_work(1)?;
                for &variable in plan
                    .plan()
                    .transport_variables(*block)
                    .ok_or(Error::Mismatch("source transport roster"))?
                {
                    check(
                        function,
                        variable,
                        SsaValueV1::BlockArgument {
                            block: *block,
                            variable,
                        },
                        SourceSite::Header(block.get()),
                        budget,
                    )?;
                }
            }
            for row in rows.entry_definitions() {
                budget.charge_work(1)?;
                if let Some(value) = row.value() {
                    check(function, row.variable(), value, SourceSite::Entry, budget)?;
                }
            }
        }
        if count != self.capture.expected.len() {
            return Err(Error::Mismatch("extra supported source rows"));
        }
        Ok(())
    }

    fn check_function(
        &self,
        original: &ProductionPreRankedKirOwnerV1,
        inventory: &Inventory<'_>,
        function: &EmittedFunction,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let coordinate = function
            .coordinate
            .ok_or(Error::Mismatch("unsealed function"))?;
        let expected = self.capture.expected_range(function.source, budget)?;
        if expected.len() != function.definitions.len() {
            return Err(Error::Mismatch(
                "complete supported emission definition roster",
            ));
        }
        for (index, row) in expected.zip(&self.capture.definitions[function.definitions.clone()]) {
            budget.charge_work(3)?;
            if row.expected != index {
                return Err(Error::Mismatch("emission definition source key"));
            }
            let source = self.capture.expected[index];
            for component in 0..source.shape.width() {
                let actual = inventory
                    .definition_for_value(coordinate, row.values[component], budget)?
                    .ok_or(Error::Mismatch("missing captured definition"))?;
                if row.definitions[component] != Some(actual.coordinate)
                    || *actual.ty != Type::Scalar(source.shape.scalar(component))
                {
                    return Err(Error::Mismatch("captured typed definition"));
                }
            }
            if source.shape.width() == 1
                && (row.values[1] != ValueId(0) || row.definitions[1].is_some())
            {
                return Err(Error::Mismatch("unused scalar component"));
            }
            if let SourceSite::Header(block) = source.site {
                let actual_block = inventory
                    .block_for_id(coordinate, BlockId(block), budget)?
                    .ok_or(Error::Mismatch("actual header block"))?;
                if !matches!(row.definitions[0], Some(Definition::BlockArgument { block, .. }) if block == actual_block.coordinate)
                {
                    return Err(Error::Mismatch("header parameter binding"));
                }
            }
            if let Shape::CheckedAdd(_) = source.shape {
                check_checked_add(self, original, inventory, function, source, row, budget)?;
            }
        }
        check_edges(self, original, inventory, function, budget)
    }
}

impl Recorder {
    pub(super) fn capacity_bytes(&self, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(
            self.functions
                .len()
                .checked_add(4)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut bytes = table_bytes::<Expected>(self.expected.capacity())?
            .checked_add(table_bytes::<EmittedFunction>(self.functions.capacity())?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<EmittedDefinition>(
                self.definitions.capacity(),
            )?)
            .ok_or(Resource::Arithmetic)?
            .checked_add(table_bytes::<EmittedEdge>(self.edges.capacity())?)
            .ok_or(Resource::Arithmetic)?;
        for function in &self.functions {
            bytes = bytes
                .checked_add(function.name.capacity())
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(bytes)
    }
}

fn check_checked_add(
    sealed: &Sealed,
    original: &ProductionPreRankedKirOwnerV1,
    inventory: &Inventory<'_>,
    function: &EmittedFunction,
    source: Expected,
    row: &EmittedDefinition,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(5)?;
    let (
        Some(Definition::Result {
            operation,
            result: 0,
        }),
        Some(Definition::Result {
            operation: overflow,
            result: 1,
        }),
    ) = (row.definitions[0], row.definitions[1])
    else {
        return Err(Error::Mismatch("checked Add result fields"));
    };
    if operation != overflow {
        return Err(Error::Mismatch("checked Add field owner"));
    }
    let block = inventory
        .functions()
        .get(operation.block.function.0 as usize)
        .and_then(|function| {
            inventory
                .blocks()
                .get(function.blocks.start + operation.block.block as usize)
        })
        .ok_or(Error::Mismatch("checked Add operation block"))?;
    let actual = inventory
        .operations()
        .get(block.operations.start + operation.operation as usize)
        .filter(|row| row.coordinate == operation)
        .ok_or(Error::Mismatch("checked Add actual operation"))?;
    if !matches!(
        actual.operation.kind,
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            ..
        }
    ) {
        return Err(Error::Mismatch("checked Add opcode"));
    }
    let SourceSite::Event(Site::Statement {
        block: source_block,
        statement,
    }) = source.site
    else {
        return Err(Error::Mismatch("checked Add source statement"));
    };
    let first = sealed.statement(original, function, source_block.get(), statement, budget)?;
    let start = first.first_operation_ordinal();
    let count = first.operation_count();
    if first.kernel_ir_block() != block.block.id
        || operation.operation < start
        || operation.operation >= start.checked_add(count).ok_or(Resource::Arithmetic)?
    {
        return Err(Error::Mismatch("checked Add exact physical span"));
    }
    Ok(())
}

fn check_edges(
    sealed: &Sealed,
    original: &ProductionPreRankedKirOwnerV1,
    inventory: &Inventory<'_>,
    function: &EmittedFunction,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let owner = original.semantic_ssa();
    let plan = owner
        .plan_for_function(function.source)
        .ok_or(Error::Mismatch("source SSA function"))?
        .plan();
    let source = &owner.source_semantic().functions()[function.source.index() as usize];
    let rows = &sealed.capture.edges[function.edges.clone()];
    let mut next = 0;
    for (block, declaration) in source.blocks().iter().enumerate() {
        budget.charge_work(3)?;
        let block = SsaBlockIdV1::new(u32::try_from(block).map_err(|_| Resource::Arithmetic)?);
        if !plan.is_reachable(block) {
            continue;
        }
        let SemanticTerminatorKindV1::Goto(target) = declaration.terminator().kind() else {
            continue;
        };
        let edge = SsaEdgeIdV1::new(block, 0);
        for argument in plan
            .edge_arguments(edge)
            .ok_or(Error::Mismatch("source Goto argument roster"))?
        {
            budget.charge_work(3)?;
            let local = source
                .locals()
                .get(argument.variable().get() as usize)
                .ok_or(Error::Mismatch("source Goto local"))?;
            let Some(ty) = fixed_scalar(owner.source_semantic().types(), local.ty()) else {
                continue;
            };
            let row = rows
                .get(next)
                .ok_or(Error::Mismatch("missing source Goto emission row"))?;
            next += 1;
            if row.source != edge
                || row.target != target.target().index()
                || row.variable != argument.variable()
                || row.incoming != argument.value()
                || row.raw_source != BlockId(block.get())
                || row.raw_target != BlockId(target.target().index())
            {
                return Err(Error::Mismatch("source Goto occurrence binding"));
            }
            let coordinate = function
                .coordinate
                .ok_or(Error::Mismatch("unsealed edge function"))?;
            let actual_block = inventory
                .block_for_id(coordinate, row.raw_source, budget)?
                .ok_or(Error::Mismatch("actual Goto source"))?;
            let [actual] = inventory
                .edges()
                .get(actual_block.edges.clone())
                .ok_or(Error::Mismatch("actual Goto edges"))?
            else {
                return Err(Error::Mismatch("Goto edge count"));
            };
            if row.edge != Some(actual.coordinate)
                || actual.target_id != row.raw_target
                || actual.arguments.get(row.argument as usize) != Some(&row.value)
            {
                return Err(Error::Mismatch("actual Goto argument occurrence"));
            }
            let input = sealed.definition(function, row.incoming, budget)?;
            let header = sealed.definition(
                function,
                SsaValueV1::BlockArgument {
                    block: SsaBlockIdV1::new(row.target),
                    variable: row.variable,
                },
                budget,
            )?;
            let actual_argument = inventory
                .edge_arguments()
                .get(actual.bindings.start + row.argument as usize)
                .filter(|argument| argument.coordinate.edge == actual.coordinate)
                .ok_or(Error::Mismatch("actual edge argument binding"))?;
            if input.values[0] != row.value
                || input.definitions[0]
                    != Some(inventory.definitions()[actual_argument.incoming_definition].coordinate)
                || header.definitions[0]
                    != Some(inventory.definitions()[actual_argument.target_definition].coordinate)
                || *inventory.definitions()[actual_argument.target_definition].ty
                    != Type::Scalar(ty)
            {
                return Err(Error::Mismatch(
                    "source SSA and actual edge definition relation",
                ));
            }
        }
    }
    if next != rows.len() {
        return Err(Error::Mismatch("extra source Goto emission rows"));
    }
    Ok(())
}

impl Sealed {
    fn check_first_operand_index(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(self.first_operands.len())?;
        if self
            .first_operands
            .windows(2)
            .any(|rows| rows[0].key >= rows[1].key)
        {
            return Err(Error::Mismatch("first-operand index order"));
        }
        let occurrences = owner
            .occurrences_v1()
            .ok_or(Error::Mismatch("source occurrence index"))?;
        let mut count = 0usize;
        for plan in owner.plans() {
            budget.charge_work(1)?;
            let function = occurrences
                .function(plan.function())
                .ok_or(Error::Mismatch("source occurrence function index"))?;
            for (ordinal, row) in function.events().iter().enumerate() {
                budget.charge_work(1)?;
                let Some(key) = first_operand_key(plan.function(), row) else {
                    continue;
                };
                charge_lookup(self.first_operands.len(), budget)?;
                let index = self
                    .first_operands
                    .binary_search_by_key(&key, |row| row.key)
                    .map_err(|_| Error::Mismatch("missing actual first-operand occurrence"))?;
                if self.first_operands[index].ordinal != ordinal {
                    return Err(Error::Mismatch("changed first-operand occurrence"));
                }
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if count != self.first_operands.len() {
            return Err(Error::Mismatch("extra first-operand occurrences"));
        }
        Ok(())
    }

    pub(super) fn first_operand(
        &self,
        original: &ProductionPreRankedKirOwnerV1,
        function: SemanticFunctionIdV1,
        site: fe2o3_mir_model::SemanticU32InductionStatementSiteV1,
        variable: SsaVariableIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SsaValueV1> {
        charge_lookup(self.first_operands.len(), budget)?;
        let key = (
            function.index(),
            site.block().block().index(),
            site.statement(),
        );
        let index = self
            .first_operands
            .binary_search_by_key(&key, |row| row.key)
            .map_err(|_| Error::Mismatch("missing first operand for source certificate"))?;
        budget.charge_work(3)?;
        let occurrences = original
            .semantic_ssa()
            .occurrences_v1()
            .ok_or(Error::Mismatch("source operand occurrences"))?;
        let rows = occurrences
            .function(function)
            .ok_or(Error::Mismatch("source operand function"))?;
        let row = rows
            .events()
            .get(self.first_operands[index].ordinal)
            .ok_or(Error::Mismatch("source operand event"))?;
        match row.resolved() {
            Some(SsaResolvedEventV1::Use {
                variable: actual,
                value,
            }) if actual == variable => Ok(value),
            _ => Err(Error::Mismatch("source operand SSA resolution")),
        }
    }

    pub(super) fn statement<'a>(
        &self,
        original: &'a ProductionPreRankedKirOwnerV1,
        function: &EmittedFunction,
        block: u32,
        statement: u32,
        budget: &mut Budget<'_>,
    ) -> Result<&'a SemanticKirStatementOperationSpanV1> {
        charge_lookup(self.statements.len(), budget)?;
        let index = self
            .statements
            .binary_search_by_key(
                &(
                    function.root.index(),
                    function.source.index(),
                    block,
                    statement,
                ),
                |row| row.key,
            )
            .map_err(|_| Error::Mismatch("source statement index lookup"))?;
        original
            .correspondence
            .statement_operation_spans()
            .get(self.statements[index].ordinal)
            .ok_or(Error::Mismatch("source statement index target"))
    }

    pub(super) fn definition(
        &self,
        function: &EmittedFunction,
        value: SsaValueV1,
        budget: &mut Budget<'_>,
    ) -> Result<&EmittedDefinition> {
        charge_lookup(self.capture.expected.len(), budget)?;
        let expected = self
            .capture
            .expected
            .binary_search_by_key(&(function.source.index(), value), |row| row.key())
            .map_err(|_| Error::Mismatch("supported source SSA definition lookup"))?;
        let rows = &self.capture.definitions[function.definitions.clone()];
        charge_lookup(rows.len(), budget)?;
        let index = rows
            .binary_search_by_key(&expected, |row| row.expected)
            .map_err(|_| Error::Mismatch("captured SSA definition lookup"))?;
        Ok(&rows[index])
    }
}

fn first_operand_key(
    function: SemanticFunctionIdV1,
    row: &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1,
) -> Option<(u32, u32, u32)> {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let Site::Statement { block, statement } = row.site() else {
        return None;
    };
    (row.is_reachable()
        && row.is_promoted()
        && row.role() == Role::BaseUse
        && row.operand() == Operand::RvalueOperand(0))
    .then_some((function.index(), block.get(), statement))
}
