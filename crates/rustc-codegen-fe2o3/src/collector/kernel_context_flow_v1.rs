//! Validate only the generated entry protocol, never recognize user kernel bodies.

use super::*;
use rustc_middle::mir::{
    BasicBlock, Const, ConstValue, Local, Location, START_BLOCK, StatementKind,
};

#[derive(Debug, Eq, PartialEq)]
struct CallOccurrenceV1 {
    location: Location,
    destination: Local,
    target: BasicBlock,
    unwind: UnwindAction,
}

/// Occurrences are qualified by the exact root instance in the retained source flow.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct AuthenticatedFlowV1<'tcx> {
    issuer: Instance<'tcx>,
    issuance: CallOccurrenceV1,
    helper_call: CallOccurrenceV1,
}

impl<'tcx> AuthenticatedFlowV1<'tcx> {
    pub(super) fn issuer(&self) -> Instance<'tcx> {
        self.issuer
    }
}

/// Session-owned proof of the unoptimized wrapper's actual value flow.
pub(super) struct SourceFlowV1<'tcx> {
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    original: AuthenticatedFlowV1<'tcx>,
    context: Ty<'tcx>,
    physical_types: Vec<Ty<'tcx>>,
}

pub(super) fn authenticate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    context: Ty<'tcx>,
    body: &Body<'tcx>,
) -> Result<SourceFlowV1<'tcx>, CollectError> {
    let signature = source_signature_v1(tcx, root).map_err(error)?;
    check_parameter_count(signature.inputs().len(), body.arg_count)?;
    let physical_types = signature.inputs().to_vec();
    let original = authenticate(tcx, root, helper, context, body, None)?;
    Ok(SourceFlowV1 {
        root,
        helper,
        original,
        context,
        physical_types,
    })
}

pub(super) fn authenticate_optimized<'tcx>(
    tcx: TyCtxt<'tcx>,
    source: &SourceFlowV1<'tcx>,
) -> Result<AuthenticatedFlowV1<'tcx>, CollectError> {
    let body = tcx.instance_mir(source.root.def);
    check_parameter_count(source.physical_types.len(), body.arg_count)?;
    let optimized = authenticate(
        tcx,
        source.root,
        source.helper,
        source.context,
        body,
        Some(source),
    )?;
    if optimized.issuer != source.original.issuer {
        return Err(error(
            "optimized entry substituted the authenticated issuer",
        ));
    }
    // Both closed protocols establish the unique issuer-to-helper correspondence.
    // MIR optimization may renumber blocks/locals and erase ZST transport.
    Ok(optimized)
}

fn check_parameter_count(
    signature_inputs: usize,
    body_arguments: usize,
) -> Result<(), CollectError> {
    if signature_inputs > fe2o3_rustc_front::MAX_PARAMETERS_PER_FUNCTION_V1 {
        return Err(error("entry protocol exceeds its physical-parameter bound"));
    }
    if signature_inputs != body_arguments {
        return Err(error(
            "entry protocol argument count disagrees with its source signature",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Origin {
    Argument(usize),
    Context,
    Result,
    Unit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    BeforeIssue,
    Issued,
    Called,
}

struct Values {
    origins: Vec<Option<Origin>>,
    arguments: usize,
    phase: Phase,
}

impl Values {
    fn new(locals: usize, arguments: usize) -> Result<Self, CollectError> {
        // Generated wrappers need only argument forwarding and two call results.
        let limit = fe2o3_rustc_front::MAX_PARAMETERS_PER_FUNCTION_V1.saturating_mul(4) + 32;
        if locals > limit || locals <= arguments {
            return Err(error("entry protocol exceeds its local-value bound"));
        }
        let mut origins = vec![None; locals];
        for (index, value) in origins.iter_mut().enumerate().take(arguments + 1).skip(1) {
            *value = Some(Origin::Argument(index - 1));
        }
        Ok(Self {
            origins,
            arguments,
            phase: Phase::BeforeIssue,
        })
    }

    fn read(&mut self, local: usize, moved: bool) -> Result<Origin, CollectError> {
        let value = self
            .origins
            .get_mut(local)
            .ok_or_else(|| error("invalid entry local"))?;
        let origin = value.ok_or_else(|| error("uninitialized entry operand"))?;
        if origin == Origin::Context && !moved {
            return Err(error("issued context must be moved, not copied"));
        }
        if moved {
            *value = None;
        }
        Ok(origin)
    }

    fn assign(&mut self, local: usize, origin: Origin) -> Result<(), CollectError> {
        if local > 0 && local <= self.arguments {
            return Err(error("entry protocol overwrites a physical argument"));
        }
        let slot = self
            .origins
            .get_mut(local)
            .ok_or_else(|| error("invalid entry destination"))?;
        if *slot == Some(Origin::Context) {
            return Err(error("entry protocol overwrites an unconsumed context"));
        }
        *slot = Some(origin);
        Ok(())
    }

    fn issue(&mut self, local: usize) -> Result<(), CollectError> {
        if self.phase != Phase::BeforeIssue || local == 0 {
            return Err(error(
                "entry protocol issues more than once or into its return place",
            ));
        }
        self.assign(local, Origin::Context)?;
        self.phase = Phase::Issued;
        Ok(())
    }

    fn call(&mut self, operands: &[Origin], destination: usize) -> Result<(), CollectError> {
        if self.phase != Phase::Issued
            || operands.len() != self.arguments + 1
            || operands[0] != Origin::Context
            || !operands[1..]
                .iter()
                .enumerate()
                .all(|(i, origin)| *origin == Origin::Argument(i))
        {
            return Err(error(
                "helper must consume the issued context and identity-forward every physical argument",
            ));
        }
        self.assign(destination, Origin::Result)?;
        self.phase = Phase::Called;
        Ok(())
    }
}

fn local(place: rustc_middle::mir::Place<'_>) -> Result<usize, CollectError> {
    place
        .as_local()
        .map(|local| local.index())
        .ok_or_else(|| error("entry protocol requires whole-local value transport"))
}

fn operand(values: &mut Values, operand: &Operand<'_>) -> Result<Origin, CollectError> {
    match operand {
        Operand::Move(place) => values.read(local(*place)?, true),
        Operand::Copy(place) => values.read(local(*place)?, false),
        Operand::Constant(constant) if constant.const_.ty().is_unit() => Ok(Origin::Unit),
        _ => Err(error("entry protocol contains a substituted operand")),
    }
}

fn authenticate<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    context: Ty<'tcx>,
    body: &Body<'tcx>,
    source: Option<&SourceFlowV1<'tcx>>,
) -> Result<AuthenticatedFlowV1<'tcx>, CollectError> {
    let step_limit = fe2o3_rustc_front::MAX_PARAMETERS_PER_FUNCTION_V1.saturating_mul(16) + 64;
    let steps = body.basic_blocks.iter().try_fold(0usize, |sum, block| {
        let next = sum.checked_add(block.statements.len())?.checked_add(1)?;
        (next <= step_limit).then_some(next)
    });
    if steps.is_none() {
        return Err(error(
            "entry protocol exceeds its bounded statement/control-flow work",
        ));
    }
    let mut values = Values::new(body.local_decls.len(), body.arg_count)?;
    let mut visited = BTreeSet::new();
    let mut cursor = START_BLOCK;
    let mut issuer = None;
    let mut issuance = None;
    let mut helper_call = None;
    let mut issued_local = None;
    let mut helper_result = None;
    loop {
        if !visited.insert(cursor) {
            return Err(error("entry protocol contains a cycle"));
        }
        let block = &body.basic_blocks[cursor];
        if block.is_cleanup {
            return Err(error("entry protocol enters a cleanup block"));
        }
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Nop => {}
                StatementKind::StorageLive(local) => {
                    if values.origins[local.index()] == Some(Origin::Context) {
                        return Err(error("entry protocol restarts live context storage"));
                    }
                    values.origins[local.index()] = None;
                }
                StatementKind::StorageDead(local) => {
                    if values.origins[local.index()] == Some(Origin::Context) {
                        return Err(error("issued context is discarded before the helper"));
                    }
                    values.origins[local.index()] = None;
                }
                StatementKind::Assign(assignment) => {
                    let (place, rvalue) = &**assignment;
                    let origin = match rvalue {
                        Rvalue::Use(value) => operand(&mut values, value)?,
                        Rvalue::Aggregate(kind, fields)
                            if matches!(**kind, AggregateKind::Tuple) && fields.is_empty() =>
                        {
                            Origin::Unit
                        }
                        _ => {
                            return Err(error(
                                "entry protocol contains computation or constructed authority",
                            ));
                        }
                    };
                    if origin == Origin::Unit
                        && (local(*place)? != 0 || values.phase != Phase::Called)
                    {
                        return Err(error(
                            "unit assignment is only allowed for the final physical return",
                        ));
                    }
                    values.assign(local(*place)?, origin)?;
                }
                _ => return Err(error("entry protocol contains an unsupported statement")),
            }
        }
        let terminator = block
            .terminator
            .as_ref()
            .ok_or_else(|| error("entry block has no terminator"))?;
        match &terminator.kind {
            TerminatorKind::Goto { target } => cursor = *target,
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
                ..
            } => {
                if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
                    return Err(error("entry protocol contains an unsupported unwind edge"));
                }
                let next = target.ok_or_else(|| error("entry protocol call has no return edge"))?;
                let callee = resolve_call(tcx, root, body, func)?;
                let occurrence = CallOccurrenceV1 {
                    location: Location {
                        block: cursor,
                        statement_index: block.statements.len(),
                    },
                    destination: destination
                        .as_local()
                        .ok_or_else(|| error("entry call result requires a whole local"))?,
                    target: next,
                    unwind: *unwind,
                };
                if trusted_device_items::classify(tcx, callee.def_id())
                    == Some(TrustedDeviceItem::KernelContextIssue)
                {
                    let signature = source_signature_v1(tcx, callee).map_err(error)?;
                    if !args.is_empty()
                        || !signature.inputs().is_empty()
                        || signature.output() != context
                    {
                        return Err(error("issuer arguments or result brand differ"));
                    }
                    let TyKind::Adt(context_adt, _) = context.kind() else {
                        return Err(error("issuer result is not an authenticated context"));
                    };
                    if callee.def_id().krate != context_adt.did().krate {
                        return Err(error("issuer and context come from different providers"));
                    }
                    values.issue(local(*destination)?)?;
                    issued_local = Some(local(*destination)?);
                    issuer = Some(callee);
                    issuance = Some(occurrence);
                } else if callee == helper {
                    let operands = args
                        .iter()
                        .enumerate()
                        .map(|(ordinal, arg)| {
                            if let Some(source) = source
                                && let Operand::Constant(constant) = &arg.node
                                && let Const::Val(ConstValue::ZeroSized, ty) = constant.const_
                            {
                                let expected = if ordinal == 0 {
                                    Some(source.context)
                                } else {
                                    source.physical_types.get(ordinal - 1).copied()
                                };
                                if expected != Some(ty) {
                                    return Err(error("optimized ZST operand changed its authenticated type or ordinal"));
                                }
                                return if ordinal == 0 {
                                    let issued = issued_local.ok_or_else(|| error("erased context has no issuer occurrence"))?;
                                    values.read(issued, true)
                                } else {
                                    Ok(Origin::Argument(ordinal - 1))
                                };
                            }
                            operand(&mut values, &arg.node)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let destination = local(*destination)?;
                    values.call(&operands, destination)?;
                    helper_result = Some(destination);
                    helper_call = Some(occurrence);
                } else {
                    return Err(error(
                        "entry protocol calls a function other than its issuer or logical helper",
                    ));
                }
                cursor = next;
            }
            TerminatorKind::Drop {
                place,
                target,
                unwind,
                ..
            } => {
                if values.phase != Phase::Called
                    || Some(local(*place)?) != helper_result
                    || !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
                    || !matches!(
                        classify_rustc_drop_v1(tcx, root, body, *place),
                        Ok(ProductionRustcDropClassV1::Trivial)
                    )
                {
                    return Err(error(
                        "entry protocol contains a nontrivial or unrelated drop",
                    ));
                }
                cursor = *target;
            }
            TerminatorKind::Return => {
                if values.phase != Phase::Called || visited.len() != body.basic_blocks.len() {
                    return Err(error(
                        "entry protocol bypasses issuance/helper or has unvisited blocks",
                    ));
                }
                let output = source_signature_v1(tcx, root).map_err(error)?.output();
                let valid_return = matches!(values.origins[0], Some(Origin::Result))
                    || (output.is_unit() && matches!(values.origins[0], None | Some(Origin::Unit)));
                if !valid_return {
                    return Err(error(
                        "physical return does not originate from the logical helper",
                    ));
                }
                return Ok(AuthenticatedFlowV1 {
                    issuer: issuer.ok_or_else(|| error("entry protocol has no issuer"))?,
                    issuance: issuance
                        .ok_or_else(|| error("entry protocol has no issuance occurrence"))?,
                    helper_call: helper_call
                        .ok_or_else(|| error("entry protocol has no helper occurrence"))?,
                });
            }
            _ => {
                return Err(error(
                    "entry protocol branches, reenters, or does not return",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_call_occurrence_equality_checks_every_coordinate() {
        let occurrence = || CallOccurrenceV1 {
            location: Location {
                block: BasicBlock::from_u32(2),
                statement_index: 3,
            },
            destination: Local::from_u32(4),
            target: BasicBlock::from_u32(5),
            unwind: UnwindAction::Unreachable,
        };
        assert_eq!(occurrence(), occurrence());
        for axis in 0..5 {
            let mut changed = occurrence();
            match axis {
                0 => changed.location.block = BasicBlock::from_u32(6),
                1 => changed.location.statement_index += 1,
                2 => changed.destination = Local::from_u32(7),
                3 => changed.target = BasicBlock::from_u32(8),
                4 => changed.unwind = UnwindAction::Continue,
                _ => unreachable!(),
            }
            assert_ne!(occurrence(), changed, "stale occurrence axis {axis}");
        }
    }

    #[test]
    fn issued_value_moves_through_temporaries_and_arguments_remain_ordered() {
        let mut values = Values::new(7, 2).unwrap();
        values.issue(3).unwrap();
        let context = values.read(3, true).unwrap();
        values.assign(4, context).unwrap();
        let operands = [
            values.read(4, true).unwrap(),
            values.read(1, false).unwrap(),
            values.read(2, true).unwrap(),
        ];
        values.call(&operands, 5).unwrap();
        assert_eq!(values.phase, Phase::Called);
        assert_eq!(values.read(5, true).unwrap(), Origin::Result);
        assert!(values.issue(6).is_err());
    }

    #[test]
    fn duplicate_unused_copied_and_substituted_contexts_reject() {
        let mut values = Values::new(6, 1).unwrap();
        assert!(
            values
                .call(&[Origin::Context, Origin::Argument(0)], 4)
                .is_err()
        );
        values.issue(2).unwrap();
        assert!(values.issue(3).is_err());
        assert!(values.read(2, false).is_err());
        assert!(values.assign(2, Origin::Unit).is_err());
        assert!(
            values
                .call(&[Origin::Unit, Origin::Argument(0)], 4)
                .is_err()
        );
        assert!(
            values
                .call(&[Origin::Context, Origin::Argument(1)], 4)
                .is_err()
        );
        assert!(values.assign(1, Origin::Unit).is_err());
    }

    #[test]
    fn entry_parameter_bound_and_signature_agreement_are_exact() {
        let maximum = fe2o3_rustc_front::MAX_PARAMETERS_PER_FUNCTION_V1;
        assert!(check_parameter_count(0, 0).is_ok());
        assert!(check_parameter_count(maximum, maximum).is_ok());
        assert!(check_parameter_count(maximum + 1, maximum + 1).is_err());
        assert!(check_parameter_count(usize::MAX, usize::MAX).is_err());
        assert!(check_parameter_count(0, 1).is_err());
        assert!(check_parameter_count(1, 0).is_err());
    }

    #[test]
    fn entry_local_bound_is_exact() {
        let maximum = fe2o3_rustc_front::MAX_PARAMETERS_PER_FUNCTION_V1.saturating_mul(4) + 32;
        assert!(Values::new(maximum, 1).is_ok());
        assert!(Values::new(maximum + 1, 1).is_err());
        assert!(Values::new(1, 1).is_err());
    }
}
