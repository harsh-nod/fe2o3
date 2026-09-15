//! Inert operation descriptions selected only from the consumed source roster.
//! No descriptor creates an SSA value or substitutes for a live KIR operand.
use super::calls::{Call, CallKind, mapped_block, selected_call};
use super::*;
use fe2o3_kernel_ir::{
    PhaseDefinedCallV1, PhaseKeyV1, PhaseLifetimeEndV1, PhaseOperationSourceV1,
    PhaseSourcePositionV1, ReusablePhaseOperationV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedReusablePhaseV1, SemanticPhaseCallableV1, SemanticPhaseIncomingCommitmentV1,
    canonical_semantic_source_body_sha256_v25,
};

pub(super) struct Description {
    pub(super) source: PhaseOperationSourceV1,
    pub(super) operation: ReusablePhaseOperationV1,
    pub(super) provenance: SemanticKernelCapabilityProvenanceV1,
}

pub(super) struct Descriptions {
    rows: Vec<Description>,
    finish_epochs: Vec<FinishEpoch>,
}

impl Descriptions {
    pub(super) fn new(checked: &CheckedRows<'_>, work: &mut usize) -> PhaseResult<Self> {
        let mut phases = reserve(checked.input.phases.len(), work)?;
        for phase in &checked.input.phases {
            phases.push(PhaseSources::new(checked, phase, work)?);
        }
        let mut rows = reserve(checked.input.rows.len(), work)?;
        for row in &checked.input.rows {
            spend(work, 1)?;
            let phase = &checked.input.phases[row.phase];
            rows.push(phases[row.phase].description(checked, phase, *row, work)?);
        }
        spend(work, std::mem::size_of::<Vec<FinishEpoch>>())?;
        let mut finish_epochs = reserve(phases.len(), work)?;
        for phase in phases {
            spend(work, 1)?;
            finish_epochs.push(phase.finish_epoch);
        }
        Ok(Self { rows, finish_epochs })
    }

    pub(super) fn finish_at(
        &self,
        block: SemanticBlockIdV1,
        work: &mut usize,
    ) -> PhaseResult<Option<(usize, FinishEpoch)>> {
        let mut found = None;
        for (index, epoch) in self.finish_epochs.iter().enumerate() {
            spend(work, 1)?;
            if epoch.block() == block.index() {
                spend(work, std::mem::size_of::<FinishEpoch>())?;
                if found.replace((index, *epoch)).is_some() {
                    return Err(rejected("phase Finish epoch occurrence is ambiguous"));
                }
            }
        }
        Ok(found)
    }

    pub(super) fn get(&self, index: usize) -> PhaseResult<&Description> {
        self.rows
            .get(index)
            .ok_or_else(|| rejected("phase operation is outside its consumed roster"))
    }
}

struct PhaseSources {
    owner: PhaseDefinedCallV1,
    wrapper: PhaseDefinedCallV1,
    issue: PhaseDefinedCallV1,
    invoke: PhaseDefinedCallV1,
    finish: PhaseDefinedCallV1,
    barrier: Call,
    drop: Call,
    drop_abi: [u8; 32],
    key: PhaseKeyV1,
    finish_epoch: FinishEpoch,
    provenance: SemanticKernelCapabilityProvenanceV1,
    closure_return_block: u32,
}

impl PhaseSources {
    fn new(
        checked: &CheckedRows<'_>,
        phase: &PhaseOccurrenceEmissionInputV1,
        work: &mut usize,
    ) -> PhaseResult<Self> {
        let (owner_record, owner) = defined(checked, phase.owner, work)?;
        let (wrapper_record, wrapper) = defined(checked, phase.wrapper, work)?;
        let (issue_record, issue_call) = defined(checked, phase.issue, work)?;
        let (finish_record, finish) = defined(checked, phase.finish, work)?;
        let Recipe::WithPhase {
            issue,
            invoke,
            drop_completion,
            issue_block,
            invoke_block,
            drop_block,
            relay,
            ..
        } = wrapper_record.recipe()
        else {
            return Err(rejected("phase wrapper changed its exact source recipe"));
        };
        let Recipe::Finish {
            barrier,
            barrier_block,
            return_block,
            ..
        } = finish_record.recipe()
        else {
            return Err(rejected("phase Finish changed its exact source recipe"));
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: barrier_contract },
            ..
        }) = checked.owner.source_semantic().callables().get(barrier.callable.index() as usize)
        else {
            return Err(rejected("phase Finish lost its original barrier contract"));
        };
        let provenance = owner_record.provenance();
        if [
            wrapper_record.provenance(),
            issue_record.provenance(),
            finish_record.provenance(),
        ]
        .iter()
        .any(|actual| *actual != provenance)
        {
            return Err(rejected("phase operation changed its source provenance"));
        }
        let selected_issue = selected_call(checked, phase.wrapper, issue_block, issue, work)?;
        if selected_issue.defined()? != issue_call.call
            || selected_issue.signature != issue_call.signature
        {
            return Err(rejected(
                "wrapper Issue no longer selects the consumed occurrence",
            ));
        }
        let selected_invoke = selected_call(checked, phase.wrapper, invoke_block, invoke, work)?;
        if selected_invoke.kind != CallKind::Expanded(phase.closure) {
            return Err(rejected(
                "wrapper invoke changed its actual closure instance",
            ));
        }
        let invoke = closure(
            checked,
            phase.closure,
            selected_invoke,
            relay.closure_function,
            relay.closure_body,
            work,
        )?;
        let barrier = selected_call(checked, phase.finish, barrier_block, barrier, work)?;
        // Finish's barrier is a retained intrinsic, never a made-up child frame.
        barrier.terminal()?;
        let view = checked
            .owner
            .execution_view_for_root(checked.input.root)
            .unwrap();
        if barrier.expanded_normal_target != mapped_block(view, phase.finish, return_block, work)? {
            return Err(rejected(
                "Finish barrier no longer returns to its checked result site",
            ));
        }
        let drop = selected_call(checked, phase.wrapper, drop_block, drop_completion, work)?;
        let closure_return_block =
            mapped_block(view, phase.closure, relay.closure_return_block, work)?.index();
        let key = PhaseKeyV1::for_begin(issue_call.call)
            .ok_or_else(|| rejected("phase key has no complete checked Begin occurrence"))?;
        spend(work, 1)?;
        let finish_epoch = FinishEpoch::new(key, barrier.terminal()?, *barrier_contract)?;
        Ok(Self {
            owner,
            wrapper,
            issue: issue_call,
            invoke,
            finish,
            barrier,
            drop,
            drop_abi: *drop_completion.abi.as_bytes(),
            key,
            finish_epoch,
            provenance,
            closure_return_block,
        })
    }

    fn description(
        &self,
        checked: &CheckedRows<'_>,
        phase: &PhaseOccurrenceEmissionInputV1,
        row: PhaseEmissionRowV1,
        work: &mut usize,
    ) -> PhaseResult<Description> {
        use PhaseEmissionActionV1 as Action;
        use PhaseOperationSourceV1 as Source;
        use ReusablePhaseOperationV1 as Op;
        let types = checked.owner.source_semantic().types();
        let ty = |id| execution_type_identity_v1(types, id);
        let marker = |id| -> PhaseResult<[u8; 32]> { Ok(ty(id)?.bytes()) };
        let protocol = checked.input.source_protocol;
        let (operation, source) = match row.action {
            Action::OwnerConvert => {
                let record = record(checked, phase.owner, work)?;
                let Recipe::OwnerConvert {
                    workgroup, owner, ..
                } = record.recipe()
                else {
                    return Err(rejected("phase OwnerConvert recipe changed"));
                };
                (
                    Op::OwnerConvert {
                        workgroup: ty(workgroup)?,
                        owner: ty(owner)?,
                    },
                    Source::Defined(self.owner),
                )
            }
            Action::Begin => {
                let record = record(checked, phase.issue, work)?;
                let Recipe::Issue {
                    owner_reference,
                    owner,
                    phase_workgroup,
                    brands,
                } = record.recipe()
                else {
                    return Err(rejected("phase Issue recipe changed"));
                };
                (
                    Op::Begin {
                        owner_reference: ty(owner_reference.reference)?,
                        owner: ty(owner)?,
                        phase_workgroup: ty(phase_workgroup)?,
                        // The execution brand is the owner's exact B (possibly a
                        // phase), not its WorkgroupBrand<B> source-layout phantom.
                        outer_brand: marker(brands.root_brand)?,
                        phase_brand: marker(brands.phase_brand)?,
                        dynamic_epoch: super::epochs::scoped_epoch(self.key, marker(brands.dynamic_epoch)?)?,
                        wrapper: self.wrapper,
                        invoke: self.invoke,
                        source_protocol: protocol,
                    },
                    Source::Defined(self.issue),
                )
            }
            Action::Bind { lease } => {
                let (record, call) = defined(checked, phase.leases[lease].bind, work)?;
                if record.provenance() != self.provenance {
                    return Err(rejected("phase Bind changed its exact source provenance"));
                }
                let Recipe::Bind {
                    phase_reference,
                    storage_reference,
                    phase_workgroup,
                    reusable_storage,
                    phase_lds,
                    element,
                    elements,
                    ..
                } = record.recipe()
                else {
                    return Err(rejected("phase Bind recipe changed"));
                };
                (
                    Op::Bind {
                        phase_reference: ty(phase_reference.reference)?,
                        storage_reference: ty(storage_reference.reference)?,
                        phase_workgroup: ty(phase_workgroup)?,
                        reusable_storage: ty(reusable_storage)?,
                        phase_lds: ty(phase_lds)?,
                        element: ty(element)?,
                        layout: execution_element_layout_v1(types, element)?,
                        elements,
                    },
                    Source::Defined(call),
                )
            }
            Action::Seal => {
                let record = record(checked, phase.finish, work)?;
                let Recipe::Finish {
                    workgroup_before_barrier,
                    workgroup_after_barrier,
                    completion,
                    ..
                } = record.recipe()
                else {
                    return Err(rejected("phase Finish recipe changed"));
                };
                if row.boundary.site.block().index() != self.barrier.expanded_normal_target.index()
                {
                    return Err(rejected(
                        "phase Seal is not on the actual barrier normal edge",
                    ));
                }
                (
                    Op::Seal {
                        workgroup_before_barrier: ty(workgroup_before_barrier)?,
                        workgroup_after_barrier: ty(workgroup_after_barrier)?,
                        completion: ty(completion)?,
                        barrier_call: self.barrier.terminal()?,
                    },
                    Source::Defined(self.finish),
                )
            }
            Action::RelayClosure | Action::RelayDrop => {
                let record = record(checked, phase.wrapper, work)?;
                let Recipe::WithPhase { completion, .. } = record.recipe() else {
                    return Err(rejected("phase relay wrapper recipe changed"));
                };
                if row.action == Action::RelayClosure {
                    (
                        Op::RelayClosure {
                            completion: ty(completion)?,
                        },
                        Source::ClosureReturn {
                            phase: self.key,
                            closure: self.invoke,
                            pack_block: row.boundary.site.block().index(),
                            pack_statement: row
                                .boundary
                                .site
                                .statement()
                                .ok_or_else(|| rejected("phase pack is not a statement"))?,
                            return_block: self.closure_return_block,
                            source_protocol: protocol,
                        },
                    )
                } else {
                    if self.drop.source.occurrence.is_none_or(|origin| {
                        origin.expanded_block() != row.boundary.site.block().index()
                    }) {
                        return Err(rejected("phase Drop is not at its actual source call"));
                    }
                    (
                        Op::RelayDrop {
                            completion: ty(completion)?,
                        },
                        Source::WrapperDrop {
                            phase: self.key,
                            drop_call: self.drop.drop_occurrence()?,
                            drop_abi: self.drop_abi,
                            source_protocol: protocol,
                        },
                    )
                }
            }
            Action::CloseStorage { lease } => {
                let (record, call) = defined(checked, phase.leases[lease].bind, work)?;
                let Recipe::Bind { phase_lds, .. } = record.recipe() else {
                    return Err(rejected("phase storage close changed its original Bind"));
                };
                (
                    Op::CloseStorage {
                        last_lease: ty(phase_lds)?,
                    },
                    Source::LeaseEnd {
                        phase: self.key,
                        bind: call.call,
                        source_event: lifetime_end(checked, row, work)?,
                        source_protocol: protocol,
                    },
                )
            }
            Action::End => (
                Op::End {
                    storage_count: u8::try_from(phase.leases.len()).map_err(|_| {
                        rejected("phase storage roster is outside the closed End arity")
                    })?,
                },
                Source::WrapperEnd {
                    phase: self.key,
                    wrapper_normal_target: self.wrapper.call.expanded_normal_target,
                    source_protocol: protocol,
                },
            ),
        };
        Ok(Description {
            source,
            operation,
            provenance: self.provenance,
        })
    }
}

fn record(
    checked: &CheckedRows<'_>,
    instance: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PhaseResult<SemanticDefinedReusablePhaseV1> {
    match checked.binding(instance, work)?.contract() {
        Defined::ReusablePhase(record) => Ok(record),
        _ => Err(rejected(
            "phase source descriptor selected another defined family",
        )),
    }
}

fn defined(
    checked: &CheckedRows<'_>,
    instance: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PhaseResult<(SemanticDefinedReusablePhaseV1, PhaseDefinedCallV1)> {
    let binding = checked.binding(instance, work)?;
    let record = record(checked, instance, work)?;
    let source = checked.owner.source_semantic();
    let caller = &source.functions()[binding.caller_function().index() as usize];
    let SemanticTerminatorKindV1::Call(original) = caller.blocks()
        [binding.call_block().index() as usize]
        .terminator()
        .kind()
    else {
        return Err(rejected("defined phase call is absent"));
    };
    let call = selected_call(
        checked,
        binding.caller_instance(),
        binding.call_block(),
        SemanticPhaseCallableV1 {
            callable: original.callee(),
            identity: record.source_identity(),
            abi: record.abi_identity(),
        },
        work,
    )?;
    if call.kind != CallKind::Expanded(instance)
        || call
            .source
            .occurrence
            .is_none_or(|o| o.expanded_block() != binding.expanded_call_block().index())
    {
        return Err(rejected(
            "phase descriptor substituted its checked defined occurrence",
        ));
    }
    let result = PhaseDefinedCallV1 {
        call: call.defined()?,
        signature: call.signature,
        defined_abi: *record.abi_identity().as_bytes(),
        defined_body: *record.body_identity(),
        incoming_count: record.incoming().count(),
        incoming_digest: *record.incoming().digest(),
        source_binding: *record.source_binding(),
    };
    if !result.is_complete() {
        return Err(rejected("phase defined descriptor is incomplete"));
    }
    Ok((record, result))
}

fn closure(
    checked: &CheckedRows<'_>,
    instance: SemanticCallInstanceIdV1,
    call: Call,
    expected_function: SemanticFunctionIdV1,
    expected_body: [u8; 32],
    work: &mut usize,
) -> PhaseResult<PhaseDefinedCallV1> {
    let semantic = checked.owner.source_semantic();
    let view = checked
        .owner
        .execution_view_for_root(checked.input.root)
        .unwrap();
    let frame = view
        .instances()
        .get(instance.index() as usize)
        .ok_or_else(|| rejected("phase closure frame is absent"))?;
    if call.kind != CallKind::Expanded(instance) || frame.function() != expected_function {
        return Err(rejected(
            "phase closure descriptor selected a foreign frame",
        ));
    }
    let body = &semantic.functions()[frame.function().index() as usize];
    let limit =
        u64::try_from(*work / 2).map_err(|_| rejected("phase body work cannot be represented"))?;
    let (digest, bytes) = canonical_semantic_source_body_sha256_v25(body, limit)
        .map_err(|_| rejected("phase closure source body reconstruction failed"))?;
    spend(
        work,
        bytes
            .checked_mul(2)
            .ok_or_else(|| rejected("phase closure body debit overflow"))?,
    )?;
    if digest != expected_body || frame.function_identity() != body.identity() {
        return Err(rejected("phase closure body changed after source checking"));
    }
    // Reuse MIR's canonical complete incoming roster, with the same counter.
    // This query is inert metadata, not a source-use or source-authority API.
    let mut remaining =
        u64::try_from(*work).map_err(|_| rejected("phase incoming work cannot be represented"))?;
    let incoming = SemanticPhaseIncomingCommitmentV1::reconstruct_for_defined_function(
        frame.function(),
        semantic.functions(),
        semantic.callables(),
        &mut remaining,
    );
    *work = usize::try_from(remaining)
        .map_err(|_| rejected("phase incoming remaining work cannot be represented"))?;
    let incoming =
        incoming.map_err(|_| rejected("phase closure incoming roster reconstruction failed"))?;
    let result = PhaseDefinedCallV1 {
        call: call.defined()?,
        signature: call.signature,
        defined_abi: *body.abi().identity().as_bytes(),
        defined_body: digest,
        incoming_count: incoming.count(),
        incoming_digest: *incoming.digest(),
        source_binding: checked.input.source_protocol,
    };
    if !result.is_complete() {
        return Err(rejected("phase closure descriptor is incomplete"));
    }
    Ok(result)
}

fn lifetime_end(
    checked: &CheckedRows<'_>,
    row: PhaseEmissionRowV1,
    work: &mut usize,
) -> PhaseResult<PhaseLifetimeEndV1> {
    spend(work, 1)?;
    let view = checked
        .owner
        .execution_view_for_root(checked.input.root)
        .unwrap();
    let site = row.boundary.site;
    let origin = view
        .block_origins()
        .get(site.block().index() as usize)
        .ok_or_else(|| rejected("phase lifetime site is absent"))?;
    let frame = &view.instances()[origin.instance().index() as usize];
    if frame.function() != origin.function() {
        return Err(rejected("phase lifetime frame changed"));
    }
    let original_position = match site.statement() {
        Some(statement) => match origin.statements().get(statement as usize) {
            Some(fe2o3_mir_model::SemanticExpandedStatementOriginV1::Source { statement }) => {
                PhaseSourcePositionV1::Statement(*statement)
            }
            Some(fe2o3_mir_model::SemanticExpandedStatementOriginV1::FrameStorageDead {
                callee,
                local,
            }) => {
                let source = checked.owner.source_semantic();
                let local_origin = view
                    .local_origins()
                    .get(row.boundary.variable.get() as usize)
                    .ok_or_else(|| rejected("phase lifetime local has no original frame"))?;
                if *callee != origin.instance()
                    || local_origin.instance() != *callee
                    || local_origin.function() != origin.function()
                    || local_origin.local() != *local
                    || origin.terminator()
                        != (fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::CallReturn {
                            callee: *callee,
                        })
                    || !matches!(
                        source.functions()[origin.function().index() as usize].blocks()
                            [origin.block().index() as usize]
                            .terminator()
                            .kind(),
                        SemanticTerminatorKindV1::Return
                    )
                    || !matches!(view.body().blocks()[site.block().index() as usize]
                        .statements()[statement as usize].kind(), SemanticStatementKindV1::StorageDead(actual)
                            if actual.index() == row.boundary.variable.get())
                {
                    return Err(rejected(
                        "phase lifetime event changed its checked normal-return frame death",
                    ));
                }
                // The expander inserts this death at the original Return.
                // Keep that terminator and the distinct expanded statement.
                PhaseSourcePositionV1::Terminator
            }
            _ => {
                return Err(rejected(
                    "phase lease close requires an original source lifetime event",
                ));
            }
        },
        None if origin.terminator()
            == fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source =>
        {
            PhaseSourcePositionV1::Terminator
        }
        None => {
            return Err(rejected(
                "phase lease close is not an original source terminator",
            ));
        }
    };
    Ok(PhaseLifetimeEndV1 {
        function: *frame.function_identity().as_bytes(),
        instance: origin.instance().index(),
        original_block: origin.block().index(),
        original_position,
        expanded_block: site.block().index(),
        expanded_position: site.statement().map_or(
            PhaseSourcePositionV1::Terminator,
            PhaseSourcePositionV1::Statement,
        ),
    })
}
