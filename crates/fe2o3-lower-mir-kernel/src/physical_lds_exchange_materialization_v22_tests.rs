//! Inert structural controls, never evidence of live Rust/source custody.
// Parent integration supplies the single shared KIR22 graph; no private copy.
use super::*;
use fe2o3_kernel_ir as physical_lds_exchange_fixture_ir;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV22};
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_lds_exchange_v22.rs"]
pub(crate) mod fixture;

fn events(module: &fe2o3_kernel_ir::Module) -> (Origin, Site, Vec<Event>) {
    let block = &module.functions[0].body.as_ref().unwrap().blocks[0];
    let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(decl) = block.operations[0].kind else {
        panic!("declaration")
    };
    let mut events = vec![Event {
        site: decl.block.label_site,
        kind: EventKind::Label(0),
    }];
    for op in &block.operations {
        if let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = op.kind {
            events.push(Event {
                site: step.site,
                kind: EventKind::Step(step.instruction),
            });
        }
    }
    events.push(Event {
        site: decl.block.terminator_site,
        kind: EventKind::Step(Instruction {
            opcode: Opcode::Endpgm0,
            destination: 0,
            source0: 0,
            source1: 0,
            immediate: 0,
        }),
    });
    (decl.origin, decl.begin_site, events)
}
fn input<'a>(origin: Origin, begin: Site, events: &'a [Event]) -> Input<'a> {
    Input {
        origin,
        begin,
        lds_frame: fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1,
        },
        events,
        export_name: "physical_lds_exchange_fixture",
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
    VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
        module,
        &mut budget,
    )
    .is_ok()
}
fn replace(module: &mut fe2o3_kernel_ir::Module, pending: Pending) {
    let caps = module.functions[0].required_capabilities.clone();
    module.functions[0] = pending.into_function();
    module.functions[0].required_capabilities = caps;
}
fn find(events: &mut [Event], opcode: Opcode) -> &mut Instruction {
    events
        .iter_mut()
        .find_map(|event| match &mut event.kind {
            EventKind::Step(instruction) if instruction.opcode == opcode => Some(instruction),
            _ => None,
        })
        .unwrap()
}
#[test]
fn physical_lds_exchange_constructs_exact_one_block_graph_and_two_logical_parameters() {
    for extra in [false, true] {
        let mut module = if extra {
            fixture::module_with_registers(true)
        } else {
            fixture::module()
        };
        let (origin, begin, events) = events(&module);
        let pending = build(&input(origin, begin, &events));
        let mut expected = module.functions[0].clone();
        expected.required_capabilities.clear();
        assert_eq!(pending.function(), &expected);
        assert_eq!(pending.function().signature.parameters.len(), 2);
        replace(&mut module, pending);
        assert!(canonical(&module));
    }
}
#[test]
fn physical_lds_exchange_edited_load_destination_remains_actual_lds_write_ssa() {
    let mut module = fixture::module();
    let (origin, begin, mut events) = events(&module);
    find(&mut events, Opcode::GlobalLoadDword).destination = 22;
    find(&mut events, Opcode::LdsWriteB32).source1 = 22;
    let pending = build(&input(origin, begin, &events));
    replace(&mut module, pending);
    assert!(canonical(&module));
}
#[test]
fn physical_lds_exchange_undefined_value_and_reserved_livein_refuse_construction() {
    let module = fixture::module();
    let (origin, begin, events) = events(&module);
    let mut undefined = events.clone();
    find(&mut undefined, Opcode::GlobalStoreDword).source1 = 22;
    assert_eq!(
        failure(&input(origin, begin, &undefined)),
        Error::UndefinedRegister
    );
    let mut reserved = events;
    find(&mut reserved, Opcode::LoadKernargPair).destination = 0;
    assert_eq!(
        failure(&input(origin, begin, &reserved)),
        Error::Instruction
    );
}
#[test]
fn physical_lds_exchange_source_order_identity_and_single_label_are_not_repaired() {
    let module = fixture::module();
    let (origin, begin, events) = events(&module);
    let mut duplicate = events.clone();
    duplicate[2].site.raw_block = duplicate[1].site.raw_block;
    assert_eq!(
        failure(&input(origin, begin, &duplicate)),
        Error::Occurrence
    );
    let mut swapped = events.clone();
    swapped.swap(1, 2);
    assert_eq!(failure(&input(origin, begin, &swapped)), Error::Occurrence);
    let mut label = events;
    label[0].kind = EventKind::Label(1);
    assert_eq!(failure(&input(origin, begin, &label)), Error::Shape);
}
#[test]
fn physical_lds_exchange_structural_construction_never_substitutes_readiness_or_provenance_proof() {
    for which in 0..5 {
        let mut module = fixture::module();
        let (origin, begin, mut events) = events(&module);
        match which {
            0 => {
                *find(&mut events, Opcode::WaitLgkm0) = Instruction {
                    opcode: Opcode::VectorMove32,
                    destination: 30,
                    source0: 255,
                    source1: 0,
                    immediate: 0,
                }
            }
            1 => {
                *find(&mut events, Opcode::WaitVm0) = Instruction {
                    opcode: Opcode::VectorMove32,
                    destination: 30,
                    source0: 255,
                    source1: 0,
                    immediate: 0,
                }
            }
            2 => find(&mut events, Opcode::VectorAddCarryIn).source0 = 3,
            3 => find(&mut events, Opcode::GlobalStoreDword).source1 = 3,
            4 => find(&mut events, Opcode::GlobalStoreDword).source0 = 6,
            _ => unreachable!(),
        }
        let pending = build(&input(origin, begin, &events));
        replace(&mut module, pending);
        assert!(!canonical(&module), "canonical safety mutant {which}");
    }
}
#[test]
fn physical_lds_exchange_unclosed_program_incomplete_origin_and_bad_symbol_refuse() {
    let module = fixture::module();
    let (origin, begin, events) = events(&module);
    assert_eq!(failure(&input(origin, begin, &[])), Error::Shape);
    assert_eq!(
        failure(&input(origin, begin, &events[..events.len() - 1])),
        Error::Shape
    );
    let mut missing = origin;
    missing.mir_body = [0; 32];
    assert_eq!(failure(&input(missing, begin, &events)), Error::Origin);
    assert_eq!(
        failure(&Input {
            export_name: "a-b",
            ..input(origin, begin, &events)
        }),
        Error::Symbol
    );
}
#[test]
fn physical_lds_exchange_cumulative_work_and_one_short_storage_restore_floor() {
    let module = fixture::module();
    let (origin, begin, events) = events(&module);
    let input = input(origin, begin, &events);
    let mut work = Work::new(1_000_000);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(79).unwrap();
    let pending = materialize(&input, &mut budget).unwrap();
    assert_eq!(budget.work(), 11 + WORK);
    assert_eq!(budget.storage(), 79);
    let peak = budget.peak_storage();
    assert!(pending.retained_storage() > 0);
    for case in 0..3 {
        let mut work = Work::new(11 + WORK - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, peak - usize::from(case == 2));
        budget.reserve_storage(79).unwrap();
        assert_eq!(materialize(&input, &mut budget).is_ok(), case == 0);
        assert_eq!(budget.storage(), 79);
    }
}

#[test]
fn physical_lds_exchange_source_frame_is_not_repaired_or_inferred() {
    let module = fixture::module();
    let (origin, begin, events) = events(&module);
    for frame in [
        fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 4,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1,
        },
        fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 508,
            alignment: 4,
            publication_epoch: 1,
        },
        fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 8,
            publication_epoch: 1,
        },
        fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 2,
        },
    ] {
        let mut input = input(origin, begin, &events);
        input.lds_frame = frame;
        assert_eq!(failure(&input), Error::Shape);
    }
}
#[test]
fn physical_lds_exchange_barrier_readiness_peer_and_memory_source_are_not_repaired() {
    for which in 0..5 {
        let mut module = fixture::module();
        let (origin, begin, mut events) = events(&module);
        match which {
            0 => find(&mut events, Opcode::WorkgroupPublishBarrier).opcode = Opcode::WaitLgkm0,
            1 => find(&mut events, Opcode::LdsWriteB32).source0 = 2,
            2 => find(&mut events, Opcode::LdsReadB32).source0 = 16,
            3 => find(&mut events, Opcode::GlobalStoreDword).source1 = 8,
            4 => {
                let index = events
                    .iter()
                    .position(
                        |e| matches!(e.kind,EventKind::Step(i) if i.opcode==Opcode::LdsReadB32),
                    )
                    .unwrap();
                let EventKind::Step(ref mut i) = events[index + 1].kind else {
                    panic!("wait")
                };
                i.opcode = Opcode::WorkgroupPublishBarrier;
            }
            _ => unreachable!(),
        }
        let pending = build(&input(origin, begin, &events));
        replace(&mut module, pending);
        assert!(!canonical(&module), "LDS mutant {which}");
    }
}
