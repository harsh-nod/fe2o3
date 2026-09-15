//! Complete original source protocol for the first acyclic, dormant-storage
//! slice. No phase lease may escape or enter an unimplemented memory consumer.
use super::{
    PhaseResult, completion,
    definitions::{BodyRecipe, Definition, Role, Types},
    hir_calls::Receipt,
    rejected,
    source_calls::{Call, reserve, spend},
    source_cfg,
};
use crate::collector::production_importer_v1::{
    AuthenticatedProductionKernelContextsV1,
    numerical_policy_v1::defined_body_v1::DefinedSourceRosterV1, reusable_lds_v1, source_body_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_hir::{
    Expr, ExprKind, HirId, PatKind, Stmt, StmtKind,
    def::Res,
    intravisit::{self, Visitor},
};
use rustc_middle::ty::{Instance, TyCtxt, TypeckResults};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Site {
    pub function: SemanticFunctionIdV1,
    pub block: SemanticBlockIdV1,
}
impl Site {
    fn of(call: &Call<'_, '_>) -> Self {
        Self {
            function: call.caller,
            block: call.block,
        }
    }
}
#[derive(Debug)]
pub(super) struct Bind {
    pub site: Site,
    pub storage: SemanticReusableLdsConversionV1,
    pub lease_binding: HirId,
}
#[derive(Debug)]
pub(super) struct Protocol {
    pub owner: Site,
    pub wrapper: Site,
    pub issue: Site,
    pub finish: Site,
    pub closure: SemanticFunctionIdV1,
    pub owner_binding: HirId,
    pub phase_binding: HirId,
    pub binds: Vec<Bind>,
}
#[derive(Clone, Copy)]
struct Allowed {
    binding: HirId,
    function: SemanticFunctionIdV1,
    expression: HirId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
    definitions: &[(SemanticFunctionIdV1, Definition<'tcx>)],
    calls: &[Call<'a, 'tcx>],
    receipts: &[Receipt<'_, 'tcx>],
    roster: &DefinedSourceRosterV1<'a, 'tcx>,
    replay: &mut source_body_v1::Replay<'a, 'tcx>,
    work: &mut usize,
) -> PhaseResult<Vec<Protocol>> {
    if calls.len() != receipts.len() {
        return Err(rejected("phase source protocol receipt roster"));
    }
    // The storage records are reauthenticated through the existing owned LDS
    // source checker, not trusted as inert provenance supplied by the caller.
    reusable_lds_v1::validate_table_carriage(
        tcx, plan, contexts, types, functions, callables, work,
    )?;
    for (call, receipt) in calls.iter().zip(receipts) {
        spend(work, 1)?;
        if !std::ptr::eq(call, receipt.call) {
            return Err(rejected("phase source protocol receipt owner"));
        }
    }
    let definition = |id, work: &mut usize| -> PhaseResult<&Definition<'tcx>> {
        for (function, definition) in definitions {
            spend(work, 1)?;
            if *function == id {
                return Ok(definition);
            }
        }
        Err(rejected("phase protocol definition outside exact roster"))
    };
    let mut used = reserve(calls.len(), work)?;
    used.resize(calls.len(), 0usize);
    let mut protocols = reserve(calls.len(), work)?;
    let mut guards = reserve(
        calls
            .len()
            .checked_mul(3)
            .ok_or_else(|| rejected("phase source guard capacity overflow"))?,
        work,
    )?;
    let mut allowed = reserve(
        calls
            .len()
            .checked_mul(3)
            .ok_or_else(|| rejected("phase source use capacity overflow"))?,
        work,
    )?;
    for (index, wrapper) in calls.iter().enumerate() {
        if definition(wrapper.callee, work)?.role != Role::WithPhase {
            continue;
        }
        let receipt = receipts
            .get(index)
            .ok_or_else(|| rejected("phase wrapper receipt"))?;
        if !std::ptr::eq(receipt.call, wrapper) {
            return Err(rejected("phase wrapper receipt owner"));
        }
        let method = receipt
            .method
            .as_ref()
            .ok_or_else(|| rejected("phase wrapper has no lexical method"))?;
        let (owner_init, _) = initializer(
            tcx,
            wrapper.caller_instance,
            method.receiver_binding,
            None,
            work,
        )?;
        let mut owner_call = None;
        for (other_index, other) in calls.iter().enumerate() {
            spend(work, 1)?;
            if other.caller == wrapper.caller
                && definition(other.callee, work)?.role == Role::OwnerConvert
                && receipts[other_index]
                    .method
                    .as_ref()
                    .is_some_and(|m| m.expression.hir_id == owner_init.hir_id)
            {
                if owner_call.replace(other_index).is_some() {
                    return Err(rejected("phase ambiguous owner initializer"));
                }
            }
        }
        let owner_index = owner_call
            .ok_or_else(|| rejected("phase wrapper owner is not its original conversion"))?;
        let owner = &calls[owner_index];
        source_cfg::acyclic(wrapper.original, work)?;
        source_cfg::normal_result_before(
            wrapper.original,
            owner.raw_block,
            wrapper.raw_block,
            work,
        )?;
        let d = definition(wrapper.callee, work)?;
        let BodyRecipe::WithPhase { invoke, issue, .. } = d.recipe else {
            return Err(rejected("phase wrapper recipe mismatch"));
        };
        let Types::WithPhase {
            owner: owner_ty,
            phase,
            ..
        } = d.types
        else {
            return Err(rejected("phase wrapper typed source mismatch"));
        };
        if !matches!(definition(owner.callee, work)?.types, Types::OwnerConvert { owner, .. } if owner == owner_ty)
        {
            return Err(rejected("phase owner type substitution"));
        }
        let closure_id = plan
            .function_producers()
            .iter()
            .enumerate()
            .try_fold(None::<SemanticFunctionIdV1>, |found, (i, p)| {
                spend(work, 1)?;
                if p.instance != invoke {
                    return Ok(found);
                }
                if found.is_some() {
                    return Err(rejected("phase duplicate closure instance"));
                }
                Ok(Some(SemanticFunctionIdV1::from_index(i as u32)))
            })?
            .ok_or_else(|| rejected("phase closure outside original plan"))?;
        let local_closure = invoke
            .def_id()
            .as_local()
            .ok_or_else(|| rejected("phase closure HIR owner"))?;
        let hir = tcx.hir_body_owned_by(local_closure);
        let [parameter] = hir.params else {
            return Err(rejected("phase closure parameter roster"));
        };
        let PatKind::Binding(_, phase_binding, _, None) = parameter.pat.kind else {
            return Err(rejected(
                "phase closure requires exact owned parameter binding",
            ));
        };
        let relay = completion::observe(tcx, d, work)
            .map_err(|_| rejected("phase completion source chain"))?;
        source_cfg::acyclic(tcx.instance_mir(invoke.def), work)?;
        let mut issue_index = None;
        let mut finish_index = None;
        let mut binds: Vec<Bind> = reserve(calls.len().min(15), work)?;
        for (other_index, other) in calls.iter().enumerate() {
            spend(work, 1)?;
            if other.caller == wrapper.callee && other.callee_instance == issue {
                if issue_index.replace(other_index).is_some() {
                    return Err(rejected("phase wrapper has multiple Issue calls"));
                }
            }
            if other.caller != closure_id {
                continue;
            }
            let other_definition = definition(other.callee, work)?;
            let other_method = receipts[other_index]
                .method
                .as_ref()
                .ok_or_else(|| rejected("phase closure method lost HIR custody"))?;
            if other_method.receiver_binding != phase_binding {
                return Err(rejected(
                    "phase operation substituted the actual phase parameter",
                ));
            }
            match other_definition.types {
                Types::Finish {
                    phase: finish_phase,
                    ..
                } => {
                    if finish_phase.workgroup != phase.workgroup
                        || other.callee_instance != relay.finish
                        || other.raw_block != relay.finish_block
                        || other_method.expression.hir_id != relay.finish_expression
                        || finish_index.replace(other_index).is_some()
                    {
                        return Err(rejected(
                            "phase Finish is not the exact initial-phase completion",
                        ));
                    }
                }
                Types::Bind {
                    phase: bind_phase,
                    storage: storage_ty,
                    ..
                } => {
                    if bind_phase.workgroup != phase.workgroup || binds.len() == 15 {
                        return Err(rejected("phase Bind nominal phase or finite roster"));
                    }
                    let storage_binding = other_method
                        .storage_binding
                        .ok_or_else(|| rejected("phase Bind has no actual storage binding"))?;
                    let (storage_init, _) =
                        initializer(tcx, wrapper.caller_instance, storage_binding, None, work)?;
                    let storage = storage_origin(
                        tcx,
                        plan,
                        functions,
                        roster,
                        replay,
                        wrapper,
                        storage_init,
                        storage_ty,
                        work,
                    )?;
                    for prior in &binds {
                        spend(work, 1)?;
                        if prior.storage.source() == storage.source() {
                            return Err(rejected(
                                "phase binds one allocation more than once before End",
                            ));
                        }
                    }
                    let (_, lease_binding) = initializer(
                        tcx,
                        invoke,
                        other_method.expression.hir_id,
                        Some(other_method.expression.hir_id),
                        work,
                    )?;
                    let ExprKind::AddrOf(_, _, storage_use) = other_method
                        .argument
                        .ok_or_else(|| rejected("phase storage argument"))?
                        .kind
                    else {
                        return Err(rejected("phase storage reference expression"));
                    };
                    guards.push(lease_binding);
                    guards.push(storage_binding);
                    allowed.push(Allowed {
                        binding: storage_binding,
                        function: closure_id,
                        expression: storage_use.hir_id,
                    });
                    binds.push(Bind {
                        site: Site::of(other),
                        storage,
                        lease_binding,
                    });
                }
                _ => {
                    return Err(rejected(
                        "phase closure contains a nested or incomplete phase protocol",
                    ));
                }
            }
            used[other_index] = used[other_index]
                .checked_add(1)
                .ok_or_else(|| rejected("phase occurrence use overflow"))?;
            allowed.push(Allowed {
                binding: phase_binding,
                function: closure_id,
                expression: other_method.receiver.hir_id,
            });
        }
        let issue_index = issue_index.ok_or_else(|| rejected("phase wrapper omitted its Issue"))?;
        let finish_index =
            finish_index.ok_or_else(|| rejected("phase wrapper omitted its Finish"))?;
        let finish = &calls[finish_index];
        for bind in &binds {
            spend(work, calls.len())?;
            let call = calls
                .iter()
                .find(|call| Site::of(call) == bind.site)
                .ok_or_else(|| rejected("phase Bind source roster changed"))?;
            source_cfg::normal_result_before(
                call.original,
                call.raw_block,
                finish.raw_block,
                work,
            )?;
        }
        for site in [index, issue_index, owner_index] {
            used[site] = used[site]
                .checked_add(1)
                .ok_or_else(|| rejected("phase occurrence use overflow"))?;
        }
        guards.push(method.receiver_binding);
        guards.push(phase_binding);
        allowed.push(Allowed {
            binding: method.receiver_binding,
            function: wrapper.caller,
            expression: method.receiver.hir_id,
        });
        protocols.push(Protocol {
            owner: Site::of(owner),
            wrapper: Site::of(wrapper),
            issue: Site::of(&calls[issue_index]),
            finish: Site::of(finish),
            closure: closure_id,
            owner_binding: method.receiver_binding,
            phase_binding,
            binds,
        });
    }
    if protocols.is_empty() {
        return Err(rejected("phase source family has no complete protocol"));
    }
    for (index, call) in calls.iter().enumerate() {
        spend(work, 1)?;
        let role = definition(call.callee, work)?.role;
        if used[index] == 0 || (role != Role::OwnerConvert && used[index] != 1) {
            return Err(rejected(
                "phase source protocol omitted or reused an incoming occurrence",
            ));
        }
    }
    for (function, producer) in plan.function_producers().iter().enumerate() {
        let Some(local) = producer.instance.def_id().as_local() else {
            continue;
        };
        if !matches!(
            tcx.def_kind(local),
            rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::Closure
        ) {
            continue;
        }
        let mut audit = Audit {
            typeck: tcx.typeck(local),
            function: SemanticFunctionIdV1::from_index(function as u32),
            guards: &guards,
            allowed: &allowed,
            work,
            depth: 0,
            error: None,
        };
        audit.visit_expr(tcx.hir_body_owned_by(local).value);
        if let Some(error) = audit.error {
            return Err(error);
        }
    }
    Ok(protocols)
}

fn storage_origin<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    functions: &'a [SemanticFunctionDeclV1],
    roster: &DefinedSourceRosterV1<'a, 'tcx>,
    replay: &mut source_body_v1::Replay<'a, 'tcx>,
    wrapper: &Call<'a, 'tcx>,
    initializer: &Expr<'tcx>,
    storage_ty: rustc_middle::ty::Ty<'tcx>,
    work: &mut usize,
) -> PhaseResult<SemanticReusableLdsConversionV1> {
    let mut found = None;
    let mapping = roster.reconstructed_body(wrapper.caller, replay, work)?;
    for function in functions {
        spend(work, 1)?;
        let Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)) =
            function.defined_capability_contract()
        else {
            continue;
        };
        spend(work, plan.type_producers().len())?;
        if record.source().caller != wrapper.caller
            || record.types().output != roster.types([storage_ty])?[0]
        {
            continue;
        }
        for (block, data) in wrapper.original.basic_blocks.iter_enumerated() {
            spend(work, 1)?;
            if mapping.block(block.as_u32())? != record.source().conversion_block
                || data.terminator().source_info.span != initializer.span
            {
                continue;
            }
            let producer = plan
                .function_producers()
                .get(record.function().index() as usize)
                .ok_or_else(|| rejected("phase storage definition roster"))?;
            let typeck = tcx.typeck(
                wrapper
                    .caller_instance
                    .def_id()
                    .as_local()
                    .ok_or_else(|| rejected("phase storage local HIR"))?,
            );
            let def = typeck
                .type_dependent_def_id(initializer.hir_id)
                .ok_or_else(|| {
                    rejected("phase storage initializer is not its conversion method")
                })?;
            let args = super::definitions::normalize(
                tcx,
                wrapper.caller_instance,
                typeck.node_args(initializer.hir_id),
            )
            .map_err(|_| rejected("phase storage initializer generic arguments"))?;
            let instance = Instance::try_resolve(
                tcx,
                rustc_middle::ty::TypingEnv::fully_monomorphized(),
                def,
                tcx.erase_and_anonymize_regions(args),
            )
            .ok()
            .flatten();
            if instance != Some(producer.instance) || found.replace(*record).is_some() {
                return Err(rejected("phase storage original conversion substitution"));
            }
            source_cfg::normal_result_before(wrapper.original, block, wrapper.raw_block, work)?;
        }
    }
    found.ok_or_else(|| rejected("phase storage is not an authenticated owned reusable allocation"))
}

pub(super) fn commit(
    tcx: TyCtxt<'_>,
    protocols: &[Protocol],
    digest: &mut super::SemanticIdentityDigestV1,
    work: &mut usize,
) -> PhaseResult<()> {
    digest.field(b"complete-original-phase-protocol/v26");
    digest.field(&(protocols.len() as u64).to_le_bytes());
    for protocol in protocols {
        spend(work, 12)?;
        for site in [
            protocol.owner,
            protocol.wrapper,
            protocol.issue,
            protocol.finish,
        ] {
            digest.field(&site.function.index().to_le_bytes());
            digest.field(&site.block.index().to_le_bytes());
        }
        digest.field(&protocol.closure.index().to_le_bytes());
        commit_hir_binding(tcx, Some(protocol.owner_binding), digest, work)?;
        commit_hir_binding(tcx, Some(protocol.phase_binding), digest, work)?;
        digest.field(&(protocol.binds.len() as u64).to_le_bytes());
        for bind in &protocol.binds {
            spend(work, 5)?;
            digest.field(&bind.site.function.index().to_le_bytes());
            digest.field(&bind.site.block.index().to_le_bytes());
            digest.field(bind.storage.body_identity());
            digest.field(&bind.storage.source().source_binding);
            digest.field(&bind.storage.function().index().to_le_bytes());
            commit_hir_binding(tcx, Some(bind.lease_binding), digest, work)?;
        }
    }
    Ok(())
}

pub(super) fn commit_hir_binding(
    tcx: TyCtxt<'_>,
    binding: Option<HirId>,
    digest: &mut super::SemanticIdentityDigestV1,
    work: &mut usize,
) -> PhaseResult<()> {
    spend(work, 3)?;
    match binding {
        Some(binding) => {
            digest.field(&[1]);
            digest.field(
                &tcx.def_path_hash(binding.owner.def_id.to_def_id())
                    .0
                    .to_le_bytes(),
            );
            digest.field(&binding.local_id.as_u32().to_le_bytes());
        }
        None => digest.field(&[0]),
    }
    Ok(())
}

fn initializer<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    binding: HirId,
    expression: Option<HirId>,
    work: &mut usize,
) -> PhaseResult<(&'tcx Expr<'tcx>, HirId)> {
    let local = caller
        .def_id()
        .as_local()
        .ok_or_else(|| rejected("phase source binding HIR owner"))?;
    let mut scan = Initializer {
        binding,
        expression,
        work,
        depth: 0,
        found: None,
        error: None,
    };
    scan.visit_expr(tcx.hir_body_owned_by(local).value);
    if let Some(error) = scan.error {
        return Err(error);
    }
    scan.found
        .ok_or_else(|| rejected("phase source binding has no exact initializer"))
}
struct Initializer<'w, 'tcx> {
    binding: HirId,
    expression: Option<HirId>,
    work: &'w mut usize,
    depth: usize,
    found: Option<(&'tcx Expr<'tcx>, HirId)>,
    error: Option<super::ProductionSemanticImportErrorV1>,
}
impl<'tcx> Visitor<'tcx> for Initializer<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = spend(self.work, 1) {
            self.error = Some(error);
            return;
        }
        if let StmtKind::Let(local) = statement.kind
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
            && let Some(value) = local.init
            && self
                .expression
                .map_or(binding == self.binding, |id| value.hir_id == id)
        {
            if self.found.replace((value, binding)).is_some() {
                self.error = Some(rejected("phase source initializer is ambiguous"));
                return;
            }
        }
        intravisit::walk_stmt(self, statement);
    }
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if self.depth == 128 {
            self.error = Some(rejected("phase initializer traversal depth"));
            return;
        }
        if let Err(error) = spend(self.work, 1) {
            self.error = Some(error);
            return;
        }
        if !matches!(expression.kind, ExprKind::Closure(_)) {
            self.depth += 1;
            intravisit::walk_expr(self, expression);
            self.depth -= 1;
        }
    }
}
struct Audit<'a, 'tcx> {
    typeck: &'tcx TypeckResults<'tcx>,
    function: SemanticFunctionIdV1,
    guards: &'a [HirId],
    allowed: &'a [Allowed],
    work: &'a mut usize,
    depth: usize,
    error: Option<super::ProductionSemanticImportErrorV1>,
}
impl<'tcx> Visitor<'tcx> for Audit<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if self.depth == 128 {
            self.error = Some(rejected("phase alias audit depth"));
            return;
        }
        let cost = self
            .guards
            .len()
            .checked_add(self.allowed.len())
            .and_then(|n| n.checked_add(1));
        if cost.is_none() {
            self.error = Some(rejected("phase alias audit work overflow"));
            return;
        }
        if let Err(error) = spend(self.work, cost.unwrap()) {
            self.error = Some(error);
            return;
        }
        if let ExprKind::Path(ref path) = expression.kind
            && let Res::Local(binding) = self.typeck.qpath_res(path, expression.hir_id)
            && self.guards.contains(&binding)
            && !self.allowed.iter().any(|a| {
                a.binding == binding
                    && a.function == self.function
                    && a.expression == expression.hir_id
            })
        {
            self.error = Some(rejected(
                "phase owner, phase, storage or lease escaped its exact source protocol",
            ));
            return;
        }
        if !matches!(expression.kind, ExprKind::Closure(_)) {
            self.depth += 1;
            intravisit::walk_expr(self, expression);
            self.depth -= 1;
        }
    }
}
