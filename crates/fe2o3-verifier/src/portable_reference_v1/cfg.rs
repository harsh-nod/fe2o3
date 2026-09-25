//! Shared reference CFG predicates; no source authentication.

use super::*;

pub(super) fn reference_block_path_predicates_v1(
    meter: &impl ReferenceWorkV1,
    effect_ir: &ReferenceEffectIrV1,
) -> Result<Vec<ReferencePathPredicateV1>, ReferenceBindingErrorV1> {
    let block_count = effect_ir.blocks.len();
    meter.charge(1)?;
    if block_count == 0 {
        return Err(ReferenceBindingErrorV1::new(
            "reference effect IR has no entry block",
        ));
    }
    for (index, block) in effect_ir.blocks.iter().enumerate() {
        meter.charge(1)?;
        if block.block as usize != index {
            return Err(ReferenceBindingErrorV1::new(
                "reference effect block identities are not contiguous",
            ));
        }
    }
    let resolver = ReferenceExpressionResolverV1::new(meter, effect_ir)?;
    meter.rows::<BTreeSet<usize>>(block_count)?;
    meter.rows::<usize>(block_count)?;
    let mut successors = vec![BTreeSet::new(); block_count];
    let mut indegree = vec![0_usize; block_count];
    for block in &effect_ir.blocks {
        meter.charge(1)?;
        for target in reference_successors_v1(&block.terminator) {
            meter.charge(1)?;
            meter.tree::<usize>(successors[block.block as usize].len())?;
            let target_index = target as usize;
            if target_index >= block_count {
                return Err(ReferenceBindingErrorV1::new(
                    "reference effect terminator target is out of bounds",
                ));
            }
            if successors[block.block as usize].insert(target_index) {
                indegree[target_index] =
                    indegree[target_index].checked_add(1).ok_or_else(|| {
                        ReferenceBindingErrorV1::new("reference effect CFG indegree overflowed")
                    })?;
            }
        }
    }
    meter.rows::<usize>(indegree.len())?;
    let mut pending = indegree
        .iter()
        .enumerate()
        .filter_map(|(block, degree)| (*degree == 0).then_some(block))
        .collect::<VecDeque<_>>();
    meter.rows::<ReferencePathPredicateV1>(block_count)?;
    meter.rows::<ReferenceGuardClauseV1>(1)?;
    let mut predicates = vec![ReferencePathPredicateV1::unreachable_v1(); block_count];
    predicates[0] = ReferencePathPredicateV1::unconditional_v1();
    let mut visited = 0_usize;
    while let Some(block_index) = pending.pop_front() {
        meter.charge(1)?;
        visited += 1;
        meter.clone_predicate(&predicates[block_index])?;
        let source = predicates[block_index].clone();
        for (target, atom) in
            reference_guarded_edges_v1(meter, &effect_ir.blocks[block_index].terminator, &resolver)?
        {
            let contribution = match atom {
                Some(atom) => reference_predicate_and_atom_v1(meter, &source, atom)?,
                None => {
                    meter.clone_predicate(&source)?;
                    source.clone()
                }
            };
            reference_predicate_or_assign_v1(
                meter,
                &mut predicates[target as usize],
                contribution,
            )?;
        }
        for target in &successors[block_index] {
            meter.charge(1)?;
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                meter.grow::<usize>(pending.len())?;
                pending.push_back(*target);
            }
        }
    }
    if visited != block_count {
        return Err(ReferenceBindingErrorV1::new(
            "reference effect CFG contains a cycle after MIR authentication",
        ));
    }
    Ok(predicates)
}

pub fn reference_successors_v1(
    terminator: &ReferenceTerminatorV1,
) -> impl Iterator<Item = u32> + '_ {
    let (values, tail): (&[(u128, u32)], Option<u32>) = match terminator {
        ReferenceTerminatorV1::Return => (&[], None),
        ReferenceTerminatorV1::Goto { target } => (&[], Some(*target)),
        ReferenceTerminatorV1::Switch {
            values, otherwise, ..
        } => (values, Some(*otherwise)),
        ReferenceTerminatorV1::Assert { success, .. } => (&[], Some(*success)),
    };
    values.iter().map(|(_, target)| *target).chain(tail)
}

pub(super) fn reference_guarded_edges_v1(
    meter: &impl ReferenceWorkV1,
    terminator: &ReferenceTerminatorV1,
    resolver: &ReferenceExpressionResolverV1<'_>,
) -> Result<Vec<(u32, Option<ReferenceGuardAtomV1>)>, ReferenceBindingErrorV1> {
    meter.rows::<(u32, Option<ReferenceGuardAtomV1>)>(1)?;
    match terminator {
        ReferenceTerminatorV1::Return => Ok(Vec::new()),
        ReferenceTerminatorV1::Goto { target } => Ok(vec![(*target, None)]),
        ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success,
            bounds_check,
        } => Ok(vec![(
            *success,
            if bounds_check.is_some() {
                None
            } else {
                Some(ReferenceGuardAtomV1::Assert {
                    condition: resolver.resolve_operand_inner_v1(
                        meter,
                        condition,
                        &mut BTreeSet::new(),
                        &mut 0,
                        1,
                    )?,
                    expected: *expected,
                })
            },
        )]),
        ReferenceTerminatorV1::Switch {
            discriminant,
            values,
            otherwise,
        } => {
            let expression = resolver.resolve_operand_inner_v1(
                meter,
                discriminant,
                &mut BTreeSet::new(),
                &mut 0,
                1,
            )?;
            let mut by_target = BTreeMap::<u32, Vec<u128>>::new();
            meter.rows::<u128>(values.len())?;
            let mut all_values = Vec::with_capacity(values.len());
            for (value, target) in values {
                meter.tree::<(u32, Vec<u128>)>(by_target.len())?;
                let accepted = by_target.entry(*target).or_default();
                meter.grow::<u128>(accepted.len())?;
                accepted.push(*value);
                all_values.push(*value);
            }
            meter.sort(all_values.len(), all_values.len())?;
            all_values.sort_unstable();
            all_values.dedup();
            meter.rows::<(u32, Option<ReferenceGuardAtomV1>)>(by_target.len() + 1)?;
            let mut edges = Vec::with_capacity(by_target.len() + 1);
            for (target, mut accepted) in by_target {
                meter.sort(accepted.len(), accepted.len())?;
                meter.clone_expression(&expression)?;
                accepted.sort_unstable();
                accepted.dedup();
                edges.push((
                    target,
                    Some(ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: expression.clone(),
                        values: accepted.into_boxed_slice(),
                        inside_set: true,
                    }),
                ));
            }
            edges.push((
                *otherwise,
                Some(ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant: expression,
                    values: all_values.into_boxed_slice(),
                    inside_set: false,
                }),
            ));
            Ok(edges)
        }
    }
}

pub fn reference_predicate_and_atom_v1(
    meter: &impl ReferenceWorkV1,
    predicate: &ReferencePathPredicateV1,
    atom: ReferenceGuardAtomV1,
) -> Result<ReferencePathPredicateV1, ReferenceBindingErrorV1> {
    meter.rows::<ReferenceGuardClauseV1>(predicate.clauses.len())?;
    let atom_units = meter.atom(&atom)?;
    let mut clauses = Vec::with_capacity(predicate.clauses.len());
    for clause in &predicate.clauses {
        let units = work::add(meter.clauses(std::slice::from_ref(clause))?, atom_units)?;
        meter.charge(units)?;
        meter.grow::<ReferenceGuardAtomV1>(clause.atoms.len())?;
        meter.sort(clause.atoms.len() + 1, units)?;
        let mut atoms = clause.atoms.to_vec();
        atoms.push(atom.clone());
        atoms.sort();
        atoms.dedup();
        clauses.push(ReferenceGuardClauseV1 {
            atoms: atoms.into_boxed_slice(),
        });
    }
    reference_normalize_predicate_v1(meter, clauses)
}

pub(super) fn reference_predicate_or_assign_v1(
    meter: &impl ReferenceWorkV1,
    target: &mut ReferencePathPredicateV1,
    source: ReferencePathPredicateV1,
) -> Result<(), ReferenceBindingErrorV1> {
    meter.clone_predicate(target)?;
    let count = work::add(target.clauses.len(), source.clauses.len())?;
    meter.rows::<ReferenceGuardClauseV1>(count)?;
    let mut clauses = target.clauses.to_vec();
    clauses.extend(source.clauses);
    *target = reference_normalize_predicate_v1(meter, clauses)?;
    Ok(())
}

pub(super) fn reference_normalize_predicate_v1(
    meter: &impl ReferenceWorkV1,
    mut clauses: Vec<ReferenceGuardClauseV1>,
) -> Result<ReferencePathPredicateV1, ReferenceBindingErrorV1> {
    let units = meter.clauses(&clauses)?;
    meter.sort(clauses.len(), units)?;
    clauses.sort();
    clauses.dedup();
    if clauses.len() > MAX_REFERENCE_GUARD_CLAUSES_V1 {
        return Err(ReferenceBindingErrorV1::new(format!(
            "reference path predicate has {} clauses; maximum is {MAX_REFERENCE_GUARD_CLAUSES_V1}",
            clauses.len(),
        )));
    }
    meter.charge(clauses.len())?;
    let atoms = clauses.iter().try_fold(0_usize, |total, clause| {
        total.checked_add(clause.atoms.len())
    });
    if atoms.is_none_or(|atoms| atoms > MAX_REFERENCE_GUARD_ATOMS_V1) {
        return Err(ReferenceBindingErrorV1::new(format!(
            "reference path predicate exceeds {MAX_REFERENCE_GUARD_ATOMS_V1} total atoms",
        )));
    }
    Ok(ReferencePathPredicateV1 {
        clauses: clauses.into_boxed_slice(),
    })
}
