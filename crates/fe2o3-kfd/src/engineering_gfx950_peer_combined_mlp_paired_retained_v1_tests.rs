use super::*;

struct Fake {
    clock: Instant,
    calls: usize,
    fail: usize,
    after: bool,
    expire: usize,
    generations: Vec<u64>,
    validated: Vec<bool>,
    resets: Vec<[bool; 2]>,
    initial: Vec<bool>,
    events: Vec<String>,
    poisoned: bool,
}

impl Fake {
    fn new(count: usize) -> Self {
        Self {
            clock: Instant::now(),
            calls: 0,
            fail: 0,
            after: false,
            expire: 0,
            generations: vec![1; count],
            validated: vec![false; count],
            resets: vec![[false; 2]; count],
            initial: vec![false; count],
            events: vec![],
            poisoned: false,
        }
    }

    fn step(&mut self, name: String, effect: impl FnOnce(&mut Self)) -> Result<()> {
        assert!(!self.poisoned);
        self.calls += 1;
        self.events.push(name);
        if self.calls == self.fail && !self.after {
            return Err("before effect".into());
        }
        effect(self);
        if self.calls == self.expire {
            self.clock += Duration::from_millis(10);
        }
        if self.calls == self.fail && self.after {
            return Err("after effect".into());
        }
        Ok(())
    }
}

impl RearmBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }
    fn validate(&mut self, pair: usize, until: Instant) -> Result<u64> {
        deadline_check(self.clock, until)?;
        self.step(format!("validate{pair}"), |s| {
            assert!(s.resets.iter().all(|r| *r == [false; 2]));
            s.validated[pair] = true;
        })?;
        Ok(self.generations[pair])
    }
    fn fence(&mut self) -> Result<()> {
        self.step("fence".into(), |_| {})
    }
    fn reset(&mut self, pair: usize, rank: usize, next: u64) -> Result<()> {
        self.step(format!("reset{pair}:{rank}"), |s| {
            assert!(s.validated.iter().all(|value| *value));
            assert_eq!(next, s.generations[pair] + 1);
            assert!(!s.resets[pair][rank]);
            s.resets[pair][rank] = true;
        })
    }
    fn initial(&mut self, pair: usize, next: u64) -> Result<()> {
        self.step(format!("initial{pair}"), |s| {
            assert_eq!(next, s.generations[pair] + 1);
            assert!(s.resets.iter().all(|r| *r == [true; 2]));
            s.initial[pair] = true;
        })
    }
    fn quarantine(&mut self) {
        self.poisoned = true;
        self.events.push("poison".into());
    }
}

#[test]
fn retained_batch_checks_every_pair_before_any_reset() {
    for count in [1, 2, 36, 72] {
        let mut fake = Fake::new(count);
        assert_eq!(rearm_all(&mut fake, count, 10, None).unwrap(), 2);
        assert!(!fake.poisoned);
        assert!(fake.initial.iter().all(|value| *value));
        assert_eq!(fake.calls, count * 4 + 2);
        assert!(
            fake.events[..count]
                .iter()
                .all(|e| e.starts_with("validate"))
        );
        assert_eq!(fake.events[count], "fence");
        assert_eq!(fake.events.last().unwrap(), "fence");
    }
}

#[test]
fn retained_batch_every_failure_boundary_is_terminal() {
    for count in [2, 36] {
        for fail in 1..=count * 4 + 2 {
            for after in [false, true] {
                let mut fake = Fake::new(count);
                fake.fail = fail;
                fake.after = after;
                assert!(rearm_all(&mut fake, count, 10, None).is_err());
                assert!(fake.poisoned);
                assert_eq!(fake.calls, fail);
                assert_eq!(fake.events.last().unwrap(), "poison");
                if fail <= count + 1 {
                    assert!(fake.resets.iter().all(|r| *r == [false; 2]));
                }
            }
        }
    }
}

#[test]
fn retained_batch_every_deadline_boundary_is_terminal() {
    for expire in 1..=10 {
        let mut fake = Fake::new(2);
        fake.expire = expire;
        assert!(rearm_all(&mut fake, 2, 10, None).is_err());
        assert!(fake.poisoned);
        assert_eq!(fake.calls, expire);
    }
}

#[test]
fn retained_batch_uses_the_original_absolute_deadline() {
    let mut fake = Fake::new(2);
    let until = fake.clock;
    assert!(rearm_all(&mut fake, 2, 100, Some(until)).is_err());
    assert_eq!(fake.calls, 0);
    assert!(fake.poisoned);
    for expire in 1..=10 {
        let mut fake = Fake::new(2);
        fake.expire = expire;
        let until = fake.clock + Duration::from_millis(10);
        assert!(rearm_all(&mut fake, 2, 100, Some(until)).is_err());
        assert_eq!(fake.calls, expire);
        assert!(fake.poisoned);
    }
}

#[test]
fn retained_batch_refuses_invalid_bounds_or_any_generation_before_stores() {
    for (count, timeout) in [(0, 10), (73, 10), (2, 0), (2, 10001)] {
        let mut fake = Fake::new(count);
        assert!(rearm_all(&mut fake, count, timeout, None).is_err());
        assert!(fake.poisoned);
        assert_eq!(fake.calls, 0);
    }
    for index in 0..36 {
        for generation in [0, 2, u64::MAX] {
            let mut fake = Fake::new(36);
            fake.generations[index] = generation;
            assert!(rearm_all(&mut fake, 36, 10, None).is_err());
            assert!(fake.poisoned);
            assert!(fake.resets.iter().all(|r| *r == [false; 2]));
        }
    }
}

fn group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 5,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    }
}

fn kernels() -> [Gfx950EngineeringPeerKernelV1; 2] {
    std::array::from_fn(|rank| Gfx950EngineeringPeerKernelV1 {
        group: 7,
        rank,
        id: rank as u64 + 1,
        metadata: KernelMetadataV1 {
            symbol: "invalid".into(),
            object_sha256: [1; 32],
            kernarg_bytes: 0,
            kernarg_alignment: 8,
            group_segment_bytes: 0,
            private_segment_bytes: 0,
            wavefront_size: 64,
            implicit_argument_offset: None,
            implicit_argument_bytes: 0,
            explicit_arguments: vec![],
        },
    })
}

fn inputs(kernels: &[Gfx950EngineeringPeerKernelV1; 2]) -> Inputs<'_> {
    let token = |rank, id| Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id,
        owner: rank,
        bytes: 8192,
    };
    Inputs {
        ranks: std::array::from_fn(|rank| RankInputs {
            kernels: [&kernels[rank]; 4],
            mlp_roots: std::array::from_fn(|i| token(rank, (rank * 12 + i + 10) as u64)),
            residual_input: token(rank, 100 + rank as u64),
            output: token(rank, 110 + rank as u64),
        }),
        partials: [token(0, 120), token(1, 121)],
        projection_sha256: [2; 32],
        mlp_sha256: [3; 32],
    }
}

fn pair(
    group: &Gfx950EngineeringPeerGroupV1,
    kernels: &[Gfx950EngineeringPeerKernelV1; 2],
) -> RetainedPair {
    RetainedPair {
        binding: Binding::capture(group, &inputs(kernels)),
        phase: Phase::Ready,
        completed: None,
        arena_policy: ArenaPolicy::Fresh,
        reusable: None,
        owners: std::array::from_fn(|rank| CombinedMlpStateV1 {
            buffer: Gfx950EngineeringPeerBufferV1 {
                group: 7,
                id: 200 + rank as u64,
                owner: rank,
                bytes: 2208,
            },
            activation: Activation::Ready,
            generation: 1,
        }),
    }
}

#[test]
fn retained_paired_terminal_opt_in_refuses_fresh_before_context_access() {
    let kernels = kernels();
    let mut group = group();
    let mut pair = pair(&group, &kernels);
    let roles = inputs(&kernels);
    let public = Gfx950EngineeringPeerGuardedMlpInputsV1 {
        ranks: roles
            .ranks
            .each_ref()
            .map(|r| Gfx950EngineeringPeerGuardedMlpRankInputsV1 {
                kernels: r.kernels,
                mlp_roots: r.mlp_roots,
                residual_input: r.residual_input,
                output: r.output,
            }),
        partials: roles.partials,
        projection_sha256: roles.projection_sha256,
        mlp_sha256: roles.mlp_sha256,
    };
    let error = group
        .dispatch_guarded_mlp_pair_paired_terminal_v1(&mut pair, public, 10)
        .err()
        .expect("Fresh-policy opt-in must refuse");
    assert_eq!(
        error,
        "paired terminal fences require a reusable retained pair"
    );
    assert_eq!(pair.phase, Phase::Poisoned);
    assert!(
        pair.owners
            .iter()
            .all(|owner| owner.activation == Activation::Poisoned)
    );
    assert!(group.poisoned && group.buffers.is_empty());
    assert_eq!(group.next_buffer, 5);
    assert!(!group.shared_full_currentness);
}

#[test]
fn retained_paired_terminal_opt_in_preserves_missing_reuse_proof_refusal() {
    let kernels = kernels();
    let mut group = group();
    let mut pair = pair(&group, &kernels);
    pair.arena_policy = ArenaPolicy::ReuseRetired;
    for owner in &mut pair.owners {
        owner.generation = 2;
    }
    let error = pair
        .run_paired_terminal(&mut group, inputs(&kernels), 10)
        .unwrap_err();
    assert_eq!(
        error,
        "retained paired arena policy or generation custody changed"
    );
    assert_eq!(pair.phase, Phase::Poisoned);
    assert!(
        pair.owners
            .iter()
            .all(|owner| owner.activation == Activation::Poisoned)
    );
    assert!(group.poisoned && group.buffers.is_empty());
    assert_eq!(group.next_buffer, 5);
}

#[test]
fn retained_arena_policy_preserves_fresh_default_and_requires_rearm_custody() {
    let group = group();
    let kernels = kernels();
    let pair = pair(&group, &kernels);
    assert_eq!(pair.arena_policy, ArenaPolicy::Fresh);
    assert!(pair.reusable.is_none());
    for generation in [0, 1, 2, 1152, u64::MAX] {
        for proof in [false, true] {
            assert_eq!(
                ArenaPolicy::Fresh.require_run(generation, proof).is_ok(),
                generation != 0 && !proof
            );
            assert_eq!(
                ArenaPolicy::ReuseRetired
                    .require_run(generation, proof)
                    .is_ok(),
                generation != 0 && proof == (generation > 1)
            );
        }
    }
}

#[test]
fn retained_reusable_public_bind_consumes_invalid_storage_without_allocating() {
    let kernels = kernels();
    for timeout in [0, 10, 10001] {
        for defect in 0..6 {
            let mut group = group();
            let mut owners = pair(&group, &kernels).owners;
            let mut captured_group = 7;
            match defect {
                0 => {}
                1 => captured_group = 8,
                2 => owners[0].generation = 2,
                3 => owners[1].buffer.bytes = 2192,
                4 => owners[1].activation = Activation::Submitted,
                _ => owners[0].buffer.id = owners[1].buffer.id,
            }
            let roles = inputs(&kernels);
            let public = Gfx950EngineeringPeerGuardedMlpInputsV1 {
                ranks: roles.ranks.each_ref().map(|r| {
                    Gfx950EngineeringPeerGuardedMlpRankInputsV1 {
                        kernels: r.kernels,
                        mlp_roots: r.mlp_roots,
                        residual_input: r.residual_input,
                        output: r.residual_input,
                    }
                }),
                partials: roles.partials,
                projection_sha256: roles.projection_sha256,
                mlp_sha256: roles.mlp_sha256,
            };
            let storage = UnboundPair {
                group: captured_group,
                owners,
            };
            // Context-free fixture cannot authorize a real allocation or GPU.
            assert!(
                unsafe {
                    group.bind_guarded_mlp_pair_exact_own_residual_reusable_unchecked_v1(
                        storage, &public, timeout,
                    )
                }
                .is_err()
            );
            assert!(group.poisoned && group.buffers.is_empty());
            assert_eq!(group.next_buffer, 5);
        }
    }
}

#[test]
fn retained_reusable_missing_proof_is_terminal_not_a_fresh_fallback() {
    let kernels = kernels();
    let mut group = group();
    let mut pair = pair(&group, &kernels);
    pair.arena_policy = ArenaPolicy::ReuseRetired;
    for owner in &mut pair.owners {
        owner.generation = 2;
    }
    let error = pair
        .run(&mut group, inputs(&kernels), 10)
        .err()
        .expect("missing proof must refuse");
    assert_eq!(
        error.to_string(),
        "retained paired arena policy or generation custody changed"
    );
    assert_eq!(pair.phase, Phase::Poisoned);
    assert!(group.poisoned && group.buffers.is_empty());
    assert_eq!(group.next_buffer, 5);
}

#[test]
fn retained_binding_covers_every_ordered_role_kernel_and_image() {
    let group = group();
    let kernels = kernels();
    let original = Binding::capture(&group, &inputs(&kernels));
    for rank in 0..2 {
        for slot in 0..4 {
            for field in 0..4 {
                let mut value = Binding::capture(&group, &inputs(&kernels));
                let kernel = &mut value.kernels[rank][slot];
                match field {
                    0 => kernel.0 += 1,
                    1 => kernel.1 ^= 1,
                    2 => kernel.2 += 1,
                    3 => kernel.3[31] ^= 1,
                    _ => unreachable!(),
                }
                assert!(value != original);
            }
        }
        for slot in 0..13 {
            for field in 0..4 {
                let mut value = Binding::capture(&group, &inputs(&kernels));
                let token = match slot {
                    0..=9 => &mut value.roots[rank][slot],
                    10 => &mut value.partials[rank],
                    11 => &mut value.residuals[rank],
                    _ => &mut value.outputs[rank],
                };
                match field {
                    0 => token.group += 1,
                    1 => token.id += 1,
                    2 => token.owner ^= 1,
                    3 => token.bytes += 1,
                    _ => unreachable!(),
                }
                assert!(value != original);
            }
        }
        let mut value = Binding::capture(&group, &inputs(&kernels));
        value.images[rank][31] ^= 1;
        assert!(value != original);
    }
    let mut value = Binding::capture(&group, &inputs(&kernels));
    value.group += 1;
    assert!(value != original);
    let mut value = Binding::capture(&group, &inputs(&kernels));
    value.roots[0].swap(2, 3);
    assert!(value != original);
}

#[test]
fn retained_native_refuses_before_gpu_and_quarantines_all_custody() {
    fn assert_static<T: 'static>() {}
    assert_static::<RetainedPair>();
    let kernels = kernels();
    for timeout in [0, 10, 10001] {
        let mut group = group();
        assert!(unsafe { RetainedPair::allocate(&mut group, &inputs(&kernels), timeout) }.is_err());
        assert!(group.poisoned);
        assert!(group.buffers.is_empty());
    }
    for phase in [Phase::Ready, Phase::Busy, Phase::Completed, Phase::Poisoned] {
        let mut group = group();
        let mut pair = pair(&group, &kernels);
        pair.phase = phase;
        assert!(pair.run(&mut group, inputs(&kernels), 10).is_err());
        assert_eq!(pair.phase, Phase::Poisoned);
        assert!(group.poisoned);
        assert!(
            pair.owners
                .iter()
                .all(|o| o.activation == Activation::Poisoned)
        );
    }
    let mut group = group();
    let mut pairs = [pair(&group, &kernels), pair(&group, &kernels)];
    assert!(rearm_pairs(&mut group, &mut pairs, 10).is_err());
    assert!(group.poisoned);
    assert!(pairs.iter().all(|p| p.phase == Phase::Poisoned));
}

#[test]
fn retained_diagnostic_refuses_nonquiescent_or_empty_group_before_gpu() {
    let kernels = kernels();
    for phase in [Phase::Ready, Phase::Busy, Phase::Completed, Phase::Poisoned] {
        let mut group = group();
        let mut pair = pair(&group, &kernels);
        pair.phase = phase;
        assert!(pair.observe_for_test(&mut group, 10).is_err());
        assert!(group.poisoned);
        assert_eq!(pair.phase, Phase::Poisoned);
        assert!(
            pair.owners
                .iter()
                .all(|o| o.activation == Activation::Poisoned)
        );
    }
}

#[test]
fn retained_operation_drop_and_unwind_poison_all_pairs() {
    let kernels = kernels();
    for unwind in [false, true] {
        let mut group = group();
        let mut pairs = [pair(&group, &kernels), pair(&group, &kernels)];
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _op = Operation::new(&mut group, &mut pairs);
            if unwind {
                panic!("test operation unwind");
            }
        }));
        assert_eq!(outcome.is_err(), unwind);
        assert!(group.poisoned);
        assert!(pairs.iter().all(|p| {
            p.phase == Phase::Poisoned
                && p.owners
                    .iter()
                    .all(|o| o.activation == Activation::Poisoned)
        }));
    }
    let mut group = group();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _allocation = Allocation {
            group: &mut group,
            owners: vec![],
            committed: false,
        };
        panic!("test allocation unwind");
    }));
    assert!(outcome.is_err());
    assert!(group.poisoned);
}

#[test]
fn retained_exact_residual_binding_keeps_policy_and_roles_immutable() {
    let group = group();
    let kernels = kernels();
    let mut inputs = inputs(&kernels);
    for rank in &mut inputs.ranks {
        rank.output = rank.residual_input;
    }
    let strict = Binding::capture(&group, &inputs);
    let model =
        Binding::capture_with_policy(&group, &inputs, profiles::OutputPolicy::ExactOwnResidual);
    assert!(strict != model);
    assert_eq!(strict.output_policy, profiles::OutputPolicy::Strict);
    assert_eq!(
        model.output_policy,
        profiles::OutputPolicy::ExactOwnResidual
    );
    assert!(model == Binding::capture_with_policy(&group, &inputs, model.output_policy));
    for rank in 0..2 {
        for field in 0..4 {
            let original = inputs.ranks[rank].output;
            match field {
                0 => inputs.ranks[rank].output.group += 1,
                1 => inputs.ranks[rank].output.id += 1,
                2 => inputs.ranks[rank].output.owner ^= 1,
                _ => inputs.ranks[rank].output.bytes += 1,
            }
            assert!(model != Binding::capture_with_policy(&group, &inputs, model.output_policy));
            inputs.ranks[rank].output = original;
        }
    }
}

#[test]
fn retained_exact_residual_constructor_refuses_invalid_custody() {
    let kernels = kernels();
    for timeout in [0, 10, 10001] {
        let mut group = group();
        assert!(
            unsafe {
                RetainedPair::allocate_exact_own_residual(&mut group, &inputs(&kernels), timeout)
            }
            .is_err()
        );
        assert!(group.poisoned && group.buffers.is_empty());
    }
    for phase in [Phase::Ready, Phase::Busy, Phase::Completed, Phase::Poisoned] {
        let mut group = group();
        let mut pair = pair(&group, &kernels);
        pair.binding.output_policy = profiles::OutputPolicy::ExactOwnResidual;
        pair.phase = phase;
        assert!(pair.run(&mut group, inputs(&kernels), 10).is_err());
        assert_eq!(pair.phase, Phase::Poisoned);
        assert!(
            group.poisoned
                && pair
                    .owners
                    .iter()
                    .all(|o| o.activation == Activation::Poisoned)
        );
    }
}

#[test]
fn retained_unbound_identity_requires_initial_private_owners() {
    let group = group();
    let kernels = kernels();
    let fresh = || pair(&group, &kernels).owners;
    assert!(unbound_identity(7, 7, &fresh()).is_ok());
    for (captured, current) in [(0, 0), (0, 7), (7, 0), (7, 8)] {
        assert!(unbound_identity(captured, current, &fresh()).is_err());
    }
    for count in 0..2 {
        assert!(unbound_identity(7, 7, &fresh()[..count]).is_err());
    }
    for rank in 0..2 {
        for field in 0..8 {
            let mut owners = fresh();
            match field {
                0 => owners[rank].buffer.group += 1,
                1 => owners[rank].buffer.id = 0,
                2 => owners[rank].buffer.owner ^= 1,
                3 => owners[rank].buffer.bytes = 2192,
                4 => owners[rank].buffer.bytes = 2209,
                5 => owners[rank].generation = 0,
                6 => owners[rank].generation = 2,
                _ => owners[rank].buffer.id = owners[1 - rank].buffer.id,
            }
            assert!(unbound_identity(7, 7, &owners).is_err());
        }
        for phase in [
            Activation::Allocated,
            Activation::Initialized,
            Activation::Submitted,
            Activation::Completed,
            Activation::Rearming,
            Activation::Poisoned,
        ] {
            let mut owners = fresh();
            owners[rank].activation = phase;
            assert!(unbound_identity(7, 7, &owners).is_err());
        }
    }
}

#[test]
fn retained_unbound_storage_refuses_invalid_custody() {
    fn assert_static<T: 'static>() {}
    assert_static::<UnboundPair>();
    for timeout in [0, 10, 10001] {
        let mut group = group();
        assert!(UnboundPair::allocate(&mut group, timeout).is_err());
        assert!(group.poisoned && group.buffers.is_empty());
        assert_eq!(group.next_buffer, 5);
    }
}

#[test]
fn retained_unbound_bind_consumes_and_quarantines_invalid_custody() {
    let kernels = kernels();
    for exact in [false, true] {
        for timeout in [0, 10, 10001] {
            for defect in 0..6 {
                let mut group = group();
                let mut owners = pair(&group, &kernels).owners;
                let mut captured_group = 7;
                match defect {
                    0 => {} // Even intact identities refuse this context-free Group.
                    1 => captured_group = 8,
                    2 => owners[0].generation = 2,
                    3 => owners[1].buffer.bytes = 2192,
                    4 => owners[1].activation = Activation::Submitted,
                    _ => owners[0].buffer.id = owners[1].buffer.id,
                }
                let storage = UnboundPair {
                    group: captured_group,
                    owners,
                };
                let mut roles = inputs(&kernels);
                if exact {
                    for rank in &mut roles.ranks {
                        rank.output = rank.residual_input;
                    }
                }
                // No GPU contexts exist: every case must refuse before dispatch.
                let result = unsafe {
                    if exact {
                        storage.bind_exact_own_residual(&mut group, &roles, timeout)
                    } else {
                        storage.bind(&mut group, &roles, timeout)
                    }
                };
                assert!(result.is_err());
                assert!(group.poisoned && group.buffers.is_empty());
                assert_eq!(group.next_buffer, 5);
            }
        }
    }
}

#[test]
fn retained_partial_allocation_custody_poisoned_on_drop_and_unwind() {
    let kernels = kernels();
    for count in 0..=2 {
        for unwind in [false, true] {
            let mut group = group();
            let owners = pair(&group, &kernels)
                .owners
                .into_iter()
                .take(count)
                .collect();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let custody = Allocation {
                    group: &mut group,
                    owners,
                    committed: false,
                };
                assert!(!custody.group.poisoned);
                assert!(
                    custody
                        .owners
                        .iter()
                        .all(|o| o.activation == Activation::Ready)
                );
                if unwind {
                    panic!("test partial allocation unwind");
                }
            }));
            assert_eq!(outcome.is_err(), unwind);
            assert!(group.poisoned && group.buffers.is_empty());
        }
    }
}
