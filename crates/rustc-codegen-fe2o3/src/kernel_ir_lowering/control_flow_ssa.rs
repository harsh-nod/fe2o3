//! Bounded local-to-block-parameter promotion for structured control flow.

use super::{
    MirFunction, MirOperandRef, MirPlaceRef, MirRvalueKind, MirStatementKind, MirTerminatorKind,
    MirTypeShape, TranslationDiagnostic, TranslationDiagnosticCode, TranslationLocation, Type,
    diagnostic, lower_scalar_type,
};
use crate::mir_import::MirLocalRole;
use fe2o3_kernel_ir::ScalarType;
use std::collections::{BTreeMap, BTreeSet};

const MAX_CONTROL_FLOW_BLOCKS_V1: usize = 128;
const MAX_PROMOTED_LOCALS_V1: usize = 64;
const MAX_BLOCK_PARAMETERS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PromotedLocalKind {
    Scalar,
    FieldlessEnum,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ControlFlowSsaPlan {
    promoted: BTreeMap<usize, (PromotedLocalKind, Type)>,
    live_in: BTreeMap<usize, Vec<usize>>,
}

impl ControlFlowSsaPlan {
    pub(super) fn analyze(
        function: &MirFunction,
        gfx942: bool,
    ) -> Result<Self, TranslationDiagnostic> {
        if function.blocks.len() > MAX_CONTROL_FLOW_BLOCKS_V1 {
            return Err(reject(
                function,
                format!(
                    "bounded gfx942 control flow supports at most {MAX_CONTROL_FLOW_BLOCKS_V1} MIR blocks; found {}",
                    function.blocks.len()
                ),
            ));
        }

        let local_shapes = function
            .locals
            .iter()
            .map(|local| (local.index, (&local.ty.shape, local.role)))
            .collect::<BTreeMap<_, _>>();
        let mut assignments = BTreeMap::<usize, Vec<MirRvalueKind>>::new();
        let mut projected = BTreeSet::new();
        for block in &function.blocks {
            for statement in &block.statements {
                for operand in &statement.operands {
                    inspect_operand_projection(operand, &mut projected);
                }
                if statement.kind != MirStatementKind::Assign {
                    continue;
                }
                let Some(destination) = &statement.destination else {
                    continue;
                };
                if !destination.projection.is_empty() {
                    projected.insert(destination.local);
                    continue;
                }
                if let Some(rvalue) = statement.rvalue {
                    assignments
                        .entry(destination.local)
                        .or_default()
                        .push(rvalue);
                }
            }
            inspect_terminator_projections(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                &mut projected,
            );
        }

        let mut promoted = BTreeMap::new();
        for (local, rvalues) in assignments {
            let Some((shape, role)) = local_shapes.get(&local).copied() else {
                return Err(reject(
                    function,
                    format!("local{local} has no imported type"),
                ));
            };
            let is_mutable = rvalues.len() > 1 || role == MirLocalRole::Arg;
            if !is_mutable || projected.contains(&local) {
                continue;
            }
            let promoted_type = if let Some(ty) = lower_scalar_type(shape) {
                Some((PromotedLocalKind::Scalar, ty))
            } else if matches!(shape, MirTypeShape::Adt { .. })
                && rvalues
                    .iter()
                    .all(|rvalue| matches!(rvalue, MirRvalueKind::FieldlessEnumVariant(_)))
            {
                Some((
                    PromotedLocalKind::FieldlessEnum,
                    Type::Scalar(ScalarType::I64),
                ))
            } else {
                None
            };
            if let Some(promoted_type) = promoted_type {
                promoted.insert(local, promoted_type);
            }
        }

        if promoted.is_empty() {
            return Ok(Self::default());
        }
        if !gfx942 {
            return Err(reject(
                function,
                "bounded mutable control flow is supported only for the exact gfx942 target profile",
            ));
        }
        if promoted.len() > MAX_PROMOTED_LOCALS_V1 {
            return Err(reject(
                function,
                format!(
                    "bounded gfx942 control flow promotes at most {MAX_PROMOTED_LOCALS_V1} locals; found {}",
                    promoted.len()
                ),
            ));
        }

        let block_ids = function
            .blocks
            .iter()
            .map(|block| block.index)
            .collect::<BTreeSet<_>>();
        let mut use_sets = BTreeMap::<usize, BTreeSet<usize>>::new();
        let mut def_sets = BTreeMap::<usize, BTreeSet<usize>>::new();
        let mut successors = BTreeMap::<usize, Vec<usize>>::new();
        for block in &function.blocks {
            let mut uses = BTreeSet::new();
            let mut defs = BTreeSet::new();
            for statement in &block.statements {
                for operand in &statement.operands {
                    collect_operand_uses(operand, &promoted, &defs, &mut uses);
                }
                if statement.kind == MirStatementKind::Assign
                    && let Some(destination) = &statement.destination
                    && destination.projection.is_empty()
                    && promoted.contains_key(&destination.local)
                {
                    defs.insert(destination.local);
                }
            }
            let Some(terminator) = &block.terminator else {
                return Err(reject(
                    function,
                    format!("bb{} has no terminator", block.index),
                ));
            };
            collect_terminator_uses(&terminator.kind, &promoted, &defs, &mut uses);
            let targets = terminator_successors(&terminator.kind);
            for target in &targets {
                if !block_ids.contains(target) {
                    return Err(reject(
                        function,
                        format!("bb{} references missing bb{target}", block.index),
                    ));
                }
            }
            successors.insert(block.index, targets);
            use_sets.insert(block.index, uses);
            def_sets.insert(block.index, defs);
        }

        let mut live_in = function
            .blocks
            .iter()
            .map(|block| (block.index, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut changed = true;
        while changed {
            changed = false;
            for block in function.blocks.iter().rev() {
                let live_out = successors[&block.index]
                    .iter()
                    .flat_map(|target| live_in[target].iter().copied())
                    .collect::<BTreeSet<_>>();
                let mut next = use_sets[&block.index].clone();
                next.extend(live_out.difference(&def_sets[&block.index]).copied());
                if next != live_in[&block.index] {
                    live_in.insert(block.index, next);
                    changed = true;
                }
            }
        }

        if live_in.get(&0).is_some_and(|locals| {
            locals.iter().any(|local| {
                local_shapes
                    .get(local)
                    .is_none_or(|(_, role)| *role != MirLocalRole::Arg)
            })
        }) {
            return Err(reject(
                function,
                "bounded control flow reads a mutable local before its entry definition",
            ));
        }

        let parameter_count = live_in
            .iter()
            .filter(|(block, _)| **block != 0)
            .map(|(_, locals)| locals.len())
            .sum::<usize>();
        if parameter_count > MAX_BLOCK_PARAMETERS_V1 {
            return Err(reject(
                function,
                format!(
                    "bounded gfx942 control flow requires {parameter_count} block parameters, exceeding {MAX_BLOCK_PARAMETERS_V1}"
                ),
            ));
        }

        for block in &function.blocks {
            let mut seen = BTreeSet::new();
            for target in &successors[&block.index] {
                if !seen.insert(*target)
                    && live_in.get(target).is_some_and(|locals| !locals.is_empty())
                {
                    return Err(reject(
                        function,
                        format!(
                            "bb{} has multiple live-value edges to bb{target}; bounded gfx942 SSA requires one edge per predecessor",
                            block.index
                        ),
                    ));
                }
            }
        }

        Ok(Self {
            promoted,
            live_in: live_in
                .into_iter()
                .map(|(block, locals)| (block, locals.into_iter().collect()))
                .collect(),
        })
    }

    pub(super) fn promoted_locals(&self) -> impl Iterator<Item = usize> + '_ {
        self.promoted.keys().copied()
    }

    pub(super) fn is_promoted(&self, local: usize) -> bool {
        self.promoted.contains_key(&local)
    }

    pub(super) fn kind(&self, local: usize) -> Option<PromotedLocalKind> {
        self.promoted.get(&local).map(|(kind, _)| *kind)
    }

    pub(super) fn ty(&self, local: usize) -> Option<&Type> {
        self.promoted.get(&local).map(|(_, ty)| ty)
    }

    pub(super) fn live_in(&self, block: usize) -> &[usize] {
        self.live_in.get(&block).map_or(&[], Vec::as_slice)
    }
}

fn reject(function: &MirFunction, message: impl Into<String>) -> TranslationDiagnostic {
    diagnostic(
        TranslationDiagnosticCode::UnsupportedStatement,
        TranslationLocation::function(function),
        message,
    )
}

fn inspect_operand_projection(operand: &MirOperandRef, projected: &mut BTreeSet<usize>) {
    if let MirOperandRef::Place(place) = operand
        && !place.projection.is_empty()
    {
        projected.insert(place.local);
    }
}

fn inspect_terminator_projections(
    terminator: Option<&MirTerminatorKind>,
    projected: &mut BTreeSet<usize>,
) {
    let Some(terminator) = terminator else {
        return;
    };
    for operand in terminator_operands(terminator) {
        inspect_operand_projection(operand, projected);
    }
}

fn collect_operand_uses(
    operand: &MirOperandRef,
    promoted: &BTreeMap<usize, (PromotedLocalKind, Type)>,
    defs: &BTreeSet<usize>,
    uses: &mut BTreeSet<usize>,
) {
    if let MirOperandRef::Place(MirPlaceRef { local, .. }) = operand
        && promoted.contains_key(local)
        && !defs.contains(local)
    {
        uses.insert(*local);
    }
}

fn collect_terminator_uses(
    terminator: &MirTerminatorKind,
    promoted: &BTreeMap<usize, (PromotedLocalKind, Type)>,
    defs: &BTreeSet<usize>,
    uses: &mut BTreeSet<usize>,
) {
    for operand in terminator_operands(terminator) {
        collect_operand_uses(operand, promoted, defs, uses);
    }
}

fn terminator_operands(terminator: &MirTerminatorKind) -> Vec<&MirOperandRef> {
    match terminator {
        MirTerminatorKind::SwitchInt { discriminant, .. } => vec![discriminant],
        MirTerminatorKind::Call { operands, .. } => operands.iter().collect(),
        MirTerminatorKind::Assert { condition, .. } => vec![condition],
        MirTerminatorKind::Return
        | MirTerminatorKind::Unreachable
        | MirTerminatorKind::Goto { .. }
        | MirTerminatorKind::Drop { .. }
        | MirTerminatorKind::Other => Vec::new(),
    }
}

pub(super) fn terminator_successors(terminator: &MirTerminatorKind) -> Vec<usize> {
    match terminator {
        MirTerminatorKind::Goto { target } => vec![*target],
        MirTerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => targets
            .iter()
            .map(|target| target.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        MirTerminatorKind::Call { target, .. } => target.iter().copied().collect(),
        MirTerminatorKind::Assert { target, .. } | MirTerminatorKind::Drop { target } => {
            vec![*target]
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable | MirTerminatorKind::Other => {
            Vec::new()
        }
    }
}
