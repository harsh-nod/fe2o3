// Original SSA resolves forwarding; physical values never choose an expression.
use fe2o3_mir_model::{SsaResolvedEventV1 as EntryEventV20, SsaValueV1 as EntryValueV20};
use fe2o3_pliron::{
    ProductionSemanticSsaEntryOriginV1 as EntryOriginV20,
    ProductionSemanticSsaEventRoleV1 as EntryRoleV20,
    ProductionSemanticSsaOccurrenceSiteV1 as EntrySiteV20,
    ProductionSemanticSsaOperandRoleV1 as EntryOperandV20,
};

#[derive(Clone, Copy)]
enum OriginalEntryDefinitionV20 {
    Argument(u32),
    Assignment { block: u32, statement: u32 },
}

#[derive(Clone, Copy)]
struct OriginalEntryDefinitionRowV20 {
    key: [u32; 2],
    local: u32,
    origin: OriginalEntryDefinitionV20,
}

struct OriginalEntryIndexV20<'a, 'source> {
    source: &'a ProductionSourceCorrespondenceV18<'source>,
    definitions: Vec<OriginalEntryDefinitionRowV20>,
    required: usize,
}

fn original_entry_site_key_v20(site: EntrySiteV20) -> [u32; 2] {
    match site {
        EntrySiteV20::Statement { block, statement } => [block.get(), statement],
        EntrySiteV20::Terminator { block } => [block.get(), u32::MAX],
    }
}

impl<'a, 'source> OriginalEntryIndexV20<'a, 'source> {
    fn build(
        source: &'a ProductionSourceCorrespondenceV18<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        source.retain_query((|| {
            source.query(budget)?;
            let owner = source.source.source_ssa(budget)?;
            let occurrences =
                owner
                    .occurrences_v1()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "private entry SSA capture absent",
                    ))?;
            let count = occurrences.function_count();
            budget.charge_work(1)?;
            if count != owner.source_semantic().functions().len() {
                return source
                    .source
                    .missing("private entry source function census differs");
            }
            let mut capacity = 0usize;
            for ordinal in 0..count {
                budget.charge_work(3)?;
                let id = SemanticFunctionIdV1::from_index(
                    u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let rows =
                    occurrences
                        .function(id)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "private entry source function",
                        ))?;
                capacity = argument_sum_v1(&[
                    capacity,
                    rows.events().len(),
                    rows.entry_definitions().len(),
                ])?;
            }
            let mut definitions =
                emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
            for ordinal in 0..count {
                budget.charge_work(2)?;
                let id = SemanticFunctionIdV1::from_index(ordinal as u32);
                let rows =
                    occurrences
                        .function(id)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "private entry source function",
                        ))?;
                for entry in rows.entry_definitions() {
                    budget.charge_work(3)?;
                    if let (
                        EntryOriginV20::Argument(argument),
                        Some(EntryValueV20::Definition(value)),
                    ) = (entry.origin(), entry.value())
                    {
                        definitions.push(OriginalEntryDefinitionRowV20 {
                            key: [id.index(), value.get()],
                            local: entry.variable().get(),
                            origin: OriginalEntryDefinitionV20::Argument(argument),
                        });
                    }
                }
                let mut previous = None;
                for event in rows.events() {
                    budget.charge_work(5)?;
                    let site = original_entry_site_key_v20(event.site());
                    if previous.is_some_and(|old| old > site) {
                        return source
                            .source
                            .missing("private entry occurrence order differs");
                    }
                    previous = Some(site);
                    if let (
                        EntrySiteV20::Statement { block, statement },
                        EntryOperandV20::Destination,
                        EntryRoleV20::DestinationDefine,
                        Some(EntryEventV20::Define {
                            variable,
                            value: EntryValueV20::Definition(value),
                        }),
                    ) = (
                        event.site(),
                        event.operand(),
                        event.role(),
                        event.resolved(),
                    ) {
                        if !event.is_promoted() || !event.is_reachable() {
                            return source
                                .source
                                .missing("private entry definition is not active SSA");
                        }
                        definitions.push(OriginalEntryDefinitionRowV20 {
                            key: [id.index(), value.get()],
                            local: variable.get(),
                            origin: OriginalEntryDefinitionV20::Assignment {
                                block: block.get(),
                                statement,
                            },
                        });
                    }
                }
            }
            private_array_heapsort_v1(
                &mut definitions,
                |row| row.key,
                &mut SourceCorrespondenceWorkV18(budget),
                || ArgumentResourceV1::Arithmetic.into(),
            )?;
            for pair in definitions.windows(2) {
                budget.charge_work(1)?;
                if pair[0].key == pair[1].key {
                    return source.source.missing("private entry definition repeated");
                }
            }
            Ok(Self {
                source,
                definitions,
                required: budget.storage(),
            })
        })())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.source.retain_query((|| {
            self.source.query(budget)?;
            if budget.storage() < self.required {
                self.source.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(())
        })())
    }

    fn definition(
        &self,
        function: SemanticFunctionIdV1,
        value: EntryValueV20,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<OriginalEntryDefinitionRowV20> {
        let EntryValueV20::Definition(value) = value else {
            return self
                .source
                .source
                .missing("private entry requires unsupported SSA block argument");
        };
        let key = [function.index(), value.get()];
        let (mut lo, mut hi) = (0, self.definitions.len());
        while lo < hi {
            budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            match self.definitions[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => return Ok(self.definitions[middle]),
            }
        }
        self.source
            .source
            .missing("private entry SSA definition is not a supported source origin")
    }

    fn promoted_use(
        &self,
        function: SemanticFunctionIdV1,
        site: EntrySiteV20,
        role: EntryOperandV20,
        local: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<EntryValueV20> {
        let owner = self.source.source.source_ssa(budget)?;
        let occurrences =
            owner
                .occurrences_v1()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private entry SSA capture absent",
                ))?;
        let rows =
            occurrences
                .function(function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private entry source function",
                ))?;
        let events = rows.events();
        let key = original_entry_site_key_v20(site);
        let (mut lo, mut hi) = (0, events.len());
        while lo < hi {
            budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if original_entry_site_key_v20(events[middle].site()) < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        let mut selected = None;
        for event in &events[lo..] {
            budget.charge_work(4)?;
            if original_entry_site_key_v20(event.site()) != key {
                break;
            }
            if event.operand() != role || event.role() != EntryRoleV20::BaseUse {
                continue;
            }
            let Some(EntryEventV20::Use { variable, value }) = event.resolved() else {
                return self.source.source.missing(
                    "private entry operand is not a captured scalar read or promoted use",
                );
            };
            if variable.get() != local
                || !event.is_promoted()
                || !event.is_reachable()
                || selected.replace(value).is_some()
            {
                return self
                    .source
                    .source
                    .missing("private entry use occurrence differs");
            }
        }
        selected.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private entry use occurrence absent",
        ))
    }

    fn expression(
        &self,
        leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
        request: &ProductionSourceEntryWriteV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.source.retain_query((|| {
            self.check(budget)?;
            let original = leaves.original_leaves(budget)?;
            budget.charge_work(2)?;
            if std::ptr::from_ref(original.leaves.relation).cast::<()>()
                != std::ptr::from_ref(self.source).cast::<()>()
                || std::ptr::from_ref(request.leaves).cast::<()>()
                    != std::ptr::from_ref(leaves).cast::<()>()
            {
                return self.source.source.missing("private entry substituted its original leaf context");
            }
            let (mut instance, function) = request.original(budget)?;
            let ProductionSourceScalarInputV18::EntryArgument { argument } = request.input_for(function, budget)? else {
                return self.source.source.missing("private entry input is not a source argument");
            };
            let ty = request.row.ty;
            let scalar = request.scalar(budget)?;
            let root = original.leaves.root;
            let instances = self.source.source.instance_count(root, budget)?;
            // A single-operand chain visits each SSA definition at most once per
            // invocation; caller traversal strictly decreases the instance.
            let limit = argument_product_v1(
                argument_sum_v1(&[self.definitions.len(), 2])?, argument_sum_v1(&[instances, 1])?,
            )?;
            let mut state = OriginalEntryStateV20::Argument(argument);
            for _ in 0..limit {
                budget.charge_work(7)?;
                let function = original.original_function(instance, budget)?;
                let (function_id, _) = self.source.source.instance(root, instance, budget)?;
                match state {
                    OriginalEntryStateV20::Argument(argument) => {
                        match original.original_argument(instance, function, argument, budget)? {
                            ProductionSourceScalarArgumentV18::Root { argument } => {
                                let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(argument)
                                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                                return Ok(ProductionSemanticExpressionV2::Symbol { symbol, scalar });
                            }
                            ProductionSourceScalarArgumentV18::Caller { instance: caller, function: declaration, block, operand } => {
                                if caller >= instance || !std::ptr::eq(original.original_function(caller, budget)?, declaration) {
                                    return self.source.source.missing("private entry caller instance does not precede callee");
                                }
                                instance = caller;
                                state = OriginalEntryStateV20::Operand {
                                    site: EntrySiteV20::Terminator { block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()) },
                                    role: EntryOperandV20::CallArgument(argument), operand,
                                };
                            }
                        }
                    }
                    OriginalEntryStateV20::Operand { site, role, operand } => {
                        if semantic_operand_type(operand) != ty {
                            return self.source.source.missing("private entry forwarding changes source type");
                        }
                        let place = match operand {
                            SemanticOperandV1::Constant(constant) => {
                                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                                    return self.source.source.missing("private entry constant is not scalar");
                                };
                                return Ok(ProductionSemanticExpressionV2::Constant {
                                    scalar, bits: u64::try_from(value.bits()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                });
                            }
                            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
                        };
                        if let Some(expression) = original.original_place(instance, function, place, budget)? {
                            if expression.scalar() != scalar {
                                return self.source.source.missing("private entry captured read type differs");
                            }
                            return Ok(expression);
                        }
                        if !place.projections().is_empty() {
                            return self.source.source.missing("private entry uncaptured projection is unsupported");
                        }
                        let value = self.promoted_use(function_id, site, role, place.local().index(), budget)?;
                        let definition = self.definition(function_id, value, budget)?;
                        if definition.local != place.local().index() {
                            return self.source.source.missing("private entry SSA source local differs");
                        }
                        match definition.origin {
                            OriginalEntryDefinitionV20::Argument(argument) => state = OriginalEntryStateV20::Argument(argument),
                            OriginalEntryDefinitionV20::Assignment { block, statement } => {
                                let Some(SemanticStatementKindV1::Assign(assignment)) = function.blocks()
                                    .get(block as usize).and_then(|b| b.statements().get(statement as usize)).map(|s| s.kind())
                                else { return self.source.source.missing("private entry original assignment absent"); };
                                if assignment.destination().local().index() != definition.local
                                    || !assignment.destination().projections().is_empty()
                                    || assignment.destination().ty() != ty
                                { return self.source.source.missing("private entry assignment destination differs"); }
                                let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                                    return self.source.source.missing("private entry original expression requires unsupported derivation");
                                };
                                state = OriginalEntryStateV20::Operand {
                                    site: EntrySiteV20::Statement { block: fe2o3_mir_model::SsaBlockIdV1::new(block), statement },
                                    role: EntryOperandV20::RvalueOperand(0), operand,
                                };
                            }
                        }
                    }
                }
            }
            self.source.source.missing("private entry forwarding exceeded its original source census")
        })())
    }
}

enum OriginalEntryStateV20<'a> {
    Argument(u32),
    Operand {
        site: EntrySiteV20,
        role: EntryOperandV20,
        operand: &'a SemanticOperandV1,
    },
}
