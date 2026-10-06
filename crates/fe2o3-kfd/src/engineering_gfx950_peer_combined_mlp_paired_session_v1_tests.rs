use super::*;

fn terminal(generation: u64) -> CombinedMlpSnapshotV1 {
    let mut prefix = [64; PREFIX_WORDS];
    prefix[..4].copy_from_slice(&[1, 0, 0, 31]);
    prefix[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[14..22].fill(u32::MAX);
    prefix[22] = 3;
    prefix[23..31].fill(u32::MAX);
    prefix[31] = 3;
    CombinedMlpSnapshotV1 {
        prefix,
        guard: guard_words(generation, 1),
    }
}

fn completion(generation: u64) -> Completion {
    Completion {
        states: [terminal(generation), terminal(generation)],
        observed_queue_frontiers: [(5, 3); 2],
        segment_host_ns: 1,
    }
}

struct Fake {
    clock: Instant,
    calls: usize,
    fail_at: usize,
    after_effect: bool,
    expire_at: usize,
    events: Vec<String>,
    generation: u64,
    actual: [CombinedMlpSnapshotV1; 2],
    guard_passed: bool,
    quiescent_error: bool,
    initial_error: bool,
    reset: [bool; 2],
    poisoned: [bool; 3],
    poison_calls: usize,
}

impl Fake {
    fn new() -> Self {
        Self {
            clock: Instant::now(),
            calls: 0,
            fail_at: 0,
            after_effect: false,
            expire_at: 0,
            events: vec![],
            generation: 1,
            actual: completion(1).states,
            guard_passed: false,
            quiescent_error: false,
            initial_error: false,
            reset: [false; 2],
            poisoned: [false; 3],
            poison_calls: 0,
        }
    }

    fn step(&mut self, name: String, effect: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        assert_eq!(self.poison_calls, 0, "no backend work after quarantine");
        self.calls += 1;
        self.events.push(name);
        if self.calls == self.fail_at && !self.after_effect {
            return Err("before effect".into());
        }
        effect(self)?;
        if self.calls == self.expire_at {
            self.clock += Duration::from_millis(10);
        }
        if self.calls == self.fail_at && self.after_effect {
            return Err("after effect".into());
        }
        Ok(())
    }
}

impl RearmBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }
    fn quiescent(
        &mut self,
        old: u64,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<()> {
        self.step("quiescent".into(), |s| {
            deadline_check(s.clock, deadline)?;
            if s.quiescent_error || s.generation != old || s.actual != *states {
                return Err("actual quiescence/generation/snapshot refused".into());
            }
            for state in states {
                require_terminal(state, old)?;
            }
            assert_eq!(s.reset, [false; 2]);
            s.guard_passed = true;
            Ok(())
        })
    }
    fn reset(&mut self, rank: usize, expected: &CombinedMlpSnapshotV1, next: u64) -> Result<()> {
        self.step(format!("reset{rank}"), |s| {
            assert!(s.guard_passed);
            assert_eq!(expected, &s.actual[rank]);
            assert_eq!(next, s.generation + 1);
            assert_eq!(s.reset, if rank == 0 { [false; 2] } else { [true, false] });
            s.reset[rank] = true;
            Ok(())
        })
    }
    fn initial(&mut self, next: u64) -> Result<()> {
        self.step("initial".into(), |s| {
            assert_eq!(s.reset, [true; 2]);
            assert_eq!(next, s.generation + 1);
            if s.initial_error {
                return Err("initial paired readback refused".into());
            }
            Ok(())
        })
    }
    fn quarantine(&mut self) {
        self.poison_calls += 1;
        self.poisoned.fill(true);
        self.events.push("poison".into());
    }
}

#[test]
fn session_rearm_checks_pair_before_any_reset() {
    let mut fake = Fake::new();
    assert_eq!(
        rearm_pair(&mut fake, 1, &completion(1).states, 10).unwrap(),
        2
    );
    assert_eq!(fake.events, ["quiescent", "reset0", "reset1", "initial"]);
    assert_eq!(fake.poison_calls, 0);
    assert_eq!(fake.reset, [true; 2]);
}

#[test]
fn session_rearm_each_failure_before_and_after_effect_is_terminal() {
    for fail_at in 1..=4 {
        for after_effect in [false, true] {
            let mut fake = Fake::new();
            fake.fail_at = fail_at;
            fake.after_effect = after_effect;
            assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
            assert_eq!(fake.calls, fail_at);
            assert_eq!(fake.poison_calls, 1);
            assert_eq!(fake.poisoned, [true; 3]);
            assert_eq!(fake.events.last().unwrap(), "poison");
            if fail_at == 3 {
                assert!(fake.reset[0], "no rollback after rank0 reset");
            }
        }
    }
}

#[test]
fn session_rearm_each_deadline_boundary_including_commit_is_terminal() {
    for expire_at in 1..=4 {
        let mut fake = Fake::new();
        fake.expire_at = expire_at;
        assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
        assert_eq!(fake.calls, expire_at);
        assert_eq!(fake.poison_calls, 1);
        assert_eq!(fake.poisoned, [true; 3]);
    }
}

#[test]
fn session_rearm_invalid_timeout_zero_and_overflow_do_not_touch_state() {
    for (old, timeout) in [(1, 0), (1, 10_001), (1, u32::MAX), (0, 10), (u64::MAX, 10)] {
        let mut fake = Fake::new();
        assert!(rearm_pair(&mut fake, old, &completion(1).states, timeout).is_err());
        assert_eq!(fake.calls, 0);
        assert_eq!(fake.reset, [false; 2]);
        assert_eq!(fake.poison_calls, 1);
    }
}

#[test]
fn session_rearm_peer_snapshot_or_generation_drift_refuses_before_stores() {
    for rank in 0..2 {
        for word in 0..PREFIX_WORDS + 4 {
            let mut fake = Fake::new();
            if word < PREFIX_WORDS {
                fake.actual[rank].prefix[word] ^= 1;
            } else {
                fake.actual[rank].guard[word - PREFIX_WORDS] ^= 1;
            }
            assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
            assert_eq!(fake.reset, [false; 2]);
            assert_eq!(fake.calls, 1);
            assert_eq!(fake.poison_calls, 1);
        }
    }
    let mut fake = Fake::new();
    fake.generation = 2;
    assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
    assert_eq!(fake.reset, [false; 2]);
}

#[test]
fn session_rearm_propagates_quiescence_and_final_readback_refusals() {
    // These exercise orchestration error propagation, not device signal/fault
    // injection. Native::finish and the existing arena tests own those checks.
    let mut fake = Fake::new();
    fake.quiescent_error = true;
    assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
    assert_eq!(fake.reset, [false; 2]);
    assert_eq!(fake.poisoned, [true; 3]);
    let mut fake = Fake::new();
    fake.initial_error = true;
    assert!(rearm_pair(&mut fake, 1, &completion(1).states, 10).is_err());
    assert_eq!(fake.reset, [true; 2]);
    assert_eq!(fake.poisoned, [true; 3]);
}

#[test]
fn session_custody_does_not_accept_replay_or_caller_mutated_completion() {
    let mut custody = Custody::Ready;
    custody.begin_run().unwrap();
    let mut reported = completion(1);
    custody.complete(1, &reported).unwrap();
    reported.states[1].prefix[547] = 0;
    reported.states[0].guard[0] = 99;
    let (old, retained) = custody.begin_rearm().unwrap();
    assert_eq!(old, 1);
    assert_eq!(retained, completion(1).states);
    assert!(matches!(custody, Custody::Busy));
    for mut phase in [
        Custody::Busy,
        Custody::Poisoned,
        Custody::Idle {
            generation: 1,
            states: completion(1).states,
        },
    ] {
        assert!(phase.begin_run().is_err());
    }
    for mut phase in [Custody::Ready, Custody::Busy, Custody::Poisoned] {
        assert!(phase.begin_rearm().is_err());
    }
    for mut phase in [
        Custody::Ready,
        Custody::Poisoned,
        Custody::Idle {
            generation: 1,
            states: completion(1).states,
        },
    ] {
        assert!(phase.complete(1, &completion(1)).is_err());
    }
    assert!(Custody::Busy.complete(0, &completion(1)).is_err());
    assert!(Custody::Busy.complete(2, &completion(1)).is_err());
}

#[test]
fn session_transfer_phase_policy_allows_writes_only_ready() {
    for (phase, read, write) in [
        (Custody::Ready, true, true),
        (Custody::Busy, false, false),
        (
            Custody::Idle {
                generation: 1,
                states: completion(1).states,
            },
            true,
            false,
        ),
        (Custody::Poisoned, false, false),
    ] {
        assert_eq!(phase.transfer(false).is_ok(), read);
        assert_eq!(phase.transfer(true).is_ok(), write);
    }
}

fn without_gpu(check: impl FnOnce(&mut Session<'_, '_>)) {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 5,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    let mut owners = std::array::from_fn(|rank| CombinedMlpStateV1 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3 + rank as u64,
            owner: rank,
            bytes: 2208,
        },
        activation: Activation::Ready,
        generation: 1,
    });
    // Deliberately inadmissible images; each test must refuse before loading,
    // allocation, publication, or any GPU backend call.
    let kernels: [Gfx950EngineeringPeerKernelV1; 2] =
        std::array::from_fn(|rank| Gfx950EngineeringPeerKernelV1 {
            group: 7,
            rank,
            id: 1,
            metadata: KernelMetadataV1 {
                symbol: "invalid".into(),
                object_sha256: [0; 32],
                kernarg_bytes: 0,
                kernarg_alignment: 8,
                group_segment_bytes: 0,
                private_segment_bytes: 0,
                wavefront_size: 64,
                implicit_argument_offset: None,
                implicit_argument_bytes: 0,
                explicit_arguments: vec![],
            },
        });
    let inputs = Inputs {
        ranks: std::array::from_fn(|rank| RankInputs {
            kernels: [&kernels[rank]; 4],
            mlp_roots: [owners[rank].buffer; 10],
            residual_input: owners[rank].buffer,
            output: owners[rank].buffer,
        }),
        partials: [owners[0].buffer, owners[1].buffer],
        projection_sha256: [1; 32],
        mlp_sha256: [1; 32],
    };
    let mut session = unsafe { Session::new(&mut group, &mut owners, inputs) };
    check(&mut session);
    drop(session);
    assert!(group.poisoned);
    assert!(
        owners
            .iter()
            .all(|owner| owner.activation == Activation::Poisoned)
    );
}

#[test]
fn session_native_entry_replay_and_transfer_errors_quarantine_both_owners() {
    without_gpu(|session| {
        assert!(session.run(10).is_err());
        assert!(matches!(session.custody, Custody::Poisoned));
        assert!(session.run(10).is_err());
        assert!(session.rearm_next(10).is_err());
    });
    without_gpu(|session| {
        assert!(session.rearm_next(10).is_err());
    });
    without_gpu(|session| {
        let token = session.native.inputs.partials[0];
        assert!(session.read(token, 0, 4).is_err());
        assert!(matches!(session.custody, Custody::Poisoned));
    });
    without_gpu(|session| {
        let mut token = session.native.inputs.partials[0];
        token.group += 1;
        assert!(session.write(token, 0, &[0; 4]).is_err());
        assert!(matches!(session.custody, Custody::Poisoned));
    });
    without_gpu(|session| {
        session.custody = Custody::Idle {
            generation: 1,
            states: completion(1).states,
        };
        let token = session.native.inputs.partials[0];
        assert!(session.write(token, 0, &[0; 4]).is_err());
    });
}

#[test]
fn session_busy_drop_and_native_generation_drift_are_terminal() {
    without_gpu(|session| {
        session.custody = Custody::Busy;
    });
    without_gpu(|session| {
        session.custody = Custody::Idle {
            generation: 1,
            states: completion(1).states,
        };
        session.native.generation = 2;
        assert!(session.rearm_next(10).is_err());
        assert!(session.native.staged.is_none());
        assert!(matches!(session.custody, Custody::Poisoned));
    });
}
