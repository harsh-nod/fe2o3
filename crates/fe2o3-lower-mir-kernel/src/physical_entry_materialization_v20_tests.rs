//! Inert component tests; this fixture is not authenticated Rust source.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV20};
#[path = "physical_entry_materialization_fixture_v20_tests.rs"]
mod fixture;

fn events(module: &fe2o3_kernel_ir::Module) -> (Origin, Site, Vec<Event>) {
    let body = module.functions[0].body.as_ref().unwrap();
    let OperationKind::Gfx942PhysicalEntryDeclaration(declaration) =
        body.blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    let mut result = Vec::new();
    for (block, contract) in body.blocks.iter().zip(declaration.blocks) {
        result.push(Event {
            site: contract.label_site,
            kind: EventKind::Label(contract.label),
        });
        for operation in &block.operations {
            if let OperationKind::Gfx942PhysicalEntryStep(step) = operation.kind {
                result.push(Event {
                    site: step.site,
                    kind: EventKind::Step(step.instruction),
                });
            }
        }
        let (opcode, immediate) = match contract.encoding {
            Encoding::Scc1 => (Opcode::BranchScc1, u32::from(declaration.blocks[2].label)),
            Encoding::Jump => (Opcode::Branch, u32::from(declaration.blocks[3].label)),
            Encoding::Fallthrough => (Opcode::Fallthrough, u32::from(declaration.blocks[3].label)),
            Encoding::Endpgm0 => (Opcode::Endpgm0, 0),
            Encoding::Inactive => panic!("inactive block"),
        };
        result.push(Event {
            site: contract.terminator_site,
            kind: EventKind::Step(Instruction {
                opcode,
                destination: 0,
                source0: 0,
                source1: 0,
                immediate,
            }),
        });
    }
    (declaration.origin, declaration.begin_site, result)
}
fn input<'a>(origin: Origin, begin: Site, events: &'a [Event]) -> Input<'a> {
    Input {
        origin,
        begin,
        events,
        export_name: "physical_entry",
    }
}
fn build(input: &Input<'_>) -> Pending {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(79).unwrap();
    let result = materialize(input, &mut budget).unwrap();
    assert_eq!(budget.storage(), 79);
    assert_eq!(budget.work(), WORK);
    result
}
fn failure(input: &Input<'_>) -> Error {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(79).unwrap();
    let result = materialize(input, &mut budget).err().unwrap();
    assert_eq!(budget.storage(), 79);
    result
}
fn canonical(module: &fe2o3_kernel_ir::Module) -> bool {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
        module,
        &mut budget,
    )
    .is_ok()
}
fn replace(module: &mut fe2o3_kernel_ir::Module, pending: Pending) {
    let capabilities = module.functions[0].required_capabilities.clone();
    module.functions[0] = pending.into_function();
    module.functions[0].required_capabilities = capabilities;
}
#[test]
fn one_and_diamond_construct_exact_existing_physical_ssa_and_cfg() {
    for select in [false, true] {
        let mut expected = fixture::module(select);
        let (origin, begin, events) = events(&expected);
        let symbol = expected.functions[0].id.as_str();
        let input = Input {
            export_name: symbol,
            ..input(origin, begin, &events)
        };
        let pending = build(&input);
        let mut expected_function = expected.functions[0].clone();
        expected_function.required_capabilities.clear();
        assert_eq!(pending.function(), &expected_function);
        replace(&mut expected, pending);
        assert!(canonical(&expected));
    }
}
#[test]
fn diamond_merges_actual_different_physical_versions_in_register_order() {
    let module = fixture::module(true);
    let (origin, begin, events) = events(&module);
    let pending = build(&input(origin, begin, &events));
    let blocks = &pending.function().body.as_ref().unwrap().blocks;
    assert_eq!(blocks[3].parameters.len(), 1);
    let phi = blocks[3].parameters[0].id;
    let stored = blocks[3].operations.iter().find_map(|op| match op.kind {
        OperationKind::Gfx942PhysicalEntryStep(step)
            if step.instruction.opcode == Opcode::GlobalStoreDword =>
        {
            step.operands[2]
        }
        _ => None,
    });
    assert_eq!(stored, Some(phi));
    let edges = [1usize, 2].map(|i| match &blocks[i].terminator {
        Some(Terminator::Branch {
            target: BlockId(3),
            arguments,
        }) => arguments.clone(),
        _ => panic!("edge"),
    });
    assert_eq!(edges[0].len(), 1);
    assert_eq!(edges[1].len(), 1);
    assert_ne!(edges[0], edges[1]);
}
#[test]
fn edited_output_register_is_actual_ssa_not_a_role_plan() {
    let mut module = fixture::module(true);
    let (origin, begin, mut events) = events(&module);
    for event in &mut events {
        if let EventKind::Step(instruction) = &mut event.kind {
            if instruction.opcode == Opcode::VectorMove32 && instruction.destination == 8 {
                instruction.destination = 22;
            }
            if instruction.opcode == Opcode::GlobalStoreDword {
                instruction.source1 = 22;
            }
        }
    }
    let name = module.functions[0].id.as_str();
    let pending = build(&Input {
        export_name: name,
        ..input(origin, begin, &events)
    });
    replace(&mut module, pending);
    assert!(canonical(&module));
}
#[test]
fn undefined_arm_register_cannot_be_read_after_merge() {
    let module = fixture::module(true);
    let (origin, begin, mut events) = events(&module);
    let event = events
        .iter_mut()
        .find(|e| {
            matches!(
                e.kind,
                EventKind::Step(Instruction {
                    opcode: Opcode::VectorMove32,
                    destination: 8,
                    source0: 13,
                    ..
                })
            )
        })
        .unwrap();
    if let EventKind::Step(i) = &mut event.kind {
        i.destination = 22;
    }
    assert_eq!(
        failure(&input(origin, begin, &events)),
        Error::UndefinedRegister
    );
}
#[test]
fn repeated_or_swapped_occurrence_is_not_silently_reordered() {
    let module = fixture::module(false);
    let (origin, begin, mut events) = events(&module);
    events[2].site.raw_block = events[1].site.raw_block;
    assert_eq!(failure(&input(origin, begin, &events)), Error::Occurrence);
    events[2].site.raw_block = 999;
    events.swap(1, 2);
    assert_eq!(failure(&input(origin, begin, &events)), Error::Occurrence);
}
#[test]
fn branch_target_and_physical_fallthrough_must_match_exact_authored_cfg() {
    let module = fixture::module(true);
    let (origin, begin, mut events) = events(&module);
    let branch = events
        .iter()
        .position(|e| {
            matches!(
                e.kind,
                EventKind::Step(Instruction {
                    opcode: Opcode::BranchScc1,
                    ..
                })
            )
        })
        .unwrap();
    if let EventKind::Step(i) = &mut events[branch].kind {
        i.immediate = 4;
    }
    assert_eq!(failure(&input(origin, begin, &events)), Error::Shape);
    if let EventKind::Step(i) = &mut events[branch].kind {
        i.immediate = 0;
    }
    let fall = events
        .iter()
        .position(|e| {
            matches!(
                e.kind,
                EventKind::Step(Instruction {
                    opcode: Opcode::Fallthrough,
                    ..
                })
            )
        })
        .unwrap();
    if let EventKind::Step(i) = &mut events[fall].kind {
        i.opcode = Opcode::Branch;
    }
    assert_eq!(failure(&input(origin, begin, &events)), Error::Shape);
}
#[test]
fn reserved_livein_register_destination_refuses_before_graph_admission() {
    let module = fixture::module(false);
    let (origin, begin, mut events) = events(&module);
    if let EventKind::Step(i) = &mut events[1].kind {
        i.destination = 0;
    }
    assert_eq!(failure(&input(origin, begin, &events)), Error::Instruction);
}
#[test]
fn shape_construction_does_not_replace_canonical_readiness_proof() {
    let mut module = fixture::module(false);
    let (origin, begin, mut events) = events(&module);
    let wait = events
        .iter()
        .position(|e| {
            matches!(
                e.kind,
                EventKind::Step(Instruction {
                    opcode: Opcode::WaitLgkm0,
                    ..
                })
            )
        })
        .unwrap();
    if let EventKind::Step(i) = &mut events[wait].kind {
        *i = Instruction {
            opcode: Opcode::VectorMove32,
            destination: 22,
            source0: 255,
            source1: 0,
            immediate: 0,
        };
    }
    let name = module.functions[0].id.as_str();
    let pending = build(&Input {
        export_name: name,
        ..input(origin, begin, &events)
    });
    replace(&mut module, pending);
    assert!(!canonical(&module));
}
#[test]
fn work_and_storage_failures_restore_the_incoming_floor() {
    let module = fixture::module(false);
    let (origin, begin, events) = events(&module);
    let input = input(origin, begin, &events);
    let mut work = Work::new(WORK - 1);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(79).unwrap();
    assert!(matches!(
        materialize(&input, &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(budget.storage(), 79);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 80);
    budget.reserve_storage(79).unwrap();
    assert!(matches!(
        materialize(&input, &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(budget.storage(), 79);
}
#[test]
fn empty_or_unclosed_program_and_incomplete_origin_refuse() {
    let module = fixture::module(false);
    let (origin, begin, events) = events(&module);
    assert_eq!(failure(&input(origin, begin, &[])), Error::Shape);
    assert_eq!(
        failure(&input(origin, begin, &events[..events.len() - 1])),
        Error::Shape
    );
    let mut missing = origin;
    missing.mir_body = [0; 32];
    assert_eq!(failure(&input(missing, begin, &events)), Error::Origin);
}
