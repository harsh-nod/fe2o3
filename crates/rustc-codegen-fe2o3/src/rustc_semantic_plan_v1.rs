//! Bounded two-pass preflight for the production rustc semantic importer.
//!
//! This module retains live rustc producers and proves that the selected raw
//! MIR is inside the first reviewed subset. It does not construct, admit, or
//! authorize canonical semantic MIR.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticBlockIdentityV1, SemanticFunctionIdV1, SemanticLocalIdV1,
    SemanticLocalIdentityV1, SemanticMirLimitsV1, SemanticMirResourceV1,
    SemanticSourceFileIdentityV1, SemanticSourceOriginV1, SemanticSourceProvenanceV1,
    SemanticTargetDataLayoutV1, SemanticTypeIdV1, SemanticTypeIdentityV1,
};
use rustc_middle::mir::{
    AggregateKind, Body, BorrowKind, Local, MutBorrowKind, Operand, Place, PlaceTy, ProjectionElem,
    Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{
    EarlyBinder, GenericArgKind, Instance, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use rustc_span::Span;

use crate::collector::CollectedFunctionRole;
use crate::production_semantic_terminal_v1::{
    ProductionSemanticTerminalRuleV1, ProductionTerminalExpansionV1,
};
use crate::rustc_semantic_adapter_v1::{
    CanonicalFunctionIdentitiesV1, CanonicalSourceProvenanceV1, SemanticIdentityDigestV1,
    canonical_function_identities_v1, canonical_source_provenance_v1, rustc_block_identity_v1,
    rustc_local_identity_v1, rustc_mir_body_sha256_v1, rustc_type_identity_v1,
};

const PREFLIGHT_PLAN_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-preflight-plan/v1";
const MAX_DIAGNOSTIC_COMPONENT_CHARS_V1: usize = 512;
const MAX_MACRO_EXPANSION_DEPTH_V1: usize = 256;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RetainedSemanticFunctionProducerV1<'tcx> {
    pub(crate) identities: CanonicalFunctionIdentitiesV1,
    pub(crate) instance: Instance<'tcx>,
    pub(crate) role: CollectedFunctionRole,
}

#[derive(Clone, Copy, Debug)]
struct RetainedSemanticTypeProducerV1<'tcx> {
    identity: SemanticTypeIdentityV1,
    ty: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RetainedSemanticLocalProducerV1 {
    identity: SemanticLocalIdentityV1,
    rustc_local: u32,
    ty: SemanticTypeIdV1,
    source: RetainedSemanticSourceProducerV1,
}

#[derive(Debug)]
struct RetainedSemanticBlockProducerV1 {
    identity: SemanticBlockIdentityV1,
    rustc_block: u32,
    source: RetainedSemanticSourceProducerV1,
    statements: Box<[RetainedSemanticSourceProducerV1]>,
    terminator: RetainedSemanticSourceProducerV1,
}

#[derive(Clone, Copy, Debug)]
struct RetainedSemanticSourceProducerV1 {
    provenance: SemanticSourceProvenanceV1,
    expansion_chain_sha256: [u8; 32],
}

#[derive(Debug)]
struct RetainedRawBlockSourceProducerV1 {
    source: RetainedSemanticSourceProducerV1,
    statements: Box<[RetainedSemanticSourceProducerV1]>,
    terminator: RetainedSemanticSourceProducerV1,
}

#[derive(Debug)]
struct RetainedRawBodySourceProducerV1 {
    source: RetainedSemanticSourceProducerV1,
    locals: Box<[RetainedSemanticSourceProducerV1]>,
    blocks: Box<[RetainedRawBlockSourceProducerV1]>,
}

#[derive(Debug)]
struct RetainedSemanticBodyProducerV1 {
    function: SemanticFunctionIdV1,
    source: RetainedSemanticSourceProducerV1,
    locals: Box<[RetainedSemanticLocalProducerV1]>,
    raw_to_semantic_locals: Box<[SemanticLocalIdV1]>,
    entry: SemanticBlockIdV1,
    blocks: Box<[RetainedSemanticBlockProducerV1]>,
    raw_to_semantic_blocks: Box<[SemanticBlockIdV1]>,
}

struct CanonicalProducerTablesV1<'tcx> {
    types: Box<[RetainedSemanticTypeProducerV1<'tcx>]>,
    source_files: Box<[SemanticSourceFileIdentityV1]>,
    bodies: Box<[RetainedSemanticBodyProducerV1]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RawMirPreflightCountsV1 {
    types: u64,
    functions: u64,
    roots: u64,
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    validation_work: u64,
}

impl RawMirPreflightCountsV1 {
    fn slot_mut(&mut self, resource: SemanticMirResourceV1) -> Option<&mut u64> {
        match resource {
            SemanticMirResourceV1::Types => Some(&mut self.types),
            SemanticMirResourceV1::Functions => Some(&mut self.functions),
            SemanticMirResourceV1::Roots => Some(&mut self.roots),
            SemanticMirResourceV1::Locals => Some(&mut self.locals),
            SemanticMirResourceV1::Blocks => Some(&mut self.blocks),
            SemanticMirResourceV1::Statements => Some(&mut self.statements),
            SemanticMirResourceV1::Projections => Some(&mut self.projections),
            SemanticMirResourceV1::Operands => Some(&mut self.operands),
            SemanticMirResourceV1::CallArguments => Some(&mut self.call_arguments),
            SemanticMirResourceV1::SwitchTargets => Some(&mut self.switch_targets),
            SemanticMirResourceV1::ValidationWork => Some(&mut self.validation_work),
            SemanticMirResourceV1::Callables
            | SemanticMirResourceV1::Allocations
            | SemanticMirResourceV1::Statics
            | SemanticMirResourceV1::VTables
            | SemanticMirResourceV1::Relocations
            | SemanticMirResourceV1::ConstantBytes
            | SemanticMirResourceV1::LinkSymbolBytes
            | SemanticMirResourceV1::CanonicalBytes => None,
        }
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
        limits: SemanticMirLimitsV1,
    ) -> Result<(), ProductionSemanticPreflightErrorV1> {
        let amount = u64::try_from(amount).unwrap_or(u64::MAX);
        let Some(slot) = self.slot_mut(resource) else {
            return Err(ProductionSemanticPreflightErrorV1::AccountingDomain { resource });
        };
        *slot =
            slot.checked_add(amount)
                .ok_or(ProductionSemanticPreflightErrorV1::LimitExceeded {
                    resource,
                    actual: u64::MAX,
                    maximum: limits.limit(resource),
                })?;
        let maximum = limits.limit(resource);
        if *slot > maximum {
            return Err(ProductionSemanticPreflightErrorV1::LimitExceeded {
                resource,
                actual: *slot,
                maximum,
            });
        }
        Ok(())
    }

    fn digest_fields(self) -> [u64; 11] {
        [
            self.types,
            self.functions,
            self.roots,
            self.locals,
            self.blocks,
            self.statements,
            self.projections,
            self.operands,
            self.call_arguments,
            self.switch_targets,
            self.validation_work,
        ]
    }

    pub(crate) const fn locals(self) -> u64 {
        self.locals
    }

    pub(crate) const fn blocks(self) -> u64 {
        self.blocks
    }

    pub(crate) const fn statements(self) -> u64 {
        self.statements
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CallEdgeV1 {
    caller: SemanticFunctionIdV1,
    callee: SemanticFunctionIdV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TerminalExpansionRecipeV1 {
    caller: SemanticFunctionIdV1,
    block: u32,
    expansion: ProductionTerminalExpansionV1,
    arguments: u32,
}

#[derive(Debug)]
pub(crate) struct ProductionSemanticPreflightPlanV1<'tcx> {
    types: Box<[RetainedSemanticTypeProducerV1<'tcx>]>,
    source_files: Box<[SemanticSourceFileIdentityV1]>,
    functions: Box<[RetainedSemanticFunctionProducerV1<'tcx>]>,
    bodies: Box<[RetainedSemanticBodyProducerV1]>,
    roots: Box<[SemanticFunctionIdV1]>,
    terminal_expansions: Box<[TerminalExpansionRecipeV1]>,
    raw_counts: RawMirPreflightCountsV1,
    sha256: [u8; 32],
}

impl ProductionSemanticPreflightPlanV1<'_> {
    pub(crate) fn type_producer_count(&self) -> usize {
        self.types.len()
    }

    pub(crate) fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub(crate) fn source_file_producer_count(&self) -> usize {
        self.source_files.len()
    }

    pub(crate) fn source_provenance_producer_count(&self) -> usize {
        source_provenance_producer_count_v1(&self.bodies)
    }

    pub(crate) fn body_producer_count(&self) -> usize {
        self.bodies.len()
    }

    pub(crate) fn root_count(&self) -> usize {
        self.roots.len()
    }

    pub(crate) fn terminal_expansion_count(&self) -> usize {
        self.terminal_expansions.len()
    }

    pub(crate) const fn raw_counts(&self) -> RawMirPreflightCountsV1 {
        self.raw_counts
    }

    pub(crate) const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticPreflightErrorV1 {
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    AccountingDomain {
        resource: SemanticMirResourceV1,
    },
    TypeIdentityCollision,
    IdentityTableMismatch,
    UnsupportedRustcMir {
        construct: String,
        function: String,
        call_chain: Box<[String]>,
        location: String,
        source: String,
    },
}

impl fmt::Display for ProductionSemanticPreflightErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "raw rustc MIR preflight rejected {resource:?} count {actual}; maximum is {maximum}",
            ),
            Self::AccountingDomain { resource } => write!(
                formatter,
                "raw rustc MIR preflight attempted to charge non-raw resource {resource:?}",
            ),
            Self::TypeIdentityCollision => formatter.write_str(
                "raw rustc MIR preflight derived one type identity for distinct normalized rustc types",
            ),
            Self::IdentityTableMismatch => formatter.write_str(
                "raw rustc MIR preflight could not assign a complete canonical producer table",
            ),
            Self::UnsupportedRustcMir {
                construct,
                function,
                call_chain,
                location,
                source,
            } => write!(
                formatter,
                "raw rustc MIR preflight rejected {construct} in {function} at {location} ({source}); reachable call chain: {}",
                call_chain.join(" -> "),
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticPreflightErrorV1 {}

#[derive(Clone, Copy)]
struct RejectionSiteV1 {
    function: SemanticFunctionIdV1,
    block: Option<u32>,
    statement: Option<u32>,
    local: Option<u32>,
    span: Span,
}

enum PendingRejectionV1 {
    Unsupported {
        construct: String,
        site: RejectionSiteV1,
    },
    Fatal(ProductionSemanticPreflightErrorV1),
}

struct BodyPreflightV1<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    function: SemanticFunctionIdV1,
    limits: SemanticMirLimitsV1,
    counts: &'a mut RawMirPreflightCountsV1,
    types: &'a mut BTreeMap<SemanticTypeIdentityV1, Ty<'tcx>>,
}

pub(crate) fn build_production_semantic_preflight_plan_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    functions: Box<[RetainedSemanticFunctionProducerV1<'tcx>]>,
    roots: Box<[SemanticFunctionIdV1]>,
    identity_inventory_sha256: [u8; 32],
) -> Result<ProductionSemanticPreflightPlanV1<'tcx>, ProductionSemanticPreflightErrorV1> {
    let limits = SemanticMirLimitsV1::default();
    let mut counts = RawMirPreflightCountsV1::default();
    counts.charge(SemanticMirResourceV1::Functions, functions.len(), limits)?;
    counts.charge(SemanticMirResourceV1::Roots, roots.len(), limits)?;

    let function_ids = functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                function.identities.function(),
                SemanticFunctionIdV1::from_index(index as u32),
            )
        })
        .collect::<BTreeMap<_, _>>();

    // Structural cardinalities are charged before pass one traverses their
    // contents. Call discovery is therefore bounded by the same limits as
    // later canonical construction.
    for function in &functions {
        let body = tcx.instance_mir(function.instance.def);
        counts.charge(
            SemanticMirResourceV1::Locals,
            body.local_decls.len(),
            limits,
        )?;
        counts.charge(
            SemanticMirResourceV1::Blocks,
            body.basic_blocks.len(),
            limits,
        )?;
        for data in body.basic_blocks.iter() {
            counts.charge(
                SemanticMirResourceV1::Statements,
                data.statements.len(),
                limits,
            )?;
            if let Some(terminator) = &data.terminator
                && let TerminatorKind::Call { args, .. } = &terminator.kind
            {
                counts.charge(SemanticMirResourceV1::CallArguments, args.len(), limits)?;
            }
        }
    }

    // Pass one resolves the complete direct-call relation and freezes typed
    // terminal recipes before any body is accepted as representable.
    let mut edges = BTreeSet::new();
    let mut terminal_expansions = BTreeSet::new();
    let mut first_rejection = None;
    for (index, function) in functions.iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(index as u32);
        let body = tcx.instance_mir(function.instance.def);
        for (block, data) in body.basic_blocks.iter_enumerated() {
            let Some(terminator) = &data.terminator else {
                remember_rejection(
                    &mut first_rejection,
                    "basic block without a terminator",
                    RejectionSiteV1 {
                        function: function_id,
                        block: Some(block.index() as u32),
                        statement: None,
                        local: None,
                        span: body.span,
                    },
                );
                continue;
            };
            let TerminatorKind::Call { func, args, .. } = &terminator.kind else {
                continue;
            };
            let site = RejectionSiteV1 {
                function: function_id,
                block: Some(block.index() as u32),
                statement: None,
                local: None,
                span: terminator.source_info.span,
            };
            match resolve_direct_call_v1(tcx, function.instance, body, func) {
                Ok(resolved) => {
                    match crate::production_semantic_terminal_v1::classify(tcx, resolved.def_id()) {
                        Some(ProductionSemanticTerminalRuleV1::Expand(expansion)) => {
                            let Ok(arguments) = u32::try_from(args.len()) else {
                                remember_rejection(
                                    &mut first_rejection,
                                    "terminal call argument count outside u32",
                                    site,
                                );
                                continue;
                            };
                            terminal_expansions.insert(TerminalExpansionRecipeV1 {
                                caller: function_id,
                                block: block.index() as u32,
                                expansion,
                                arguments,
                            });
                        }
                        Some(ProductionSemanticTerminalRuleV1::Reject(item)) => {
                            remember_rejection(
                                &mut first_rejection,
                                format!("reviewed terminal without production expansion: {item:?}"),
                                site,
                            );
                        }
                        None => {
                            let identity =
                                canonical_function_identities_v1(tcx, resolved).function();
                            if let Some(callee) = function_ids.get(&identity).copied() {
                                edges.insert(CallEdgeV1 {
                                    caller: function_id,
                                    callee,
                                });
                            } else {
                                remember_rejection(
                                    &mut first_rejection,
                                    "direct call escaped the collector-sealed function closure",
                                    site,
                                );
                            }
                        }
                    }
                }
                Err(construct) => remember_rejection(&mut first_rejection, construct, site),
            }
        }
    }

    if let Some(rejection) = first_rejection {
        return Err(materialize_rejection_v1(
            tcx, &functions, &roots, &edges, rejection,
        ));
    }

    if let Some(unreachable) = first_unreachable_function_v1(&roots, &edges, functions.len()) {
        let function = &functions[unreachable.index() as usize];
        return Err(materialize_rejection_v1(
            tcx,
            &functions,
            &roots,
            &edges,
            reject(
                "collector-sealed function without a root call path",
                RejectionSiteV1 {
                    function: unreachable,
                    block: None,
                    statement: None,
                    local: None,
                    span: tcx.instance_mir(function.instance.def).span,
                },
            ),
        ));
    }

    // Pass two walks every raw MIR node once, classifies the first supported
    // subset, and charges only resources directly observed in rustc MIR.
    let mut types = BTreeMap::new();
    let mut source_producers = Vec::with_capacity(functions.len());
    let mut source_cache = HashMap::new();
    for (index, function) in functions.iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(index as u32);
        let body = tcx.instance_mir(function.instance.def);
        let sources = capture_body_sources_v1(
            tcx,
            function_id,
            body,
            &mut source_cache,
            &mut counts,
            limits,
        )
        .map_err(|rejection| {
            materialize_rejection_v1(tcx, &functions, &roots, &edges, rejection)
        })?;
        source_producers.push(sources);
        let mut preflight = BodyPreflightV1 {
            tcx,
            instance: function.instance,
            body,
            function: function_id,
            limits,
            counts: &mut counts,
            types: &mut types,
        };
        if let Err(rejection) = preflight.inspect_body() {
            return Err(materialize_rejection_v1(
                tcx, &functions, &roots, &edges, rejection,
            ));
        }
    }

    let CanonicalProducerTablesV1 {
        types,
        source_files,
        bodies,
    } = build_canonical_producer_tables_v1(tcx, &functions, source_producers, types)?;

    let terminal_expansions = terminal_expansions.into_iter().collect::<Box<[_]>>();
    let sha256 = preflight_plan_sha256_v1(
        target,
        identity_inventory_sha256,
        &types,
        &source_files,
        &functions,
        &bodies,
        &roots,
        &edges,
        &terminal_expansions,
        counts,
        tcx,
    );
    Ok(ProductionSemanticPreflightPlanV1 {
        types,
        source_files,
        functions,
        bodies,
        roots,
        terminal_expansions,
        raw_counts: counts,
        sha256,
    })
}

impl<'a, 'tcx> BodyPreflightV1<'a, 'tcx> {
    fn inspect_body(&mut self) -> Result<(), PendingRejectionV1> {
        for (local, declaration) in self.body.local_decls.iter_enumerated() {
            let site = RejectionSiteV1 {
                function: self.function,
                block: None,
                statement: None,
                local: Some(local.index() as u32),
                span: declaration.source_info.span,
            };
            self.inspect_type(declaration.ty, site)?;
        }
        for (block, data) in self.body.basic_blocks.iter_enumerated() {
            for (statement_index, statement) in data.statements.iter().enumerate() {
                let site = RejectionSiteV1 {
                    function: self.function,
                    block: Some(block.index() as u32),
                    statement: Some(statement_index as u32),
                    local: None,
                    span: statement.source_info.span,
                };
                self.inspect_statement(&statement.kind, site)?;
            }
            let terminator = data.terminator.as_ref().ok_or_else(|| {
                reject(
                    "basic block without a terminator",
                    RejectionSiteV1 {
                        function: self.function,
                        block: Some(block.index() as u32),
                        statement: None,
                        local: None,
                        span: self.body.span,
                    },
                )
            })?;
            let site = RejectionSiteV1 {
                function: self.function,
                block: Some(block.index() as u32),
                statement: None,
                local: None,
                span: terminator.source_info.span,
            };
            self.inspect_terminator(&terminator.kind, site)?;
        }
        Ok(())
    }

    fn inspect_statement(
        &mut self,
        statement: &StatementKind<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        self.work()?;
        match statement {
            StatementKind::Assign(assignment) => {
                let (destination, value) = &**assignment;
                self.inspect_place(*destination, site)?;
                self.inspect_rvalue(value, site)
            }
            StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                self.require_local(*local, site)
            }
            StatementKind::SetDiscriminant { place, .. } => self.inspect_place(**place, site),
            StatementKind::Nop => Ok(()),
            StatementKind::FakeRead(..) => Err(reject("FakeRead statement", site)),
            StatementKind::Intrinsic(..) => Err(reject("intrinsic statement", site)),
            StatementKind::Retag(..) => Err(reject("Retag statement", site)),
            StatementKind::PlaceMention(..) => Err(reject("PlaceMention statement", site)),
            StatementKind::AscribeUserType(..) => Err(reject("AscribeUserType statement", site)),
            StatementKind::Coverage(..) => Err(reject("Coverage statement", site)),
            StatementKind::ConstEvalCounter => Err(reject("ConstEvalCounter statement", site)),
            StatementKind::BackwardIncompatibleDropHint { .. } => {
                Err(reject("BackwardIncompatibleDropHint statement", site))
            }
        }
    }

    fn inspect_rvalue(
        &mut self,
        value: &Rvalue<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        self.work()?;
        match value {
            Rvalue::Use(operand) => self.inspect_operand(operand, site),
            Rvalue::Ref(_, borrow, place) => {
                match borrow {
                    BorrowKind::Shared | BorrowKind::Fake(_) => {}
                    // Two-phase activation is borrow-checker state; both forms
                    // have the same mutable-reference runtime representation.
                    BorrowKind::Mut {
                        kind: MutBorrowKind::Default | MutBorrowKind::TwoPhaseBorrow,
                    } => {}
                    BorrowKind::Mut {
                        kind: MutBorrowKind::ClosureCapture,
                    } => {
                        return Err(reject("closure-capture mutable borrow rvalue", site));
                    }
                }
                self.inspect_place(*place, site)
            }
            Rvalue::Discriminant(place) | Rvalue::CopyForDeref(place) => {
                self.inspect_place(*place, site)
            }
            Rvalue::Aggregate(kind, operands) => {
                match &**kind {
                    AggregateKind::Array(_) | AggregateKind::Tuple | AggregateKind::Adt(..) => {}
                    AggregateKind::Closure(..) => {
                        return Err(reject("closure aggregate rvalue", site));
                    }
                    AggregateKind::CoroutineClosure(..) => {
                        return Err(reject("coroutine-closure aggregate rvalue", site));
                    }
                    AggregateKind::Coroutine(..) => {
                        return Err(reject("coroutine aggregate rvalue", site));
                    }
                    AggregateKind::RawPtr(..) => {
                        return Err(reject("raw-pointer aggregate rvalue", site));
                    }
                }
                for operand in operands {
                    self.inspect_operand(operand, site)?;
                }
                Ok(())
            }
            Rvalue::Repeat(..) => Err(reject("Repeat rvalue", site)),
            Rvalue::RawPtr(..) => Err(reject("RawPtr rvalue", site)),
            Rvalue::Cast(..) => Err(reject("Cast rvalue", site)),
            Rvalue::BinaryOp(..) => Err(reject("BinaryOp rvalue", site)),
            Rvalue::UnaryOp(..) => Err(reject("UnaryOp rvalue", site)),
            Rvalue::ThreadLocalRef(..) => Err(reject("ThreadLocalRef rvalue", site)),
            Rvalue::WrapUnsafeBinder(..) => Err(reject("WrapUnsafeBinder rvalue", site)),
        }
    }

    fn inspect_terminator(
        &mut self,
        terminator: &TerminatorKind<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        self.work()?;
        match terminator {
            TerminatorKind::Return | TerminatorKind::Unreachable | TerminatorKind::Goto { .. } => {
                Ok(())
            }
            TerminatorKind::SwitchInt { discr, targets } => {
                self.inspect_operand(discr, site)?;
                self.charge(SemanticMirResourceV1::SwitchTargets, targets.iter().count())
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                unwind,
                ..
            } => {
                if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
                    return Err(reject("call with executable unwind edge", site));
                }
                self.inspect_operand(func, site)?;
                for argument in args {
                    self.inspect_operand(&argument.node, site)?;
                }
                self.inspect_place(*destination, site)
            }
            TerminatorKind::TailCall { .. } => Err(reject("TailCall terminator", site)),
            TerminatorKind::Drop { .. } => Err(reject("Drop terminator", site)),
            TerminatorKind::Assert { .. } => Err(reject("Assert terminator", site)),
            TerminatorKind::UnwindResume => Err(reject("UnwindResume terminator", site)),
            TerminatorKind::UnwindTerminate(..) => Err(reject("UnwindTerminate terminator", site)),
            TerminatorKind::Yield { .. } => Err(reject("Yield terminator", site)),
            TerminatorKind::CoroutineDrop => Err(reject("CoroutineDrop terminator", site)),
            TerminatorKind::FalseEdge { .. } => Err(reject("FalseEdge terminator", site)),
            TerminatorKind::FalseUnwind { .. } => Err(reject("FalseUnwind terminator", site)),
            TerminatorKind::InlineAsm { .. } => Err(reject("InlineAsm terminator", site)),
        }
    }

    fn inspect_operand(
        &mut self,
        operand: &Operand<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        self.charge(SemanticMirResourceV1::Operands, 1)?;
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.inspect_place(*place, site),
            Operand::Constant(constant) => {
                let normalized = self
                    .instance
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        self.tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(constant.const_),
                    )
                    .map_err(|_| reject("constant that failed monomorphic normalization", site))?;
                self.inspect_type(normalized.ty(), site)
            }
            Operand::RuntimeChecks(..) => Err(reject("RuntimeChecks operand", site)),
        }
    }

    fn inspect_place(
        &mut self,
        place: Place<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        self.require_local(place.local, site)?;
        self.charge(SemanticMirResourceV1::Projections, place.projection.len())?;
        let local_ty = self.body.local_decls[place.local].ty;
        self.inspect_type(local_ty, site)?;
        let mut derived = PlaceTy::from_ty(local_ty);
        for projection in place.projection {
            match projection {
                ProjectionElem::Deref
                | ProjectionElem::Field(..)
                | ProjectionElem::ConstantIndex { .. }
                | ProjectionElem::Subslice { .. }
                | ProjectionElem::Downcast(..)
                | ProjectionElem::OpaqueCast(..) => {}
                ProjectionElem::Index(local) => self.require_local(local, site)?,
                ProjectionElem::UnwrapUnsafeBinder(..) => {
                    return Err(reject("UnwrapUnsafeBinder projection", site));
                }
            }
            derived = derived.projection_ty(self.tcx, projection);
            self.inspect_type(derived.ty, site)?;
        }
        Ok(())
    }

    fn inspect_type(
        &mut self,
        raw: Ty<'tcx>,
        site: RejectionSiteV1,
    ) -> Result<(), PendingRejectionV1> {
        let mut pending = vec![raw];
        while let Some(raw) = pending.pop() {
            let ty = normalize_type_v1(self.tcx, self.instance, raw)
                .map_err(|_| reject("type that failed monomorphic normalization", site))?;
            if ty.has_param() || ty.has_escaping_bound_vars() {
                return Err(reject("non-monomorphic type", site));
            }
            let identity = rustc_type_identity_v1(self.tcx, ty);
            match self.types.entry(identity) {
                std::collections::btree_map::Entry::Occupied(existing) => {
                    if *existing.get() != ty {
                        return Err(PendingRejectionV1::Fatal(
                            ProductionSemanticPreflightErrorV1::TypeIdentityCollision,
                        ));
                    }
                    continue;
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(ty);
                }
            }
            self.charge(SemanticMirResourceV1::Types, 1)?;
            self.work()?;
            match ty.kind() {
                TyKind::Bool
                | TyKind::Char
                | TyKind::Int(_)
                | TyKind::Uint(_)
                | TyKind::Float(_)
                | TyKind::Never => {}
                TyKind::Tuple(fields) => {
                    for field in fields.iter() {
                        self.queue_type(&mut pending, field)?;
                    }
                }
                TyKind::Adt(_, arguments) | TyKind::FnDef(_, arguments) => {
                    for argument in arguments.iter() {
                        match argument.kind() {
                            GenericArgKind::Type(argument) => {
                                self.queue_type(&mut pending, argument)?;
                            }
                            GenericArgKind::Const(argument)
                                if argument.has_param() || argument.has_escaping_bound_vars() =>
                            {
                                return Err(reject("non-monomorphic const generic argument", site));
                            }
                            GenericArgKind::Const(_) | GenericArgKind::Lifetime(_) => {}
                        }
                    }
                }
                TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => {
                    self.queue_type(&mut pending, *pointee)?;
                }
                TyKind::Array(element, length) => {
                    if length.has_param() || length.has_escaping_bound_vars() {
                        return Err(reject("non-monomorphic array length", site));
                    }
                    self.queue_type(&mut pending, *element)?;
                }
                TyKind::Slice(element) => self.queue_type(&mut pending, *element)?,
                TyKind::Str => return Err(reject("str type", site)),
                TyKind::Pat(..) => return Err(reject("pattern type", site)),
                TyKind::Foreign(..) => return Err(reject("foreign type", site)),
                TyKind::FnPtr(..) => return Err(reject("function-pointer type", site)),
                TyKind::UnsafeBinder(..) => return Err(reject("unsafe-binder type", site)),
                TyKind::Dynamic(..) => return Err(reject("dynamic trait-object type", site)),
                TyKind::Closure(..) => return Err(reject("closure type", site)),
                TyKind::CoroutineClosure(..) => {
                    return Err(reject("coroutine-closure type", site));
                }
                TyKind::Coroutine(..) => return Err(reject("coroutine type", site)),
                TyKind::CoroutineWitness(..) => {
                    return Err(reject("coroutine-witness type", site));
                }
                TyKind::Alias(..) => return Err(reject("unresolved alias type", site)),
                TyKind::Param(..) => return Err(reject("generic parameter type", site)),
                TyKind::Bound(..) => return Err(reject("bound type", site)),
                TyKind::Placeholder(..) => return Err(reject("placeholder type", site)),
                TyKind::Infer(..) => return Err(reject("inference type", site)),
                TyKind::Error(..) => return Err(reject("error type", site)),
            }
        }
        Ok(())
    }

    fn queue_type(
        &self,
        pending: &mut Vec<Ty<'tcx>>,
        ty: Ty<'tcx>,
    ) -> Result<(), PendingRejectionV1> {
        let maximum = self.limits.limit(SemanticMirResourceV1::Types);
        let pending_count = u64::try_from(pending.len()).unwrap_or(u64::MAX);
        if pending_count >= maximum {
            return Err(PendingRejectionV1::Fatal(
                ProductionSemanticPreflightErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::Types,
                    actual: pending_count.saturating_add(1),
                    maximum,
                },
            ));
        }
        pending.push(ty);
        Ok(())
    }

    fn require_local(&self, local: Local, site: RejectionSiteV1) -> Result<(), PendingRejectionV1> {
        if local.index() < self.body.local_decls.len() {
            Ok(())
        } else {
            Err(reject("place references a local outside the body", site))
        }
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
    ) -> Result<(), PendingRejectionV1> {
        self.counts
            .charge(resource, amount, self.limits)
            .map_err(PendingRejectionV1::Fatal)
    }

    fn work(&mut self) -> Result<(), PendingRejectionV1> {
        self.charge(SemanticMirResourceV1::ValidationWork, 1)
    }
}

fn capture_body_sources_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    function: SemanticFunctionIdV1,
    body: &Body<'tcx>,
    cache: &mut HashMap<Span, RetainedSemanticSourceProducerV1>,
    counts: &mut RawMirPreflightCountsV1,
    limits: SemanticMirLimitsV1,
) -> Result<RetainedRawBodySourceProducerV1, PendingRejectionV1> {
    let function_site = RejectionSiteV1 {
        function,
        block: None,
        statement: None,
        local: None,
        span: body.span,
    };
    let source = capture_source_v1(tcx, body.span, function_site, cache, counts, limits)?;

    let mut locals = Vec::with_capacity(body.local_decls.len());
    for (local, declaration) in body.local_decls.iter_enumerated() {
        let site = RejectionSiteV1 {
            function,
            block: None,
            statement: None,
            local: Some(local.index() as u32),
            span: declaration.source_info.span,
        };
        locals.push(capture_source_v1(
            tcx,
            declaration.source_info.span,
            site,
            cache,
            counts,
            limits,
        )?);
    }

    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let Some(terminator) = &data.terminator else {
            return Err(reject(
                "basic block without a terminator",
                RejectionSiteV1 {
                    function,
                    block: Some(block.index() as u32),
                    statement: None,
                    local: None,
                    span: body.span,
                },
            ));
        };
        let mut statements = Vec::with_capacity(data.statements.len());
        for (statement_index, statement) in data.statements.iter().enumerate() {
            let site = RejectionSiteV1 {
                function,
                block: Some(block.index() as u32),
                statement: Some(statement_index as u32),
                local: None,
                span: statement.source_info.span,
            };
            statements.push(capture_source_v1(
                tcx,
                statement.source_info.span,
                site,
                cache,
                counts,
                limits,
            )?);
        }
        let terminator_site = RejectionSiteV1 {
            function,
            block: Some(block.index() as u32),
            statement: None,
            local: None,
            span: terminator.source_info.span,
        };
        let terminator = capture_source_v1(
            tcx,
            terminator.source_info.span,
            terminator_site,
            cache,
            counts,
            limits,
        )?;
        // A basic block has no independent rustc span. Its canonical source is
        // exactly the first statement's producer, or the terminator's producer
        // for an empty block.
        let source = statements.first().copied().unwrap_or(terminator);
        blocks.push(RetainedRawBlockSourceProducerV1 {
            source,
            statements: statements.into_boxed_slice(),
            terminator,
        });
    }
    Ok(RetainedRawBodySourceProducerV1 {
        source,
        locals: locals.into_boxed_slice(),
        blocks: blocks.into_boxed_slice(),
    })
}

fn capture_source_v1(
    tcx: TyCtxt<'_>,
    span: Span,
    site: RejectionSiteV1,
    cache: &mut HashMap<Span, RetainedSemanticSourceProducerV1>,
    counts: &mut RawMirPreflightCountsV1,
    limits: SemanticMirLimitsV1,
) -> Result<RetainedSemanticSourceProducerV1, PendingRejectionV1> {
    if let Some(source) = cache.get(&span).copied() {
        return Ok(source);
    }
    let captured = canonical_source_provenance_v1(tcx, span, MAX_MACRO_EXPANSION_DEPTH_V1)
        .map_err(|error| reject(format!("invalid source provenance: {error}"), site))?;
    counts
        .charge(
            SemanticMirResourceV1::ValidationWork,
            captured.expansion_depth().saturating_add(2),
            limits,
        )
        .map_err(PendingRejectionV1::Fatal)?;
    let source = retained_source_v1(captured);
    cache.insert(span, source);
    Ok(source)
}

const fn retained_source_v1(
    captured: CanonicalSourceProvenanceV1,
) -> RetainedSemanticSourceProducerV1 {
    RetainedSemanticSourceProducerV1 {
        provenance: captured.provenance(),
        expansion_chain_sha256: captured.expansion_chain_sha256(),
    }
}

fn build_canonical_producer_tables_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[RetainedSemanticFunctionProducerV1<'tcx>],
    source_producers: Vec<RetainedRawBodySourceProducerV1>,
    types: BTreeMap<SemanticTypeIdentityV1, Ty<'tcx>>,
) -> Result<CanonicalProducerTablesV1<'tcx>, ProductionSemanticPreflightErrorV1> {
    if source_producers.len() != functions.len() {
        return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
    }
    let mut type_ids = BTreeMap::new();
    let mut type_producers = Vec::with_capacity(types.len());
    for (index, (identity, ty)) in types.into_iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
        type_ids.insert(identity, SemanticTypeIdV1::from_index(index));
        type_producers.push(RetainedSemanticTypeProducerV1 { identity, ty });
    }

    let mut bodies = Vec::with_capacity(functions.len());
    for (function_index, (function, raw_sources)) in
        functions.iter().zip(source_producers).enumerate()
    {
        let function_id = SemanticFunctionIdV1::from_index(
            u32::try_from(function_index)
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?,
        );
        let function_identity = function.identities.function();
        let body = tcx.instance_mir(function.instance.def);
        if raw_sources.locals.len() != body.local_decls.len()
            || raw_sources.blocks.len() != body.basic_blocks.len()
        {
            return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        }
        let mir_body_sha256 = rustc_mir_body_sha256_v1(tcx, function.instance);

        let mut locals = Vec::with_capacity(body.local_decls.len());
        for (raw_local, declaration) in body.local_decls.iter_enumerated() {
            let raw_local = u32::try_from(raw_local.index())
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            let normalized = normalize_type_v1(tcx, function.instance, declaration.ty)
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            let type_identity = rustc_type_identity_v1(tcx, normalized);
            let ty = type_ids
                .get(&type_identity)
                .copied()
                .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            locals.push(RetainedSemanticLocalProducerV1 {
                identity: rustc_local_identity_v1(function_identity, mir_body_sha256, raw_local),
                rustc_local: raw_local,
                ty,
                source: *raw_sources
                    .locals
                    .get(raw_local as usize)
                    .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?,
            });
        }
        locals.sort_unstable_by_key(|local| local.identity);
        if locals
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        }
        let mut raw_to_semantic_locals = vec![None; locals.len()];
        for (semantic_index, local) in locals.iter().enumerate() {
            let semantic_index = u32::try_from(semantic_index)
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            let Some(slot) = raw_to_semantic_locals.get_mut(local.rustc_local as usize) else {
                return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
            };
            if slot
                .replace(SemanticLocalIdV1::from_index(semantic_index))
                .is_some()
            {
                return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
            }
        }
        let raw_to_semantic_locals = raw_to_semantic_locals
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;

        let mut blocks = Vec::with_capacity(body.basic_blocks.len());
        for (raw_block, data) in body.basic_blocks.iter_enumerated() {
            let raw_block = u32::try_from(raw_block.index())
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            let raw_source = raw_sources
                .blocks
                .get(raw_block as usize)
                .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            if raw_source.statements.len() != data.statements.len() {
                return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
            }
            blocks.push(RetainedSemanticBlockProducerV1 {
                identity: rustc_block_identity_v1(function_identity, mir_body_sha256, raw_block),
                rustc_block: raw_block,
                source: raw_source.source,
                statements: raw_source.statements.clone(),
                terminator: raw_source.terminator,
            });
        }
        blocks.sort_unstable_by_key(|block| block.identity);
        if blocks
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        }
        let mut raw_to_semantic_blocks = vec![None; blocks.len()];
        for (semantic_index, block) in blocks.iter().enumerate() {
            let semantic_index = u32::try_from(semantic_index)
                .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
            let Some(slot) = raw_to_semantic_blocks.get_mut(block.rustc_block as usize) else {
                return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
            };
            if slot
                .replace(SemanticBlockIdV1::from_index(semantic_index))
                .is_some()
            {
                return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
            }
        }
        let raw_to_semantic_blocks = raw_to_semantic_blocks
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
        let entry = raw_to_semantic_blocks
            .get(START_BLOCK.index())
            .copied()
            .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;

        bodies.push(RetainedSemanticBodyProducerV1 {
            function: function_id,
            source: raw_sources.source,
            locals: locals.into_boxed_slice(),
            raw_to_semantic_locals: raw_to_semantic_locals.into_boxed_slice(),
            entry,
            blocks: blocks.into_boxed_slice(),
            raw_to_semantic_blocks: raw_to_semantic_blocks.into_boxed_slice(),
        });
    }
    let mut source_files = BTreeSet::new();
    for body in &bodies {
        remember_source_files_v1(&mut source_files, body.source.provenance);
        for local in &body.locals {
            remember_source_files_v1(&mut source_files, local.source.provenance);
        }
        for block in &body.blocks {
            remember_source_files_v1(&mut source_files, block.source.provenance);
            for statement in &block.statements {
                remember_source_files_v1(&mut source_files, statement.provenance);
            }
            remember_source_files_v1(&mut source_files, block.terminator.provenance);
        }
    }
    Ok(CanonicalProducerTablesV1 {
        types: type_producers.into_boxed_slice(),
        source_files: source_files.into_iter().collect(),
        bodies: bodies.into_boxed_slice(),
    })
}

fn remember_source_files_v1(
    files: &mut BTreeSet<SemanticSourceFileIdentityV1>,
    provenance: SemanticSourceProvenanceV1,
) {
    if let Some(origin) = provenance.expansion() {
        files.insert(origin.file());
    }
    if let Some(origin) = provenance.call_site() {
        files.insert(origin.file());
    }
}

fn normalize_type_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    raw: Ty<'tcx>,
) -> Result<Ty<'tcx>, &'static str> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| "type failed monomorphic normalization")
}

fn resolve_direct_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    function: &Operand<'tcx>,
) -> Result<Instance<'tcx>, &'static str> {
    let raw = function.ty(body, tcx);
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| "callable type that failed monomorphic normalization")?;
    let TyKind::FnDef(def_id, arguments) = callable.kind() else {
        return Err("indirect or non-function-definition call");
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, arguments)
        .map_err(|_| "direct call whose concrete rustc instance failed resolution")?
        .ok_or("direct call without a concrete rustc instance")
}

fn remember_rejection(
    first: &mut Option<PendingRejectionV1>,
    construct: impl Into<String>,
    site: RejectionSiteV1,
) {
    if first.is_none() {
        *first = Some(reject(construct, site));
    }
}

fn reject(construct: impl Into<String>, site: RejectionSiteV1) -> PendingRejectionV1 {
    PendingRejectionV1::Unsupported {
        construct: bounded_diagnostic_component_v1(&construct.into()),
        site,
    }
}

fn materialize_rejection_v1(
    tcx: TyCtxt<'_>,
    functions: &[RetainedSemanticFunctionProducerV1<'_>],
    roots: &[SemanticFunctionIdV1],
    edges: &BTreeSet<CallEdgeV1>,
    rejection: PendingRejectionV1,
) -> ProductionSemanticPreflightErrorV1 {
    let (construct, site) = match rejection {
        PendingRejectionV1::Unsupported { construct, site } => (construct, site),
        PendingRejectionV1::Fatal(error) => return error,
    };
    let function = &functions[site.function.index() as usize];
    let function_path =
        bounded_diagnostic_component_v1(&tcx.def_path_str(function.instance.def_id()));
    let call_chain = call_path_v1(roots, edges, site.function)
        .into_iter()
        .map(|id| {
            bounded_diagnostic_component_v1(
                &tcx.def_path_str(functions[id.index() as usize].instance.def_id()),
            )
        })
        .collect::<Box<[_]>>();
    ProductionSemanticPreflightErrorV1::UnsupportedRustcMir {
        construct,
        function: function_path,
        call_chain,
        location: location_diagnostic_v1(site),
        source: source_diagnostic_v1(tcx, site.span),
    }
}

fn call_path_v1(
    roots: &[SemanticFunctionIdV1],
    edges: &BTreeSet<CallEdgeV1>,
    target: SemanticFunctionIdV1,
) -> Vec<SemanticFunctionIdV1> {
    let predecessors = rooted_predecessors_v1(roots, edges);
    if !predecessors.contains_key(&target) {
        return vec![target];
    }
    let mut reverse = vec![target];
    let mut cursor = target;
    while let Some(Some(predecessor)) = predecessors.get(&cursor) {
        reverse.push(*predecessor);
        cursor = *predecessor;
    }
    reverse.reverse();
    reverse
}

fn first_unreachable_function_v1(
    roots: &[SemanticFunctionIdV1],
    edges: &BTreeSet<CallEdgeV1>,
    function_count: usize,
) -> Option<SemanticFunctionIdV1> {
    let predecessors = rooted_predecessors_v1(roots, edges);
    (0..function_count)
        .map(|index| SemanticFunctionIdV1::from_index(index as u32))
        .find(|function| !predecessors.contains_key(function))
}

fn rooted_predecessors_v1(
    roots: &[SemanticFunctionIdV1],
    edges: &BTreeSet<CallEdgeV1>,
) -> BTreeMap<SemanticFunctionIdV1, Option<SemanticFunctionIdV1>> {
    let mut adjacency = BTreeMap::<SemanticFunctionIdV1, Vec<SemanticFunctionIdV1>>::new();
    for edge in edges {
        adjacency.entry(edge.caller).or_default().push(edge.callee);
    }
    let mut predecessors = BTreeMap::new();
    let mut queue = VecDeque::new();
    for root in roots.iter().copied() {
        if predecessors.insert(root, None).is_none() {
            queue.push_back(root);
        }
    }
    while let Some(caller) = queue.pop_front() {
        for callee in adjacency.get(&caller).into_iter().flatten().copied() {
            if let std::collections::btree_map::Entry::Vacant(entry) = predecessors.entry(callee) {
                entry.insert(Some(caller));
                queue.push_back(callee);
            }
        }
    }
    predecessors
}

fn location_diagnostic_v1(site: RejectionSiteV1) -> String {
    match (site.local, site.block, site.statement) {
        (Some(local), _, _) => format!("local {local}"),
        (_, Some(block), Some(statement)) => format!("block {block}, statement {statement}"),
        (_, Some(block), None) => format!("block {block}, terminator"),
        _ => "function body".to_owned(),
    }
}

fn source_diagnostic_v1(tcx: TyCtxt<'_>, span: Span) -> String {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    bounded_diagnostic_component_v1(&format!(
        "{}:{}:{}",
        location
            .file
            .name
            .prefer_remapped_unconditionally()
            .to_string_lossy(),
        location.line,
        location.col.0 + 1,
    ))
}

fn bounded_diagnostic_component_v1(value: &str) -> String {
    value
        .chars()
        .take(MAX_DIAGNOSTIC_COMPONENT_CHARS_V1)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn preflight_plan_sha256_v1<'tcx>(
    target: SemanticTargetDataLayoutV1,
    identity_inventory_sha256: [u8; 32],
    types: &[RetainedSemanticTypeProducerV1<'tcx>],
    source_files: &[SemanticSourceFileIdentityV1],
    functions: &[RetainedSemanticFunctionProducerV1<'tcx>],
    bodies: &[RetainedSemanticBodyProducerV1],
    roots: &[SemanticFunctionIdV1],
    edges: &BTreeSet<CallEdgeV1>,
    terminal_expansions: &[TerminalExpansionRecipeV1],
    counts: RawMirPreflightCountsV1,
    tcx: TyCtxt<'tcx>,
) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(PREFLIGHT_PLAN_DOMAIN_V1);
    digest.field(target.identity().as_bytes());
    digest.field(&identity_inventory_sha256);
    for cardinality in [
        types.len(),
        source_files.len(),
        functions.len(),
        bodies.len(),
        source_provenance_producer_count_v1(bodies),
        roots.len(),
        edges.len(),
        terminal_expansions.len(),
    ] {
        digest.field(&u64::try_from(cardinality).unwrap_or(u64::MAX).to_le_bytes());
    }
    for count in counts.digest_fields() {
        digest.field(&count.to_le_bytes());
    }
    for ty in types {
        digest.field(ty.identity.as_bytes());
        digest.field(rustc_type_identity_v1(tcx, ty.ty).as_bytes());
    }
    for source_file in source_files {
        digest.field(source_file.as_bytes());
    }
    for function in functions {
        digest.field(function.identities.function().as_bytes());
        digest.field(&rustc_mir_body_sha256_v1(tcx, function.instance));
        digest.field(&[function_role_tag_v1(function.role)]);
    }
    for body in bodies {
        digest.field(&body.function.index().to_le_bytes());
        digest_source_producer_v1(&mut digest, body.source);
        for local in &body.locals {
            digest.field(local.identity.as_bytes());
            digest.field(&local.rustc_local.to_le_bytes());
            digest.field(&local.ty.index().to_le_bytes());
            digest_source_producer_v1(&mut digest, local.source);
        }
        for local in &body.raw_to_semantic_locals {
            digest.field(&local.index().to_le_bytes());
        }
        digest.field(&body.entry.index().to_le_bytes());
        for block in &body.blocks {
            digest.field(block.identity.as_bytes());
            digest.field(&block.rustc_block.to_le_bytes());
            digest_source_producer_v1(&mut digest, block.source);
            for statement in &block.statements {
                digest_source_producer_v1(&mut digest, *statement);
            }
            digest_source_producer_v1(&mut digest, block.terminator);
        }
        for block in &body.raw_to_semantic_blocks {
            digest.field(&block.index().to_le_bytes());
        }
    }
    for root in roots {
        digest.field(&root.index().to_le_bytes());
    }
    for edge in edges {
        digest.field(&edge.caller.index().to_le_bytes());
        digest.field(&edge.callee.index().to_le_bytes());
    }
    for recipe in terminal_expansions {
        digest.field(&recipe.caller.index().to_le_bytes());
        digest.field(&recipe.block.to_le_bytes());
        digest.field(&[terminal_expansion_tag_v1(recipe.expansion)]);
        digest.field(&recipe.arguments.to_le_bytes());
    }
    digest.finish()
}

fn source_provenance_producer_count_v1(bodies: &[RetainedSemanticBodyProducerV1]) -> usize {
    bodies
        .iter()
        .map(|body| {
            1 + body.locals.len()
                + body
                    .blocks
                    .iter()
                    .map(|block| 2 + block.statements.len())
                    .sum::<usize>()
        })
        .sum()
}

fn digest_source_producer_v1(
    digest: &mut SemanticIdentityDigestV1,
    source: RetainedSemanticSourceProducerV1,
) {
    digest.field(&source.expansion_chain_sha256);
    digest_source_origin_v1(digest, source.provenance.expansion());
    digest_source_origin_v1(digest, source.provenance.call_site());
}

fn digest_source_origin_v1(
    digest: &mut SemanticIdentityDigestV1,
    origin: Option<SemanticSourceOriginV1>,
) {
    let Some(origin) = origin else {
        digest.field(&[0]);
        return;
    };
    digest.field(&[1]);
    digest.field(origin.file().as_bytes());
    let (byte_start, byte_end) = origin.byte_range();
    digest.field(&byte_start.to_le_bytes());
    digest.field(&byte_end.to_le_bytes());
    let (line_start, column_start) = origin.start_coordinate();
    digest.field(&line_start.to_le_bytes());
    digest.field(&column_start.to_le_bytes());
    let (line_end, column_end) = origin.end_coordinate();
    digest.field(&line_end.to_le_bytes());
    digest.field(&column_end.to_le_bytes());
}

const fn function_role_tag_v1(role: CollectedFunctionRole) -> u8 {
    match role {
        CollectedFunctionRole::KernelEntry => 0,
        CollectedFunctionRole::InternalHelper => 1,
        CollectedFunctionRole::DeviceFfiExport => 2,
    }
}

const fn terminal_expansion_tag_v1(expansion: ProductionTerminalExpansionV1) -> u8 {
    match expansion {
        ProductionTerminalExpansionV1::ThreadIndex1d => 0,
        ProductionTerminalExpansionV1::ThreadIndexGet => 1,
        ProductionTerminalExpansionV1::DisjointSliceGetMut => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_count_budget_rejects_overflow_before_record_construction() {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Statements, 2)
            .unwrap();
        let mut counts = RawMirPreflightCountsV1::default();
        counts
            .charge(SemanticMirResourceV1::Statements, 2, limits)
            .unwrap();
        assert!(matches!(
            counts.charge(SemanticMirResourceV1::Statements, 1, limits),
            Err(ProductionSemanticPreflightErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Statements,
                actual: 3,
                maximum: 2,
            })
        ));
    }

    #[test]
    fn deterministic_call_path_uses_sorted_roots_and_edges() {
        let id = SemanticFunctionIdV1::from_index;
        let edges = BTreeSet::from([
            CallEdgeV1 {
                caller: id(2),
                callee: id(3),
            },
            CallEdgeV1 {
                caller: id(0),
                callee: id(2),
            },
            CallEdgeV1 {
                caller: id(1),
                callee: id(3),
            },
        ]);
        assert_eq!(call_path_v1(&[id(0), id(1)], &edges, id(3)), [id(1), id(3)]);
        assert_eq!(call_path_v1(&[id(0)], &edges, id(3)), [id(0), id(2), id(3)]);
        assert_eq!(call_path_v1(&[id(0)], &edges, id(9)), [id(9)]);
        assert_eq!(
            first_unreachable_function_v1(&[id(0)], &edges, 4),
            Some(id(1))
        );
        assert_eq!(
            first_unreachable_function_v1(&[id(0), id(1)], &edges, 4),
            None,
        );
    }

    #[test]
    fn terminal_recipe_tags_are_closed_and_distinct() {
        let tags = [
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::ThreadIndex1d),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::ThreadIndexGet),
            terminal_expansion_tag_v1(ProductionTerminalExpansionV1::DisjointSliceGetMut),
        ];
        assert_eq!(tags, [0, 1, 2]);
    }

    #[test]
    fn diagnostics_are_bounded_by_unicode_scalar_count() {
        let bounded = bounded_diagnostic_component_v1(&"x".repeat(1_024));
        assert_eq!(bounded.len(), MAX_DIAGNOSTIC_COMPONENT_CHARS_V1);
    }
}
