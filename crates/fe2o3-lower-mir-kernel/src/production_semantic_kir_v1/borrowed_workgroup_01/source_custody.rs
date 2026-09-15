mod borrow_route { include!("borrow_route.rs"); }
include!("borrow_transport.rs");

// The same source checks are borrowed by ranked use custody and KIR lowering.
// This layer does not select or materialize a KIR value.
pub(super) struct WorkgroupSourceResolverV1<'a, 'g> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) view: &'a SemanticExpandedRootV1,
    pub(super) context: &'g RootKernelContextLoweringV1,
    pub(super) graph: &'g mut Graph<'a>,
    pub(super) epochs: &'g [EpochOccurrence],
}

impl WorkgroupSourceResolverV1<'_, '_> {
    pub(super) fn execution_call(
        &self,
        block: u32,
    ) -> Result<(&SemanticDirectCallV1, SemanticExecutionCapabilityContractV1)> {
        checked_execution_source_call_v1(self.owner, self.view, self.context, block)
    }

    pub(super) fn resolve_operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        tail: &[SemanticProjectionV1],
        contract: SemanticExecutionCapabilityContractV1,
        depth: usize,
    ) -> Result<Origin> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(reject(
                "Workgroup authority cannot originate from a constant",
            ));
        };
        let mut projections = place.projections().to_vec();
        projections.extend_from_slice(tail);
        let value = self.graph.use_value(block, place.local().index())?;
        self.resolve(value, &projections, contract, depth + 1)
    }

    fn resolve(
        &mut self,
        value: SsaValueV1,
        projections: &[SemanticProjectionV1],
        contract: SemanticExecutionCapabilityContractV1,
        depth: usize,
    ) -> Result<Origin> {
        self.graph.charge(1)?;
        if depth > 128 || projections.len() > 16 {
            return Err(reject(
                "borrowed Workgroup source path is cyclic or too deep",
            ));
        }
        if let SsaValueV1::BlockArgument { block, variable } = value {
            let incoming = self.graph.incoming(block.get(), variable.get())?;
            let mut merged: Option<Origin> = None;
            for input in incoming {
                let candidate = self.resolve(input, projections, contract, depth + 1)?;
                if let Some(previous) = &mut merged {
                    if previous.issuer != candidate.issuer
                        || previous.contract != candidate.contract
                        || previous.epoch_projection != candidate.epoch_projection
                    {
                        return Err(reject(
                            "borrowed Workgroup has conflicting SSA edge issuers",
                        ));
                    }
                    self.graph.charge(candidate.loans.len())?;
                    previous.loans.extend(candidate.loans);
                } else {
                    merged = Some(candidate);
                }
            }
            return merged
                .ok_or_else(|| reject("borrowed Workgroup block argument has no incoming issuer"));
        }
        let site = self.graph.definition(value)?;
        if let Some(statement) = site.statement {
            let SemanticStatementKindV1::Assign(assignment) =
                self.graph.body.blocks()[site.block as usize].statements()[statement as usize]
                    .kind()
            else {
                return Err(reject(
                    "Workgroup SSA definition is not a retained assignment",
                ));
            };
            let assignment = assignment.clone();
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    self.resolve_operand(site.block, operand, projections, contract, depth)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    if !matches!(
                        aggregate.kind(),
                        SemanticAggregateKindV1::Tuple | SemanticAggregateKindV1::Aggregate
                    ) {
                        return Err(reject(
                            "Workgroup forwarding does not admit enum or array projections",
                        ));
                    }
                    let Some((first, rest)) = projections.split_first() else {
                        return Err(reject(
                            "aggregate construction cannot issue Workgroup authority",
                        ));
                    };
                    let SemanticProjectionKindV1::Field(field) = first.kind() else {
                        return Err(reject(
                            "Workgroup aggregate transport requires an exact source field",
                        ));
                    };
                    let operand = aggregate
                        .operands()
                        .get(field as usize)
                        .ok_or_else(|| reject("Workgroup aggregate field missing"))?;
                    if operand.ty() != first.result_type() {
                        return Err(reject("Workgroup aggregate field type changed"));
                    }
                    self.resolve_operand(site.block, operand, rest, contract, depth)
                }
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => {
                    let (workgroup_reference, workgroup) =
                        workgroup_reference_pair(self.owner.source_semantic().types(), contract)?;
                    let epoch = self
                        .epochs
                        .iter()
                        .find(|occurrence| {
                            occurrence.binding.expanded_entry_block().index() == site.block
                                && occurrence.binding.callee_return().index() == site.local
                        })
                        .cloned();
                    if epoch.is_none()
                        && (assignment.value().result_type() != workgroup_reference
                            || !matches!(projections, [] | [_])
                            || matches!(projections, [p] if p.kind() != SemanticProjectionKindV1::Dereference)
                            || !matches!(place.projections(), [] | [_])
                            || matches!(place.projections(), [p] if p.kind() != SemanticProjectionKindV1::Dereference))
                    {
                        return self.carrier_borrow(site, projections, contract, depth);
                    }
                    if !projections.is_empty()
                        && !matches!(projections, [p] if p.kind() == SemanticProjectionKindV1::Dereference)
                    {
                        return Err(reject(
                            "unsupported projection through a Workgroup shared borrow",
                        ));
                    }
                    let epoch_projection = epoch.is_some();
                    let source_place = if let Some(occurrence) = epoch {
                        let record = occurrence.projection;
                        if statement != 0
                            || record.types().workgroup != workgroup
                            || record.types().reference != workgroup_reference
                            || record.provenance() != contract.provenance()
                            || Some(record.brand()) != contract.workgroup_brand()
                            || Some(record.epoch()) != contract.epoch_before()
                            || assignment.value().result_type() != record.types().epoch_reference
                            || place.local() != occurrence.binding.callee_arguments()[0]
                            || place.projections() != record.projection()
                        {
                            return Err(reject(
                                "epoch projection lost its replay-checked defined-call binding",
                            ));
                        }
                        SemanticPlaceV1::new(place.local(), Vec::new(), workgroup_reference)
                            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    } else {
                        if assignment.value().result_type() != workgroup_reference
                            || place.ty() != workgroup
                            || !shared_type(
                                self.owner.source_semantic().types(),
                                workgroup_reference,
                                workgroup,
                            )
                            || !matches!(place.projections(), [] | [_])
                            || matches!(place.projections(), [p] if p.kind() != SemanticProjectionKindV1::Dereference)
                        {
                            return Err(reject(
                                "Workgroup borrow lacks its exact shared reference and referent",
                            ));
                        }
                        place.clone()
                    };
                    let referent = self
                        .graph
                        .use_value(site.block, source_place.local().index())?;
                    // The exact SSA use above supplies both recursion and the
                    // storage loan; do not query it again through a Copy wrapper.
                    let mut origin = self.resolve(
                        referent,
                        source_place.projections(),
                        contract,
                        depth + 1,
                    )?;
                    origin.epoch_projection |= epoch_projection;
                    // Callee reference slots may die after a return transfer;
                    // the shared loan is against the owned referent's storage.
                    if source_place.projections().is_empty() && source_place.ty() == workgroup {
                        origin.loans.push(Loan {
                            borrow: site,
                            owner_value: referent,
                            owner_local: source_place.local().index(),
                        });
                    }
                    Ok(origin)
                }
                _ => Err(reject("unsupported Workgroup reference definition")),
            }
        } else {
            if !projections.is_empty() {
                return Err(reject("Workgroup issuer result was projected"));
            }
            let (_, issuer) = self.execution_call(site.block)?;
            let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
                issuer.operation()
            else {
                return Err(reject(
                    "owned Workgroup must originate from its authenticated derive call",
                ));
            };
            let (_, expected) =
                workgroup_reference_pair(self.owner.source_semantic().types(), contract)?;
            if workgroup != expected
                || issuer.provenance() != contract.provenance()
                || issuer.workgroup_brand() != contract.workgroup_brand()
                || issuer.epoch_before() != contract.epoch_before()
                || issuer.epoch_after().is_some()
            {
                return Err(reject(
                    "owned Workgroup issuer brand, epoch or root changed",
                ));
            }
            Ok(Origin {
                issuer: value,
                contract: issuer,
                loans: Vec::new(),
                epoch_projection: false,
            })
        }
    }

    pub(super) fn check_live(&mut self, origin: &Origin, consumer: u32) -> Result<()> {
        for loan in &origin.loans {
            self.graph.loan_live(
                *loan,
                Site {
                    block: consumer,
                    statement: None,
                    local: 0,
                },
            )?;
        }
        let issuer = self.graph.definition(origin.issuer)?;
        for block in 0..self.graph.body.blocks().len() as u32 {
            self.graph.charge(1)?;
            if let Ok((_, transition)) = self.execution_call(block) {
                if transition.provenance() == origin.contract.provenance()
                    && transition.workgroup_brand() == origin.contract.workgroup_brand()
                    && transition.epoch_before() == origin.contract.epoch_before()
                    && transition.epoch_after().is_some()
                    && self.graph.reaches(issuer.block, block)?
                    && self.graph.reaches(block, consumer)?
                {
                    return Err(reject(
                        "borrowed Workgroup crosses a possible epoch transition",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn checked_epoch_occurrences_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    bindings: Vec<SemanticExpandedDefinedCapabilityV1>,
) -> Result<Vec<EpochOccurrence>> {
    let mut epochs = Vec::new();
    for binding in bindings {
        if binding.root() != view.root() {
            continue;
        }
        let source = owner
            .source_semantic()
            .functions()
            .get(binding.contract().function().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        // Other closed contracts remain in the unchanged source owner;
        // selecting epoch views never removes or interprets Math records.
        let Some(projection) = source.workgroup_epoch_projection().copied() else {
            continue;
        };
        if binding.contract()
            != SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(projection)
            || binding.arguments().len() != 1
            || binding.callee_arguments().len() != 1
            || binding.expansion_identity() != owner.execution_expansion().identity()
            || binding.root_identity() != view.identity()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        epochs.push(EpochOccurrence {
            projection,
            binding,
        });
    }

    Ok(epochs)
}

pub(super) fn checked_execution_source_call_v1<'a>(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    context: &RootKernelContextLoweringV1,
    block: u32,
) -> Result<(
    &'a SemanticDirectCallV1,
    SemanticExecutionCapabilityContractV1,
)> {
    let origin = view
        .block_origins()
        .get(block as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source {
        return Err(reject(
            "execution issuer lacks an original source call occurrence",
        ));
    }
    let SemanticTerminatorKindV1::Call(call) =
        view.body().blocks()[block as usize].terminator().kind()
    else {
        return Err(reject("Workgroup issuer is not a direct source call"));
    };
    let source = &owner.source_semantic().functions()[origin.function().index() as usize];
    let SemanticTerminatorKindV1::Call(original) = source.blocks()[origin.block().index() as usize]
        .terminator()
        .kind()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ..
    }) = owner
        .source_semantic()
        .callables()
        .get(call.callee().index() as usize)
    else {
        return Err(reject(
            "Workgroup issuer is not a retained execution intrinsic",
        ));
    };
    if original.callee() != call.callee()
        || binding.identity() != contract.source_identity()
        || !global_capability_provenance_matches_v1(context, contract.provenance())
        || call
            .arguments()
            .iter()
            .map(SemanticOperandV1::ty)
            .ne(contract.signature().arguments())
        || call.destination().map(|d| d.place().ty()) != Some(contract.signature().output())
    {
        return Err(reject(
            "Workgroup source call identity or root custody changed",
        ));
    }
    Ok((call, *contract))
}
