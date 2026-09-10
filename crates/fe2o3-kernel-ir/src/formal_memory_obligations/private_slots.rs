use super::*;

pub(super) fn classify_eligible_private_slots(
    function: &Function,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> BTreeSet<ValueId> {
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry is defined");
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let slots = definitions
        .operations
        .iter()
        .filter_map(|(value, (operation, _))| {
            matches!(
                operation.kind,
                OperationKind::Alloca {
                    count: None,
                    address_space: AddressSpace::Private,
                    ..
                }
            )
            .then_some(*value)
        })
        .collect::<BTreeSet<_>>();
    let exact_slot = |value| {
        definitions
            .exact_ssa_origin(value, value_types)
            .filter(|origin| slots.contains(origin))
    };
    let mut escapes = BTreeMap::<ValueId, (FunctionOperationLocation, ValueId)>::new();
    let record_terminator_escape = |value: ValueId, escapes: &mut BTreeMap<_, _>| {
        if let Some(slot) = exact_slot(value)
            && let Some((_, location)) = definitions.operations.get(&slot)
        {
            escapes.entry(slot).or_insert((*location, value));
        }
    };
    let record_edge_escapes =
        |target: BlockId, arguments: &[ValueId], escapes: &mut BTreeMap<_, _>| {
            let parameters = blocks.get(&target).map(|block| block.parameters.as_slice());
            let exact_edge = parameters.is_some_and(|parameters| {
                arguments.len() == parameters.len()
                    && arguments
                        .iter()
                        .zip(parameters)
                        .all(|(argument, parameter)| {
                            value_types
                                .get(argument)
                                .is_some_and(|ty| ty == &parameter.ty)
                        })
            });
            for (index, argument) in arguments.iter().copied().enumerate() {
                let Some(slot) = exact_slot(argument) else {
                    continue;
                };
                let exact_transport = exact_edge
                    && parameters
                        .and_then(|parameters| parameters.get(index))
                        .is_some_and(|parameter| exact_slot(parameter.id) == Some(slot));
                if !exact_transport && let Some((_, location)) = definitions.operations.get(&slot) {
                    escapes.entry(slot).or_insert((*location, argument));
                }
            }
        };

    for block in body
        .blocks
        .iter()
        .filter(|block| definitions.is_reachable(block.id))
    {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            for operand in operation.kind.operands() {
                let Some(slot) = exact_slot(operand) else {
                    continue;
                };
                let exact_access = match &operation.kind {
                    OperationKind::Load { pointer, access } => {
                        *pointer == operand
                            && access.address_space == AddressSpace::Private
                            && exact_slot(*pointer) == Some(slot)
                    }
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } => {
                        *pointer == operand
                            && access.address_space == AddressSpace::Private
                            && exact_slot(*pointer) == Some(slot)
                            && exact_slot(*value) != Some(slot)
                    }
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value,
                        ..
                    } => {
                        *value == operand
                            && matches!(
                                operation.results.as_slice(),
                                [result] if exact_slot(result.id) == Some(slot)
                            )
                    }
                    _ => false,
                };
                if !exact_access {
                    escapes.entry(slot).or_insert((location, operand));
                }
            }
        }

        let Some(terminator) = &block.terminator else {
            continue;
        };
        match terminator {
            crate::Terminator::Branch { target, arguments } => {
                record_edge_escapes(*target, arguments, &mut escapes);
            }
            crate::Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                record_terminator_escape(*condition, &mut escapes);
                record_edge_escapes(*then_target, then_arguments, &mut escapes);
                record_edge_escapes(*else_target, else_arguments, &mut escapes);
            }
            crate::Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                record_terminator_escape(*selector, &mut escapes);
                for case in cases {
                    record_edge_escapes(case.target, &case.arguments, &mut escapes);
                }
                record_edge_escapes(*default_target, default_arguments, &mut escapes);
            }
            crate::Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                record_terminator_escape(*selector, &mut escapes);
                for case in cases {
                    record_edge_escapes(case.target, &case.arguments, &mut escapes);
                }
                record_edge_escapes(*default_target, default_arguments, &mut escapes);
            }
            crate::Terminator::Return { values } => {
                for value in values {
                    record_terminator_escape(*value, &mut escapes);
                }
            }
            crate::Terminator::Unreachable => {}
        }
    }

    for (location, pointer) in escapes.values() {
        reasons.insert(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: *location,
            pointer: *pointer,
        });
    }
    let escaped_slots = escapes.keys().copied().collect::<BTreeSet<_>>();
    slots.difference(&escaped_slots).copied().collect()
}

fn exact_private_alloca_access_origin(
    pointer: ValueId,
    access: MemoryAccess,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
) -> Option<ValueId> {
    if access.address_space != AddressSpace::Private {
        return None;
    }
    definitions
        .exact_ssa_origin(pointer, value_types)
        .filter(|origin| eligible_private_slots.contains(origin))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateSlotState {
    Uninitialized,
    Exact(ValueId),
    Unknown,
}

impl PrivateSlotState {
    fn join(self, other: Self) -> Self {
        if self == other { self } else { Self::Unknown }
    }
}

pub(super) fn collect_private_load_sources(
    function: &Function,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
) -> BTreeMap<ValueId, ValueId> {
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    let slots = eligible_private_slots;
    if slots.is_empty() || body.blocks.is_empty() {
        return BTreeMap::new();
    }

    let block_ids = body
        .blocks
        .iter()
        .filter(|block| definitions.is_reachable(block.id))
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut predecessors = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
        if let Some(terminator) = &block.terminator {
            for successor in terminator.successors() {
                if let Some(incoming) = predecessors.get_mut(&successor) {
                    incoming.insert(block.id);
                }
            }
        }
    }

    let entry = body.blocks[0].id;
    let initial = slots
        .iter()
        .copied()
        .map(|slot| (slot, PrivateSlotState::Uninitialized))
        .collect::<BTreeMap<_, _>>();
    let mut incoming = BTreeMap::<BlockId, BTreeMap<ValueId, PrivateSlotState>>::new();
    let mut outgoing = BTreeMap::<BlockId, BTreeMap<ValueId, PrivateSlotState>>::new();
    incoming.insert(entry, initial.clone());

    loop {
        let mut changed = false;
        for block in &body.blocks {
            if !definitions.is_reachable(block.id) {
                continue;
            }
            let next_incoming = if block.id == entry {
                Some(initial.clone())
            } else {
                predecessors.get(&block.id).and_then(|blocks| {
                    let mut states = blocks.iter().filter_map(|block| outgoing.get(block));
                    let first = states.next()?.clone();
                    Some(states.fold(first, |mut joined, state| {
                        for slot in slots {
                            let left = joined
                                .get(slot)
                                .copied()
                                .unwrap_or(PrivateSlotState::Unknown);
                            let right = state
                                .get(slot)
                                .copied()
                                .unwrap_or(PrivateSlotState::Unknown);
                            joined.insert(*slot, left.join(right));
                        }
                        joined
                    }))
                })
            };
            let Some(next_incoming) = next_incoming else {
                continue;
            };
            changed |= incoming.get(&block.id) != Some(&next_incoming);
            incoming.insert(block.id, next_incoming.clone());
            let mut next_outgoing = next_incoming;
            transfer_private_slot_stores(
                block,
                definitions,
                value_types,
                eligible_private_slots,
                &mut next_outgoing,
            );
            changed |= outgoing.get(&block.id) != Some(&next_outgoing);
            outgoing.insert(block.id, next_outgoing);
        }
        if !changed {
            break;
        }
    }

    let mut sources = BTreeMap::new();
    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
        let Some(mut state) = incoming.get(&block.id).cloned() else {
            continue;
        };
        for operation in &block.operations {
            if let OperationKind::Load { pointer, access } = operation.kind
                && let Some(slot) = exact_private_alloca_access_origin(
                    pointer,
                    access,
                    definitions,
                    value_types,
                    eligible_private_slots,
                )
                && let Some(PrivateSlotState::Exact(source)) = state.get(&slot).copied()
            {
                for result in &operation.results {
                    sources.insert(result.id, source);
                }
            }
            transfer_private_slot_store(
                operation,
                definitions,
                value_types,
                eligible_private_slots,
                &mut state,
            );
        }
    }
    sources
}

fn transfer_private_slot_stores(
    block: &crate::BasicBlock,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
    state: &mut BTreeMap<ValueId, PrivateSlotState>,
) {
    for operation in &block.operations {
        transfer_private_slot_store(
            operation,
            definitions,
            value_types,
            eligible_private_slots,
            state,
        );
    }
}

fn transfer_private_slot_store(
    operation: &Operation,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
    state: &mut BTreeMap<ValueId, PrivateSlotState>,
) {
    let OperationKind::Store {
        pointer,
        value,
        access,
    } = operation.kind
    else {
        return;
    };
    if let Some(slot) = exact_private_alloca_access_origin(
        pointer,
        access,
        definitions,
        value_types,
        eligible_private_slots,
    ) {
        state.insert(
            slot,
            definitions
                .exact_ssa_origin(value, value_types)
                .map_or(PrivateSlotState::Unknown, PrivateSlotState::Exact),
        );
    }
}
