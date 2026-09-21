use super::{fixture::*, *};

#[test]
fn already_fixed_input_keeps_one_real_terminal_round_and_exact_rosters() {
    with_input(Module::new("fixed-point-empty"), |input, budget| {
        let owned = finish(input, budget);
        assert_eq!(owned.rounds().len(), 1);
        assert_eq!(
            owned.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        assert_eq!(
            owned.execution().policy_identity(),
            b"FE2O3/CHECKED-SCALAR-FIXED-POINT/V1\0"
        );
        assert_eq!(
            &owned.execution().canonical_bytes()[..16],
            b"F2SFP1\0\0\x01\0\x10\0\x01\0\x02\0"
        );
        for round in owned.rounds() {
            assert_eq!(round.ordinal(), 0);
            assert_eq!(
                round
                    .integer()
                    .report()
                    .passes()
                    .iter()
                    .map(|p| p.pass())
                    .collect::<Vec<_>>(),
                INTEGER_PASSES
            );
            assert_eq!(
                round
                    .scalar()
                    .report()
                    .passes()
                    .iter()
                    .map(|p| p.pass())
                    .collect::<Vec<_>>(),
                SCALAR_PASSES
            );
        }
        assert!(!owned.execution().grants_authority());
        assert!(!owned.grants_authority());
        assert!(!owned.authenticates_compiler_origin());
        owned.replay_against(input, budget).unwrap();
        release(owned, budget);
    });
}

#[test]
fn reverse_physical_order_requires_two_changing_full_rounds_then_terminal() {
    with_input(reverse_chain(1), |input, budget| {
        assert_eq!(binaries(input), 2);
        let owned = finish(input, budget);
        assert_eq!(
            owned.rounds().len(),
            3,
            "two changing COMPLETE rounds, not two integer sweeps"
        );
        let q0 = input.canonical().canonical_bytes();
        let q1 = owned.rounds()[0].output().canonical().canonical_bytes();
        let q2 = owned.rounds()[1].output().canonical().canonical_bytes();
        let q3 = owned.rounds()[2].output().canonical().canonical_bytes();
        assert_ne!(q0, q1);
        assert_ne!(q1, q2);
        assert_eq!(q2, q3);
        assert_eq!(binaries(owned.rounds()[0].integer().owner()), 1);
        assert_eq!(binaries(owned.rounds()[0].output()), 1);
        assert_eq!(binaries(owned.rounds()[1].integer().owner()), 0);
        assert_eq!(binaries(owned.rounds()[1].output()), 0);
        owned.replay_against(input, budget).unwrap();
        let repeat = finish(owned.output(), budget);
        assert_eq!(repeat.rounds().len(), 1);
        assert_eq!(repeat.output().canonical().canonical_bytes(), q3);
        repeat.replay_against(owned.output(), budget).unwrap();
        release(repeat, budget);
        release(owned, budget);
    });
}

#[test]
fn changed_round_cap_refuses_instead_of_publishing_partial_history() {
    for length in [1, 2, 7, 32, 64] {
        with_input(reverse_chain(length), |input, budget| {
            let bytes = input.canonical().canonical_bytes().to_vec();
            let floor = budget.storage();
            for limit in [1, 2] {
                let result = prepare(input, limit, budget);
                assert!(
                    matches!(result, Err(Error::IterationLimit { completed, limit: actual }) if completed == limit && actual == limit)
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(input.canonical().canonical_bytes(), bytes);
            }
            let owned = finish(input, budget);
            assert!(owned.rounds().len() <= SCALAR_FIXED_POINT_MAX_ROUNDS_V1);
            owned.replay_against(input, budget).unwrap();
            release(owned, budget);
        });
    }
    with_input(Module::new("already-fixed-limit-one"), |input, budget| {
        let owner = prepare(input, 1, budget).unwrap();
        assert_eq!(owner.rounds().len(), 1);
        budget.reserve_storage(owner.retained_storage()).unwrap();
        owner.replay_against(input, budget).unwrap();
        release(owner, budget);
        assert!(matches!(prepare(input, 0, budget), Err(Error::History)));
        assert!(matches!(prepare(input, 17, budget), Err(Error::History)));
    });
}

#[test]
fn hostile_round_order_count_header_and_foreign_subjects_fail_replay() {
    with_input(reverse_chain(1), |input, budget| {
        let mut owner = finish(input, budget);
        for offset in 0..SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 {
            owner.execution.bytes[offset] ^= 1;
            assert!(matches!(
                owner.replay_against(input, budget),
                Err(Error::History)
            ));
            owner.execution.bytes[offset] ^= 1;
        }
        let original = owner.output().canonical().canonical_bytes().to_vec();
        owner.rounds.swap(0, 1);
        assert!(owner.replay_against(input, budget).is_err());
        owner.rounds.swap(0, 1);
        owner.rounds[0].ordinal = 1;
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::History)
        ));
        owner.rounds[0].ordinal = 0;
        let old_retained = owner.retained;
        owner.retained += 1;
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::History)
        ));
        owner.retained = old_retained;
        let rounds = std::mem::take(&mut owner.rounds);
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::History)
        ));
        owner.rounds = rounds;
        let terminal = owner.rounds.pop().unwrap();
        // Recompute outer metadata to ensure the actual terminal check rejects,
        // not merely the stale count/receipt. The removed owner stays caller-paid.
        resources::scoped(budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
            meter.reserve(CHECK_SCRATCH)?;
            owner.execution.bytes = record(input, owner.output(), owner.rounds.len(), meter)?;
            owner.retained = retained(owner.rounds.capacity(), &owner.rounds)?;
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::History)
        ));
        owner.rounds.push(terminal);
        owner.retained = old_retained;
        resources::scoped(budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
            meter.reserve(CHECK_SCRATCH)?;
            owner.execution.bytes = record(input, owner.output(), owner.rounds.len(), meter)?;
            Ok(())
        })
        .unwrap();
        let donor = finish(owner.output(), budget);
        let mut donor = donor;
        let donor_retained = donor.retained;
        std::mem::swap(&mut owner.rounds[0].integer, &mut donor.rounds[0].integer);
        owner.retained = retained(owner.rounds.capacity(), &owner.rounds).unwrap();
        donor.retained = retained(donor.rounds.capacity(), &donor.rounds).unwrap();
        assert!(owner.replay_against(input, budget).is_err());
        assert_eq!(owner.output().canonical().canonical_bytes(), original);
        std::mem::swap(&mut owner.rounds[0].integer, &mut donor.rounds[0].integer);
        owner.retained = old_retained;
        donor.retained = donor_retained;
        // Move a genuine extra terminal round into existing paid spare capacity.
        // Its full receipt remains paid through donor; no cloned witness exists.
        let mut extra = donor.rounds.pop().unwrap();
        extra.ordinal = owner.rounds.len() as u16;
        assert!(owner.rounds.len() < owner.rounds.capacity());
        owner.rounds.push(extra);
        resources::scoped(budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
            meter.reserve(CHECK_SCRATCH)?;
            owner.execution.bytes = record(input, owner.output(), owner.rounds.len(), meter)?;
            owner.retained = retained(owner.rounds.capacity(), &owner.rounds)?;
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::History)
        ));
        let mut extra = owner.rounds.pop().unwrap();
        extra.ordinal = 0;
        donor.rounds.push(extra);
        owner.retained = old_retained;
        resources::scoped(budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
            meter.reserve(CHECK_SCRATCH)?;
            owner.execution.bytes = record(input, owner.output(), owner.rounds.len(), meter)?;
            Ok(())
        })
        .unwrap();
        release(donor, budget);
        let mut foreign_module = reverse_chain(1);
        foreign_module.id = "foreign-input".into();
        let (foreign, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&foreign_module, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(owner.replay_against(&foreign, budget).is_err());
        drop(foreign);
        budget.release_storage(receipt.retained_storage()).unwrap();
        owner.replay_against(input, budget).unwrap();
        release(owner, budget);
    });
}

#[test]
fn early_pair_rows_are_checked_even_when_final_bytes_are_unchanged() {
    with_input(reverse_chain(1), |input, budget| {
        let owner = finish(input, budget);
        let round = &owner.rounds()[0];
        resources::scoped(budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
            let mut rows = round.integer().occurrences().candidate();
            assert!(!rows.uses.is_empty());
            rows.uses = &[];
            assert!(matches!(
                replay::pair(input, round.integer().owner(), rows, meter),
                Err(Error::Transition(_))
            ));
            Ok(())
        })
        .unwrap();
        owner.replay_against(input, budget).unwrap();
        release(owner, budget);
    });
}

#[test]
fn all_integer_neutral_families_and_live_checked_flags_compose_across_every_width() {
    for (zero, one, mask) in [
        (Constant::I8(0), Constant::I8(1), Constant::I8(-1)),
        (Constant::I16(0), Constant::I16(1), Constant::I16(-1)),
        (Constant::I32(0), Constant::I32(1), Constant::I32(-1)),
        (Constant::I64(0), Constant::I64(1), Constant::I64(-1)),
        (Constant::U8(0), Constant::U8(1), Constant::U8(u8::MAX)),
        (Constant::U16(0), Constant::U16(1), Constant::U16(u16::MAX)),
        (Constant::U32(0), Constant::U32(1), Constant::U32(u32::MAX)),
        (Constant::U64(0), Constant::U64(1), Constant::U64(u64::MAX)),
    ] {
        let ty = zero.ty();
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            op(1, ty.clone(), Kind::Constant(zero)),
            op(2, ty.clone(), Kind::Constant(one)),
            op(3, ty.clone(), Kind::Constant(mask)),
        ];
        let mut value = 0;
        let mut next = 4;
        for (operator, literal) in [
            (BinaryOp::Add, 1),
            (BinaryOp::Subtract, 1),
            (BinaryOp::Multiply, 2),
            (BinaryOp::BitAnd, 3),
            (BinaryOp::BitOr, 1),
            (BinaryOp::BitXor, 1),
        ] {
            for swap in [false, true] {
                if swap && operator == BinaryOp::Subtract {
                    continue;
                }
                let (lhs, rhs) = if swap {
                    (literal, value)
                } else {
                    (value, literal)
                };
                entry.operations.push(op(
                    next,
                    ty.clone(),
                    Kind::Binary {
                        op: operator,
                        lhs: ValueId(lhs),
                        rhs: ValueId(rhs),
                    },
                ));
                value = next;
                next += 1;
            }
        }
        let mut flags = Vec::new();
        for (operator, literal) in [
            (CheckedBinaryOperator::Add, 1),
            (CheckedBinaryOperator::Subtract, 1),
            (CheckedBinaryOperator::Multiply, 2),
        ] {
            for swap in [false, true] {
                if swap && operator == CheckedBinaryOperator::Subtract {
                    continue;
                }
                let (lhs, rhs) = if swap {
                    (literal, value)
                } else {
                    (value, literal)
                };
                entry.operations.push(Operation::checked_binary(
                    ValueDef::new(ValueId(next), ty.clone()),
                    ValueDef::new(ValueId(next + 1), Type::BOOL),
                    operator,
                    ValueId(lhs),
                    ValueId(rhs),
                ));
                value = next;
                flags.push(ValueId(next + 1));
                next += 2;
            }
        }
        let mut results = vec![ty.clone()];
        results.extend(vec![Type::BOOL; flags.len()]);
        let mut returned = vec![ValueId(value)];
        returned.extend(flags);
        entry.terminator = Some(Terminator::Return { values: returned });
        let mut module = Module::new("all-scalar-neutral-families");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![ty], results),
            vec![ValueId(0)],
            vec![entry],
        ));
        with_input(module, |input, budget| {
            assert_eq!(binaries(input), 16);
            let owner = finish(input, budget);
            assert_eq!(binaries(owner.output()), 0);
            let body = owner.output().module().functions[0].body.as_ref().unwrap();
            let Some(Terminator::Return { values }) = &body.blocks[0].terminator else {
                panic!("return");
            };
            assert_eq!(values.len(), 6);
            assert_eq!(values[0], body.parameters[0]);
            for flag in &values[1..] {
                assert!(
                    body.blocks[0].operations.iter().any(|op| op
                        .results
                        .iter()
                        .any(|result| result.id == *flag)
                        && op.kind == Kind::Constant(Constant::Bool(false)))
                );
            }
            owner.replay_against(input, budget).unwrap();
            release(owner, budget);
        });
    }
}

#[test]
fn integer_widths_live_overflow_flags_boundary_values_and_non_neutral_controls_simulate() {
    let mut runs = 0;
    for zero in [
        Constant::I8(0),
        Constant::I16(0),
        Constant::I32(0),
        Constant::I64(0),
        Constant::U8(0),
        Constant::U16(0),
        Constant::U32(0),
        Constant::U64(0),
    ] {
        let Type::Scalar(scalar) = zero.ty() else {
            unreachable!()
        };
        let bits = width(scalar) * 8;
        let mask = (1u128 << bits) - 1;
        let sign = 1u128 << (bits - 1);
        let signed = matches!(
            scalar,
            ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64
        );
        for neighbor in [false, true] {
            with_input(integer_kernel(zero.clone(), neighbor), |input, budget| {
                let owner = finish(input, budget);
                assert_eq!(binaries(owner.output()), usize::from(neighbor));
                owner.replay_against(input, budget).unwrap();
                let before = sim(input);
                let after = sim(owner.output());
                let mut values = vec![0, 1, sign - 1, sign, mask];
                if bits == 8 {
                    values.extend(0..=255);
                }
                let mut seed = 0x539au128;
                for _ in 0..16 {
                    seed = (seed * 1_664_525 + 1_013_904_223) & mask;
                    values.push(seed);
                }
                for value in values {
                    let output =
                        compare_sim(&before, &after, &scalar_request(scalar, value, true), false)
                            .unwrap();
                    let SimulationArgumentV1::Buffer(data) = &output[1] else {
                        panic!("data buffer")
                    };
                    let SimulationArgumentV1::Buffer(flag) = &output[2] else {
                        panic!("flag buffer")
                    };
                    let expected = (value + u128::from(neighbor)) & mask;
                    assert_eq!(
                        &data.bytes()[..width(scalar)],
                        &expected.to_le_bytes()[..width(scalar)]
                    );
                    assert_eq!(&data.bytes()[width(scalar)..], vec![0xa5; width(scalar)]);
                    let overflow = neighbor && value == if signed { sign - 1 } else { mask };
                    assert_eq!(flag.bytes(), &[u8::from(overflow), 0xa5]);
                    assert!(data.initialized()[..width(scalar)].iter().all(|v| *v));
                    assert!(data.initialized()[width(scalar)..].iter().all(|v| !v));
                    runs += 1;
                }
                release(owner, budget);
            });
        }
    }
    assert!(runs > 1000);
}

#[test]
fn strict_ieee_half_single_double_keep_addition_and_special_value_behavior() {
    for (zero, values) in [
        (
            Constant::F16Bits(0),
            vec![0, 0x8000, 0x7c00, 0xfc00, 0x7e01, 1],
        ),
        (
            Constant::F32Bits(0),
            vec![0, 0x8000_0000, 0x7f80_0000, 0xff80_0000, 0x7fc0_0001, 1],
        ),
        (
            Constant::F64Bits(0),
            vec![
                0,
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0xfff0_0000_0000_0000,
                0x7ff8_0000_0000_0001,
                1,
            ],
        ),
    ] {
        let Type::Scalar(scalar) = zero.ty() else {
            unreachable!()
        };
        with_input(float_kernel(zero), |input, budget| {
            let owner = finish(input, budget);
            assert_eq!(
                binaries(owner.output()),
                1,
                "integer rules must not erase IEEE x + 0"
            );
            owner.replay_against(input, budget).unwrap();
            let before = sim(input);
            let after = sim(owner.output());
            for bits in values {
                compare_sim(&before, &after, &scalar_request(scalar, bits, false), false);
            }
            release(owner, budget);
        });
    }
}

#[test]
fn cyclic_phi_duplicate_edges_unreachable_blocks_volatile_effects_and_traps_survive() {
    for trap in [false, true] {
        for volatile in [false, true] {
            with_input(loop_kernel(trap, volatile), |input, budget| {
                let owner = finish(input, budget);
                assert!(owner.rounds().len() >= 2);
                owner.replay_against(input, budget).unwrap();
                let before = sim(input);
                let after = sim(owner.output());
                for bound in [0, 1, 2, 7] {
                    for x in [0, 1, u32::MAX] {
                        for condition in [false, true] {
                            let request = SimulationRequestV1::new(
                                "k",
                                [1, 1, 1],
                                [1, 1, 1],
                                vec![
                                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(bound)),
                                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(x)),
                                    buffer(ScalarType::U32),
                                    SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(condition)),
                                ],
                            );
                            let output = compare_sim(&before, &after, &request, trap && bound != 0);
                            if let Some(output) = output {
                                let SimulationArgumentV1::Buffer(data) = &output[2] else {
                                    panic!("data")
                                };
                                let expected = if bound == 0 {
                                    [0xa5; 4]
                                } else {
                                    x.to_le_bytes()
                                };
                                assert_eq!(&data.bytes()[..4], &expected);
                                assert_eq!(&data.bytes()[4..], &[0xa5; 4]);
                            }
                        }
                    }
                }
                release(owner, budget);
            });
        }
    }
}

#[test]
fn aliasing_memory_operations_remain_exact_through_scalar_cleanup() {
    for volatile in [false, true] {
        let mut module = crate::checked_load_forwarding_v1::tests::fixture();
        if volatile {
            for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let Kind::Load { access, .. } | Kind::Store { access, .. } = &mut operation.kind
                {
                    access.volatile = true;
                }
            }
        }
        with_input(module, |input, budget| {
            let owner = finish(input, budget);
            let memory = |owner: &Owner| {
                operations(owner)
                    .filter(|op| {
                        matches!(
                            op.kind,
                            Kind::Alloca { .. } | Kind::Load { .. } | Kind::Store { .. }
                        )
                    })
                    .map(|op| op.kind.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(memory(input), memory(owner.output()));
            owner.replay_against(input, budget).unwrap();
            release(owner, budget);
        });
    }
}

#[test]
fn atomic_ordering_barriers_unknown_call_wave_and_inline_assembly_are_not_scalar_rules() {
    use fe2o3_kernel_ir::{
        AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity, Atomic,
        AtomicKind, Barrier, BarrierSemantics, Convergence, Fence, InlineAssembly,
        InlineAssemblyTarget, MemoryOrdering, SynchronizationScope as Scope, WaveOperation,
        WaveOperationKind, WaveWidth, WorkgroupBarrier,
    };
    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = block(
        0,
        Terminator::Return {
            values: vec![ValueId(4), ValueId(5)],
        },
    );
    entry.operations = vec![
        op(
            3,
            ty.clone(),
            Kind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(1)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: Scope::Device,
                ordering: MemoryOrdering::AcquireRelease,
                failure_ordering: None,
            }),
        ),
        Operation::new(
            vec![],
            Kind::Fence(Fence {
                memory_scope: Scope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
            }),
        ),
        Operation::new(
            vec![],
            Kind::Barrier(Barrier {
                execution_scope: Scope::Workgroup,
                memory_scope: Scope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
            }),
        ),
        Operation::new(
            vec![],
            Kind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: Scope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(Scope::Workgroup),
            }),
        ),
        Operation::new(
            vec![],
            Kind::Call {
                callee: "unknown".into(),
                arguments: vec![ValueId(0)],
            },
        ),
        op(
            4,
            ty.clone(),
            Kind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ),
        op(
            5,
            ty.clone(),
            Kind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                // Inert structural identity only, never a signed-source positive.
                source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                mnemonic: "v_xor_b32".into(),
                operands: vec![
                    AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                    AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
                    AssemblyOperand::input(ValueId(2), AssemblyConstraint::Vgpr32),
                ],
                options: [AssemblyOption::NoMemory].into(),
                declared_effects: Default::default(),
            }),
        ),
    ];
    let pointer = Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut function = Function::internal_helper(
        "f",
        Signature::new(
            vec![pointer.clone(), ty.clone(), ty.clone()],
            vec![ty.clone(), ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut module = Module::new("scalar-effects-are-not-identities");
    module.functions.push(function);
    module.functions.push(Function::external_import(
        "unknown",
        Signature::new(vec![pointer], vec![]),
    ));
    with_input(module, |input, budget| {
        let owner = finish(input, budget);
        assert_eq!(owner.rounds().len(), 1);
        assert_eq!(
            input.canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        assert_eq!(operations(input).count(), 7);
        owner.replay_against(input, budget).unwrap();
        release(owner, budget);
    });
}
