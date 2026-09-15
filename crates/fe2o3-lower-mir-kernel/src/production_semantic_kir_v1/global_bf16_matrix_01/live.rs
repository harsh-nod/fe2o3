use super::*;
use capability_ssa_graph_01::{CapabilityDefinitionSiteV1, CapabilityLoanV1, CapabilitySsaGraphV1};

mod guarded_result {
    include!("guarded_result.rs");
}

mod borrow_failure_observation {
    use super::*;
    include!("borrow_failure_observation.rs");
}

fn reject(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ResolveTarget {
    Global,
    MatrixConstruction,
}

struct Resolver<'graph, 'a> {
    graph: &'graph mut CapabilitySsaGraphV1<'a>,
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    active: BTreeSet<SsaValueV1>,
    consumer: CapabilityDefinitionSiteV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
    target: ResolveTarget,
    observation_source: Option<(&'a ProductionSemanticSsaOwnerV1, SemanticFunctionIdV1)>,
}

impl Resolver<'_, '_> {
    fn place(
        &mut self,
        block: u32,
        place: &SemanticPlaceV1,
        suffix: &[SemanticProjectionKindV1],
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        let count = place
            .projections()
            .len()
            .checked_add(suffix.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.graph.charge(std::mem::size_of::<Vec<SemanticProjectionKindV1>>()
            .div_ceil(std::mem::size_of::<usize>()))?;
        self.graph.charge(count.saturating_mul(
            std::mem::size_of::<SemanticProjectionKindV1>().div_ceil(std::mem::size_of::<usize>())
                + 1,
        ))?;
        let mut projections = Vec::with_capacity(count);
        projections.extend(place.projections().iter().map(|p| p.kind()));
        projections.extend_from_slice(suffix);
        let value = self.graph.use_value(block, place.local().index())?;
        if matches!(place.projections().first().map(|p| p.kind()), Some(SemanticProjectionKindV1::Downcast(_)))
            && let [SemanticProjectionKindV1::Downcast(variant), SemanticProjectionKindV1::Field(field), remaining @ ..] = projections.as_slice()
        {
            let enum_type = self.graph.body.locals()[place.local().index() as usize].ty();
            if let Some(issuer) = self.guarded_payload(value, enum_type, *variant, *field, remaining, block)? {
                return Ok(issuer);
            }
        }
        self.value(value, &projections)
    }

    fn operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        suffix: &[SemanticProjectionKindV1],
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(block, place, suffix)
            }
            SemanticOperandV1::Constant(_) => Err(reject(
                "global BF16 storage cannot originate from a constant",
            )),
        }
    }

    fn value(
        &mut self,
        value: SsaValueV1,
        projections: &[SemanticProjectionKindV1],
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        self.graph.charge(1)?;
        if self.active.len() >= 128 || !self.active.insert(value) {
            return Err(reject(
                "global BF16 capture has cyclic or excessive SSA custody",
            ));
        }
        let result = self.definition(value, projections);
        self.active.remove(&value);
        result
    }

    fn definition(
        &mut self,
        value: SsaValueV1,
        projections: &[SemanticProjectionKindV1],
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        if let SsaValueV1::BlockArgument { block, variable } = value {
            let incoming = self.graph.incoming(block.get(), variable.get())?;
            let mut issuer = None;
            for incoming in incoming {
                let resolved = self.value(incoming, projections)?;
                if issuer.replace(resolved).is_some_and(|old| old != resolved) {
                    return Err(reject(
                        "global BF16 capture merges different Global issuers",
                    ));
                }
            }
            return issuer.ok_or_else(|| reject("global BF16 capture has no incoming issuer"));
        }
        let site = self.graph.definition(value)?;
        let body = self.graph.body;
        let block = &body.blocks()[site.block as usize];
        if let Some(statement) = site.statement {
            let SemanticStatementKindV1::Assign(assignment) =
                block.statements()[statement as usize].kind()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if self.target == ResolveTarget::MatrixConstruction && projections.is_empty() {
                if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                    if assignment.value().result_type() != self.contract.types().matrix
                        || aggregate.kind() != &SemanticAggregateKindV1::Aggregate
                        || aggregate.operands().len() != 8
                    {
                        return Err(reject(
                            "global BF16 matrix construction changed its exact source aggregate",
                        ));
                    }
                    return Ok(value);
                }
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    self.operand(site.block, operand, projections)
                }
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => {
                    let Some((SemanticProjectionKindV1::Dereference, remaining)) =
                        projections.split_first()
                    else {
                        return Err(reject(
                            "global BF16 capture loses its shared-reference projection",
                        ));
                    };
                    if !super::global_bf16_shared_reference_v1(
                        self.types,
                        assignment.value().result_type(),
                        place.ty(),
                    ) {
                        return Err(reject(
                            "global BF16 capture changed the exact shared-reference type",
                        ));
                    }
                    let owner_value = match self.graph.use_value(site.block, place.local().index()) {
                        Ok(value) => value,
                        Err(error) => {
                            if matches!(&error, ProductionSemanticKirErrorV1::Unsupported {
                                detail: "capability reference has no exact SSA use", ..
                            }) {
                                borrow_failure_observation::emit(
                                    self, site, value, assignment, place, projections,
                                );
                            }
                            return Err(error);
                        }
                    };
                    let issuer = self.place(site.block, place, remaining)?;
                    // The common graph checks ordered moves, overwrite, storage death,
                    // mutable/raw exposure and cyclic storage generations.
                    self.graph.loan_live(
                        CapabilityLoanV1 {
                            borrow: site,
                            owner_local: place.local().index(),
                            owner_value,
                        },
                        self.consumer,
                    )?;
                    Ok(issuer)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    let (field, remaining) = match (aggregate.kind(), projections) {
                        (
                            SemanticAggregateKindV1::Aggregate | SemanticAggregateKindV1::Tuple,
                            [SemanticProjectionKindV1::Field(field), remaining @ ..],
                        ) => (*field, remaining),
                        (
                            SemanticAggregateKindV1::EnumVariant(actual),
                            [
                                SemanticProjectionKindV1::Downcast(expected),
                                SemanticProjectionKindV1::Field(field),
                                remaining @ ..,
                            ],
                        ) if actual == expected => (*field, remaining),
                        (SemanticAggregateKindV1::EnumVariant(_), _) => {
                            if std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").as_deref()
                                == Some(std::ffi::OsStr::new("1"))
                            {
                                use std::io::Write as _;
                                let mut diagnostic = std::io::stderr().lock();
                                let _ = writeln!(diagnostic,
                                    "BF16_RESULT_BOUNDARY body={:?} target={} consumer={:?} value={:?} definition={:?} enum_type={} aggregate={:?} projections={} prefix={:?} active_depth={} operands={}",
                                    body.identity(),
                                    match self.target { ResolveTarget::Global => "Global", ResolveTarget::MatrixConstruction => "MatrixConstruction" },
                                    self.consumer, value, site, assignment.value().result_type().index(),
                                    aggregate.kind(), projections.len(), &projections[..projections.len().min(12)],
                                    self.active.len(), aggregate.operands().len(),
                                );
                                for (index, operand) in aggregate.operands().iter().take(4).enumerate() {
                                    match operand {
                                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                                            let _ = writeln!(diagnostic,
                                                "BF16_RESULT_OPERAND index={} mode={} local={} type={} projections={} prefix={:?}",
                                                index, if matches!(operand, SemanticOperandV1::Copy(_)) { "Copy" } else { "Move" },
                                                place.local().index(), place.ty().index(), place.projections().len(),
                                                &place.projections()[..place.projections().len().min(12)],
                                            );
                                        }
                                        SemanticOperandV1::Constant(_) => {
                                            let _ = writeln!(diagnostic,
                                                "BF16_RESULT_OPERAND index={} mode=Constant type={}", index, operand.ty().index(),
                                            );
                                        }
                                    }
                                }
                            }
                            return Err(reject(
                                "global BF16 Result needs variant-sensitive payload SSA custody",
                            ));
                        }
                        _ => {
                            return Err(reject(
                                "global BF16 capture has an unsupported aggregate projection",
                            ));
                        }
                    };
                    let operand = aggregate
                        .operands()
                        .get(field as usize)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    self.operand(site.block, operand, remaining)
                }
                _ => Err(reject(
                    "global BF16 capture is not an immutable source value flow",
                )),
            }
        } else {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation:
                    SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                        view,
                        element,
                        contract,
                        provenance,
                        source_identity,
                        ..
                    },
                ..
            }) = self.callables.get(call.callee().index() as usize)
            else {
                return Err(reject(
                    "global BF16 storage lacks its actual read-only Global bind",
                ));
            };
            if self.target != ResolveTarget::Global
                || !projections.is_empty()
                || *view != self.contract.types().global
                || *element != self.contract.types().element
                || *contract != self.contract.memory()
                || *provenance != self.contract.provenance()
                || *source_identity != binding.identity()
                || call.destination().map(|d| d.place().ty()) != Some(*view)
            {
                return Err(reject(
                    "global BF16 actual Global issuer type or provenance changed",
                ));
            }
            Ok(value)
        }
    }
}

#[cfg(test)]
pub(super) fn resolve_component(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    ssa: &fe2o3_mir_model::SsaConstructionPlanV1,
    block: u32,
    operand: &SemanticOperandV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    let mut graph = CapabilitySsaGraphV1::new(function, ssa, 65_536)?;
    Resolver {
        graph: &mut graph,
        types,
        callables,
        active: BTreeSet::new(),
        contract,
        target: ResolveTarget::Global,
        observation_source: None,
        consumer: CapabilityDefinitionSiteV1 {
            block,
            statement: None,
            local: 0,
        },
    }
    .operand(
        block,
        operand,
        &[
            SemanticProjectionKindV1::Dereference,
            SemanticProjectionKindV1::Field(0),
            SemanticProjectionKindV1::Dereference,
        ],
    )
}

pub(super) fn verify(
    owner: &ProductionSemanticSsaOwnerV1,
    function: &SemanticFunctionDeclV1,
    root: SemanticFunctionIdV1,
    context: Option<&RootKernelContextLoweringV1>,
    max_work: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let semantic = owner.source_semantic();
    let calls = function
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(block, body)| {
            let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                return None;
            };
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
                ..
            } = semantic.callables().get(call.callee().index() as usize)?
            else {
                return None;
            };
            Some((block as u32, call, binding, *contract))
        })
        .collect::<Vec<_>>();
    if calls.is_empty() {
        return Ok(());
    }
    owner
        .verify_replay()
        .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
    let view = owner
        .execution_view_for_root(root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let plan = owner
        .execution_plan_for_root(root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let context = context.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if view.body() != function || context.selected_root != root {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let first = calls[0].3;
    let mut graph = CapabilitySsaGraphV1::new(function, plan.plan(), max_work)?;
    let mut resolver = Resolver {
        graph: &mut graph,
        types: semantic.types(),
        callables: semantic.callables(),
        active: BTreeSet::new(),
        consumer: CapabilityDefinitionSiteV1 {
            block: 0,
            statement: None,
            local: 0,
        },
        contract: first,
        target: ResolveTarget::Global,
        observation_source: Some((owner, root)),
    };
    for (block, call, binding, contract) in calls {
        if !global_capability_provenance_matches_v1(context, contract.provenance())
            || binding.identity() != contract.source_identity()
            || call.arguments().len() != 4
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        resolver.contract = contract;
        resolver.consumer.block = block;
        resolver.operand(
            block,
            &call.arguments()[0],
            &[
                SemanticProjectionKindV1::Dereference,
                SemanticProjectionKindV1::Field(0),
                SemanticProjectionKindV1::Dereference,
            ],
        )?;
    }
    Ok(())
}

include!("source_inputs.rs");
include!("constructor_frame.rs");
include!("source_session.rs");
include!("source_transport.rs");
