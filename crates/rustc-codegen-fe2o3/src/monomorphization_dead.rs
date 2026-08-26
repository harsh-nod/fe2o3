//! Compiler-private authority for V1 monomorphization-dead branch exclusion.
//!
//! The portable evidence type is caller-constructible and inert. Only this
//! module can derive the private observation from the active rustc body, and
//! every consumer recomputes it before excluding a block.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fe2o3_rustc_front::{
    CONSTANT_FOLD_POLICY_VERSION_V1, ConstantSwitchCaseV1, ConstantSwitchV1, DeadBranchContextV1,
    FixedWidthIntegerV1, MonomorphizationDeadEvidenceV1, prove_constant_switch_v1,
};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::mir::{Body, Operand, Rvalue, StatementKind, TerminatorKind};
use rustc_middle::ty::{
    ConstKind, EarlyBinder, Instance, IntTy, TyCtxt, TyKind, TypingEnv, UintTy,
};
use rustc_span::Span;
use sha2::{Digest as _, Sha256};

const FUNCTION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.monomorphization-dead.function.v1\0";
const CFG_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.monomorphization-dead.cfg.v1\0";
const SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.monomorphization-dead.source.v1\0";
const TARGET_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.monomorphization-dead.target.v1\0";
const MAX_LOCAL_CONST_SCAN_STATEMENTS_V1: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerDeadBranchObservationV1 {
    evidence: MonomorphizationDeadEvidenceV1,
    selected_successors: BTreeMap<usize, usize>,
    excluded_blocks: BTreeSet<usize>,
    policy_reachable_blocks: BTreeSet<usize>,
}

impl CompilerDeadBranchObservationV1 {
    pub(crate) fn observe<'tcx>(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        body: &Body<'tcx>,
    ) -> Result<Self, DeadBranchObservationError> {
        let context = DeadBranchContextV1::new(
            function_identity(tcx, instance),
            cfg_identity(tcx, body),
            source_identity(tcx, instance, body)?,
            target_identity(tcx),
        )
        .map_err(|error| DeadBranchObservationError::new(error.to_string()))?;

        let mut decisions = Vec::new();
        for (block, data) in body.basic_blocks.iter_enumerated() {
            let Some(terminator) = &data.terminator else {
                return Err(DeadBranchObservationError::new(format!(
                    "MIR bb{} has no terminator",
                    block.as_usize()
                )));
            };
            let TerminatorKind::SwitchInt { discr, targets } = &terminator.kind else {
                continue;
            };
            let Some(discriminant) = fixed_operand(tcx, instance, body, block, discr) else {
                continue;
            };
            let branch_block = u32::try_from(block.as_usize())
                .map_err(|_| DeadBranchObservationError::new("MIR block identity exceeds u32"))?;
            let mut cases = Vec::with_capacity(targets.iter().len());
            let mut representable = true;
            for (value, target) in targets.iter() {
                let Ok(value) =
                    FixedWidthIntegerV1::new(discriminant.width(), discriminant.is_signed(), value)
                else {
                    representable = false;
                    break;
                };
                let Ok(target) = u32::try_from(target.as_usize()) else {
                    representable = false;
                    break;
                };
                cases.push(ConstantSwitchCaseV1::new(value, target));
            }
            let Ok(otherwise) = u32::try_from(targets.otherwise().as_usize()) else {
                continue;
            };
            if !representable {
                continue;
            }
            let Ok(switch) =
                ConstantSwitchV1::new(branch_block, discriminant.into(), cases, otherwise)
            else {
                continue;
            };
            if let Ok(decision) = prove_constant_switch_v1(CONSTANT_FOLD_POLICY_VERSION_V1, &switch)
            {
                decisions.push(decision);
            }
        }

        let evidence = MonomorphizationDeadEvidenceV1::new(
            CONSTANT_FOLD_POLICY_VERSION_V1,
            context,
            decisions,
        )
        .map_err(|error| DeadBranchObservationError::new(error.to_string()))?;
        let selected_successors = evidence
            .decisions()
            .iter()
            .map(|decision| {
                (
                    decision.branch_block() as usize,
                    decision.selected_successor() as usize,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let successors = body_successors(body)?;
        let excluded_blocks = excluded_blocks(&successors, &selected_successors);
        let policy_reachable_blocks = reachable_blocks(&successors, &selected_successors);
        Ok(Self {
            evidence,
            selected_successors,
            excluded_blocks,
            policy_reachable_blocks,
        })
    }

    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    pub(crate) fn validate_against<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        body: &Body<'tcx>,
    ) -> Result<(), DeadBranchObservationError> {
        let actual = Self::observe(tcx, instance, body)?;
        if self != &actual {
            return Err(DeadBranchObservationError::new(
                "stored dead-branch observation disagrees with active rustc MIR",
            ));
        }
        Ok(())
    }

    pub(crate) fn includes_block(&self, block: usize) -> bool {
        !self.excluded_blocks.contains(&block)
    }

    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    pub(crate) fn selected_successor(&self, block: usize) -> Option<usize> {
        self.selected_successors.get(&block).copied()
    }

    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    pub(crate) fn imports_block(&self, block: usize) -> bool {
        self.policy_reachable_blocks.contains(&block)
    }

    pub(crate) fn excluded_blocks(&self) -> &BTreeSet<usize> {
        &self.excluded_blocks
    }

    pub(crate) fn evidence(&self) -> &MonomorphizationDeadEvidenceV1 {
        &self.evidence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeadBranchObservationError {
    message: String,
}

impl DeadBranchObservationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DeadBranchObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DeadBranchObservationError {}

fn fixed_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    block: rustc_middle::mir::BasicBlock,
    operand: &Operand<'tcx>,
) -> Option<FixedWidthIntegerV1> {
    match operand {
        Operand::Constant(constant) => fixed_constant(tcx, instance, constant),
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            fixed_monomorphized_local(tcx, instance, body, block, place.local)
        }
        Operand::Copy(_) | Operand::Move(_) => None,
        Operand::RuntimeChecks(_) => None,
    }
}

fn fixed_monomorphized_local<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    block: rustc_middle::mir::BasicBlock,
    local: rustc_middle::mir::Local,
) -> Option<FixedWidthIntegerV1> {
    if local.as_usize() <= body.arg_count
        || body
            .basic_blocks
            .iter()
            .map(|data| data.statements.len())
            .sum::<usize>()
            > MAX_LOCAL_CONST_SCAN_STATEMENTS_V1
    {
        return None;
    }

    let mut source = None;
    for (candidate_block, data) in body.basic_blocks.iter_enumerated() {
        for statement in &data.statements {
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (destination, rvalue) = &**assignment;
                    if rvalue_borrows_local(rvalue, local) {
                        return None;
                    }
                    if destination.local != local {
                        continue;
                    }
                    if !destination.projection.is_empty()
                        || candidate_block != block
                        || source.is_some()
                    {
                        return None;
                    }
                    let Rvalue::Use(Operand::Constant(constant)) = rvalue else {
                        return None;
                    };
                    if !matches!(
                        constant.const_,
                        rustc_middle::mir::Const::Ty(_, value)
                            if matches!(value.kind(), ConstKind::Param(_))
                    ) {
                        return None;
                    }
                    source = Some(constant);
                }
                StatementKind::SetDiscriminant { place, .. } if place.local == local => {
                    return None;
                }
                _ => {}
            }
        }
    }
    fixed_constant(tcx, instance, source?)
}

fn rvalue_borrows_local(rvalue: &Rvalue<'_>, local: rustc_middle::mir::Local) -> bool {
    matches!(
        rvalue,
        Rvalue::Ref(_, _, place) | Rvalue::RawPtr(_, place) if place.local == local
    )
}

fn fixed_constant<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    constant: &rustc_middle::mir::ConstOperand<'tcx>,
) -> Option<FixedWidthIntegerV1> {
    let typing_env = TypingEnv::fully_monomorphized();
    let constant = tcx.instantiate_and_normalize_erasing_regions(
        instance.args,
        typing_env,
        EarlyBinder::bind(constant.const_),
    );
    let (width, signed) = fixed_integer_type(constant.ty())?;
    let bits = constant.try_eval_bits(tcx, typing_env)?;
    FixedWidthIntegerV1::new(width, signed, bits).ok()
}

fn fixed_integer_type(ty: rustc_middle::ty::Ty<'_>) -> Option<(u16, bool)> {
    match ty.kind() {
        TyKind::Bool => Some((1, false)),
        TyKind::Int(IntTy::I8) => Some((8, true)),
        TyKind::Int(IntTy::I16) => Some((16, true)),
        TyKind::Int(IntTy::I32) => Some((32, true)),
        TyKind::Int(IntTy::I64) => Some((64, true)),
        TyKind::Int(IntTy::I128) => Some((128, true)),
        TyKind::Uint(UintTy::U8) => Some((8, false)),
        TyKind::Uint(UintTy::U16) => Some((16, false)),
        TyKind::Uint(UintTy::U32) => Some((32, false)),
        TyKind::Uint(UintTy::U64) => Some((64, false)),
        TyKind::Uint(UintTy::U128) => Some((128, false)),
        TyKind::Int(IntTy::Isize) | TyKind::Uint(UintTy::Usize) => None,
        _ => None,
    }
}

fn body_successors(body: &Body<'_>) -> Result<Vec<Vec<usize>>, DeadBranchObservationError> {
    body.basic_blocks
        .iter()
        .enumerate()
        .map(|(index, data)| {
            let terminator = data.terminator.as_ref().ok_or_else(|| {
                DeadBranchObservationError::new(format!("MIR bb{index} has no terminator"))
            })?;
            Ok(terminator
                .successors()
                .map(|successor| successor.as_usize())
                .collect())
        })
        .collect()
}

fn excluded_blocks(
    successors: &[Vec<usize>],
    selected_successors: &BTreeMap<usize, usize>,
) -> BTreeSet<usize> {
    let original = reachable_blocks(successors, &BTreeMap::new());
    let folded = reachable_blocks(successors, selected_successors);
    original.difference(&folded).copied().collect()
}

fn reachable_blocks(
    successors: &[Vec<usize>],
    selected_successors: &BTreeMap<usize, usize>,
) -> BTreeSet<usize> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![0_usize];
    while let Some(block) = pending.pop() {
        if block >= successors.len() || !reachable.insert(block) {
            continue;
        }
        if let Some(selected) = selected_successors.get(&block) {
            pending.push(*selected);
        } else {
            pending.extend(successors[block].iter().copied());
        }
    }
    reachable
}

fn function_identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FUNCTION_IDENTITY_DOMAIN_V1);
    append_bytes(
        &mut hasher,
        tcx.def_path_hash(instance.def_id())
            .0
            .to_le_bytes()
            .as_slice(),
    );
    append_bytes(&mut hasher, tcx.symbol_name(instance).name.as_bytes());
    hasher.finalize().into()
}

fn cfg_identity<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 32] {
    // rustc's structured HashStable implementation covers the complete MIR
    // body, including declarations, statements, operands, and terminators.
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut stable_hasher = StableHasher::new();
        body.hash_stable(&mut context, &mut stable_hasher);
        stable_hasher.finish()
    });
    let mut hasher = Sha256::new();
    hasher.update(CFG_IDENTITY_DOMAIN_V1);
    hasher.update(fingerprint.to_le_bytes());
    hasher.finalize().into()
}

fn target_identity(tcx: TyCtxt<'_>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_IDENTITY_DOMAIN_V1);
    append_bytes(&mut hasher, tcx.sess.target.llvm_target.as_bytes());
    append_bytes(&mut hasher, tcx.sess.target.data_layout.as_bytes());
    hasher.update(tcx.sess.target.pointer_width.to_le_bytes());
    hasher.finalize().into()
}

fn source_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Result<[u8; 32], DeadBranchObservationError> {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_IDENTITY_DOMAIN_V1);
    append_span(&mut hasher, tcx, tcx.def_span(instance.def_id()))?;
    append_usize(&mut hasher, body.basic_blocks.len())?;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        append_usize(&mut hasher, block.as_usize())?;
        append_usize(&mut hasher, data.statements.len())?;
        for statement in &data.statements {
            append_span(&mut hasher, tcx, statement.source_info.span)?;
        }
        let terminator = data.terminator.as_ref().ok_or_else(|| {
            DeadBranchObservationError::new(format!("MIR bb{} has no terminator", block.as_usize()))
        })?;
        append_span(&mut hasher, tcx, terminator.source_info.span)?;
    }
    Ok(hasher.finalize().into())
}

fn append_span(
    hasher: &mut Sha256,
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<(), DeadBranchObservationError> {
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_char_pos(span.lo());
    let end = source_map.lookup_char_pos(span.hi());
    let file = start
        .file
        .name
        .prefer_remapped_unconditionally()
        .to_string_lossy();
    append_bytes(hasher, file.as_bytes());
    append_usize(hasher, start.line)?;
    append_usize(hasher, start.col.0)?;
    append_usize(hasher, end.line)?;
    append_usize(hasher, end.col.0)
}

fn append_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn append_usize(hasher: &mut Sha256, value: usize) -> Result<(), DeadBranchObservationError> {
    let value = u64::try_from(value)
        .map_err(|_| DeadBranchObservationError::new("identity field exceeds u64"))?;
    hasher.update(value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{excluded_blocks, reachable_blocks};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn exclusion_requires_loss_of_original_entry_reachability() {
        let graph = vec![
            vec![1, 2],
            vec![3],
            vec![3],
            vec![],
            vec![], // Structurally unreachable blocks remain subject to analysis.
        ];
        let selected = BTreeMap::from([(0, 1)]);
        assert_eq!(excluded_blocks(&graph, &selected), BTreeSet::from([2]));
        assert!(reachable_blocks(&graph, &selected).contains(&3));
        assert!(!excluded_blocks(&graph, &selected).contains(&4));
    }

    #[test]
    fn chained_constant_decisions_remove_only_their_closed_region() {
        let graph = vec![vec![1, 2], vec![3, 4], vec![5], vec![5], vec![5], vec![]];
        let selected = BTreeMap::from([(0, 1), (1, 4)]);
        assert_eq!(excluded_blocks(&graph, &selected), BTreeSet::from([2, 3]));
        assert!(reachable_blocks(&graph, &selected).contains(&5));
    }

    #[test]
    fn policy_reachable_import_set_closes_unreachable_to_excluded_edges() {
        let graph = vec![vec![1, 2], vec![], vec![], vec![2]];
        let selected = BTreeMap::from([(0, 1)]);
        let retained = reachable_blocks(&graph, &selected);

        assert_eq!(excluded_blocks(&graph, &selected), BTreeSet::from([2]));
        assert_eq!(retained, BTreeSet::from([0, 1]));
        assert!(!retained.contains(&3));
        for block in &retained {
            let successors = selected
                .get(block)
                .map_or_else(|| graph[*block].clone(), |target| vec![*target]);
            assert!(successors.iter().all(|target| retained.contains(target)));
        }
    }
}
