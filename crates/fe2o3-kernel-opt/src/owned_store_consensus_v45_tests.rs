use super::*;

fn diamond(scalar: ScalarType) -> Module {
    let mut module = fixture(scalar);
    let store = blocks(&mut module)[0].operations.pop().unwrap();
    blocks(&mut module)[1].operations.push(store.clone());
    blocks(&mut module)[2].operations.push(store);
    module
}

#[test]
fn store_consensus_owned_forwards_all_fixed_widths_without_one_dominating_store() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        assert_count(diamond(ty), 1);
    }
    let mut cycle = diamond(ScalarType::U32);
    blocks(&mut cycle)[1].terminator = Some(branch(1, 30, 40));
    blocks(&mut cycle)[2].terminator = Some(branch(1, 20, 40));
    assert_count(cycle.clone(), 1);
    blocks(&mut cycle)[1].operations.clear();
    blocks(&mut cycle)[2].operations.clear();
    assert_count(cycle, 0);
    let mut missing = diamond(ScalarType::U32);
    blocks(&mut missing)[2].operations.clear();
    assert_count(missing, 0);
}

#[test]
fn store_consensus_owned_distinct_values_and_effect_cuts_remain_noops() {
    for fault in 0..5 {
        let mut module = diamond(ScalarType::U32);
        match fault {
            0 => {
                blocks(&mut module)[0].operations.push(value(
                    2,
                    ty(),
                    Kind::Unary {
                        op: UnaryOp::Not,
                        operand: ValueId(0),
                    },
                ));
                blocks(&mut module)[2].operations[0] =
                    store(100, 2, MemoryAccess::new(AddressSpace::Private, 4));
            }
            1 => blocks(&mut module)[2]
                .operations
                .push(allocation(200, ty())),
            2 => {
                let Kind::Store { access, .. } = &mut blocks(&mut module)[2].operations[0].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            3 => {
                let Kind::Store { access, .. } = &mut blocks(&mut module)[2].operations[0].kind
                else {
                    unreachable!()
                };
                access.alignment = 2;
            }
            4 => blocks(&mut module)[2].operations.push(value(
                2,
                ty(),
                Kind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(0),
                    rhs: ValueId(0),
                },
            )),
            _ => unreachable!(),
        }
        assert_count(module, 0);
    }
}

#[test]
fn store_consensus_owned_exact_and_one_short_work_storage_preserve_custody() {
    let (input, floor) = admit(&diamond(ScalarType::U32));
    let full = resource_run(&input, floor, WORK, STORAGE);
    let owner = full.0.unwrap();
    assert_eq!(selected(&owner), 1);
    let exact = resource_run(&input, floor, full.1, full.2);
    assert_eq!(exact.0.as_ref().unwrap().origins(), owner.origins());
    assert_eq!((exact.1, exact.2), (full.1, full.2));
    let work_short = resource_run(&input, floor, full.1 - 1, full.2);
    assert!(work_short.0.is_err());
    assert_eq!(work_short.3, Some(full.1));
    assert_eq!(work_short.4, None);
    let storage_short = resource_run(&input, floor, full.1, full.2 - 1);
    assert!(storage_short.0.is_err());
    assert_eq!(storage_short.3, None);
    assert_eq!(storage_short.4, Some(full.2));
}

#[test]
fn store_consensus_owned_v18_keeps_exact_layout_owner_and_replays_after_noop() {
    use fe2o3_kernel_ir::{
        StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
        VerifiedCanonicalKernelIrModuleV18 as Owner18,
    };
    let mut module = diamond(ScalarType::U32);
    let layouts = StorageLayoutLimitsV1 {
        rows: 1,
        edges: 0,
        containment_depth: 1,
        object_bytes: 4,
    };
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, size) =
        Owner18::from_module_ref_with_verification_budget_v18(&module, layouts, &mut budget)
            .unwrap();
    budget.reserve_storage(size.retained_storage()).unwrap();
    let floor = budget.storage();
    let tail =
        prepare_owned_cross_block_forwarding_v18(&input, Limits::default(), layouts, &mut budget)
            .unwrap();
    budget.reserve_storage(tail.retained_storage()).unwrap();
    assert_eq!(
        tail.origins()
            .iter()
            .filter(|row| row.store.is_some())
            .count(),
        1
    );
    assert_eq!(
        tail.output().module().storage_layouts,
        module.storage_layouts
    );
    let (pair, size) = tail.replay_against(&input, &mut budget).unwrap();
    budget.reserve_storage(size.retained_storage()).unwrap();
    assert!(!pair.grants_authority());
    drop(pair);
    budget.release_storage(size.retained_storage()).unwrap();
    let second = prepare_owned_cross_block_forwarding_v18(
        tail.output(),
        Limits::default(),
        layouts,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(second.retained_storage()).unwrap();
    assert!(second.origins().iter().all(|row| row.store.is_none()));
    assert_eq!(
        second.output().canonical_bytes(),
        tail.output().canonical_bytes()
    );
    assert!(matches!(
        tail.replay_against(second.output(), &mut budget),
        Err(Error::ForeignInput)
    ));
    let bytes = second.retained_storage();
    drop(second);
    budget.release_storage(bytes).unwrap();
    let bytes = tail.retained_storage();
    drop(tail);
    budget.release_storage(bytes).unwrap();
    assert_eq!(budget.storage(), floor);
}

fn dynamic_diamond(scalar: ScalarType) -> Module {
    let mut module = diamond(scalar);
    let access = MemoryAccess::new(
        AddressSpace::Global,
        u32::from(scalar.bit_width().unwrap() / 8),
    );
    blocks(&mut module)[3]
        .operations
        .push(store(2, 101, access));
    blocks(&mut module)[3].terminator = Some(Terminator::Return { values: vec![] });
    let body = module.functions[0].body.take().unwrap().blocks;
    module.functions[0] = Function::kernel_entry(
        "f",
        Signature::new(
            vec![
                Type::Scalar(scalar),
                Type::BOOL,
                Type::pointer(
                    Type::Scalar(scalar),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        body,
    );
    module.kernels.push(Kernel::new(
        "consensus",
        "f",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn store_consensus_dynamic_both_arms_preserve_values_and_every_nonread_event() {
    let target = SimulationTargetV1::amdgpu_64();
    for scalar in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        let bits = scalar.bit_width().unwrap();
        let width = usize::from(bits / 8);
        with_input(dynamic_diamond(scalar), |input, budget| {
            let owner =
                prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            let row = owner
                .origins()
                .iter()
                .find(|row| row.store.is_some())
                .unwrap();
            assert_eq!(selected(&owner), 1);
            assert_eq!(row.input, row.output);
            assert_eq!((row.input.block.block, row.input.operation), (3, 0));
            let before = sim(input);
            let after = sim(owner.output());
            let mut nonrepresentative = 0;
            for arm in [false, true] {
                for raw in [0, 1, (1u128 << bits) - 1] {
                    let request = SimulationRequestV1::new(
                        "consensus",
                        [1, 1, 1],
                        [1, 1, 1],
                        vec![
                            SimulationArgumentV1::Scalar(
                                ScalarBitsV1::new(scalar, raw, target).unwrap(),
                            ),
                            SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(arm)),
                            SimulationArgumentV1::Buffer(
                                BufferArgumentV1::new(
                                    scalar,
                                    AccessMode::ReadWrite,
                                    width as u32,
                                    vec![0xa5; 2 * width],
                                    vec![false; 2 * width],
                                    target,
                                )
                                .unwrap(),
                            ),
                        ],
                    );
                    let mut old = Events::default();
                    let mut new = Events::default();
                    let a = before
                        .simulate_observed_with_sink(
                            &request,
                            target,
                            SimulationLimitsV1::default(),
                            &mut old,
                        )
                        .unwrap();
                    let b = after
                        .simulate_observed_with_sink(
                            &request,
                            target,
                            SimulationLimitsV1::default(),
                            &mut new,
                        )
                        .unwrap();
                    assert_eq!(a.arguments(), b.arguments());
                    assert_eq!(a.conflict_assessment(), b.conflict_assessment());
                    for result in [&a, &b] {
                        let buffer = result.buffer(2).unwrap();
                        assert_eq!(&buffer.bytes()[..width], &raw.to_le_bytes()[..width]);
                        assert_eq!(&buffer.bytes()[width..], vec![0xa5; width]);
                        assert_eq!(&buffer.initialized()[..width], vec![true; width]);
                        assert_eq!(&buffer.initialized()[width..], vec![false; width]);
                    }
                    let mut expected = Vec::new();
                    let mut removed = 0;
                    for (index, event) in old.0.iter().enumerate() {
                        if event.site.block == BlockId(40)
                            && event.site.operation == Some(0)
                            && matches!(event.kind, EventKind::MemoryRead { .. })
                        {
                            let EventKind::MemoryRead {
                                allocation,
                                offset,
                                bytes,
                            } = event.kind
                            else {
                                panic!("only the exact original Load observation may disappear");
                            };
                            assert_eq!((offset, bytes), (0, width));
                            let executed = if arm { BlockId(20) } else { BlockId(30) };
                            let writes: Vec<_> = old.0[..index]
                                .iter()
                                .filter(|prior| {
                                    prior.invocation == event.invocation
                                        && prior.kind
                                            == EventKind::MemoryWrite {
                                                allocation,
                                                offset,
                                                bytes,
                                            }
                                })
                                .collect();
                            assert_eq!(writes.len(), 1);
                            assert_eq!(
                                (writes[0].site.block, writes[0].site.operation),
                                (executed, Some(0))
                            );
                            let representative =
                                &input.module().functions[0].body.as_ref().unwrap().blocks
                                    [row.store.unwrap().block.block as usize];
                            nonrepresentative += usize::from(representative.id != executed);
                            removed += 1;
                        } else {
                            expected.push(event.clone());
                        }
                    }
                    assert_eq!(removed, 1);
                    assert_eq!(expected, new.0);
                }
            }
            assert_eq!(nonrepresentative, 3);
            release(owner, budget);
        });
    }
}

fn consensus_record() -> String {
    let (input, floor) = admit(&diamond(ScalarType::U32));
    let (result, work, peak, failed_work, failed_storage) =
        resource_run(&input, floor, WORK, STORAGE);
    let owner = result.unwrap();
    assert_eq!(selected(&owner), 1);
    assert_eq!((failed_work, failed_storage), (None, None));
    format!(
        "{:?}|{:?}|{work}|{peak}",
        owner.output().canonical().canonical_bytes(),
        owner.origins()
    )
}

#[test]
fn store_consensus_fresh_process_child() {
    println!("\nSTORE_CONSENSUS_V45 {}", consensus_record());
}

#[test]
fn store_consensus_two_processes_match_exact_output_lineage_work_and_peak() {
    let expected = consensus_record();
    for _ in 0..2 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "owned_cross_block_forwarding_v1::tests::consensus_v45::store_consensus_fresh_process_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
        let records: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("STORE_CONSENSUS_V45 "))
            .collect();
        assert_eq!(records, [expected.as_str()]);
    }
}
