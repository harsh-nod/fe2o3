fn token() -> StateToken {
    StateToken {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3,
            owner: 0,
            bytes: STATE_BYTES as u64,
        },
    }
}

fn complete() -> [u32; STATE_WORDS] {
    let tasks = STATE_WORDS - 6;
    let mut words = [64; STATE_WORDS];
    words[..6].copy_from_slice(&[
        1,
        0,
        (1 << tasks) - 1,
        (1 << tasks) - 1,
        (0..tasks).fold(0, |v, task| v | (1 << (task * 2))),
        0,
    ]);
    words
}

struct Fake {
    words: [u32; STATE_WORDS],
    calls: Vec<&'static str>,
    fail: Option<usize>,
    corrupt: bool,
}

impl Fake {
    fn new() -> Self {
        Self {
            words: complete(),
            calls: vec![],
            fail: None,
            corrupt: false,
        }
    }
    fn step(&mut self, name: &'static str) -> Result<()> {
        let index = self.calls.len();
        self.calls.push(name);
        if self.fail == Some(index) {
            Err("injected rearm failure".into())
        } else {
            Ok(())
        }
    }
}

impl StateBackend for Fake {
    fn check(&mut self) -> Result<()> {
        self.step("check")
    }
    fn allocate(&mut self) -> Result<StateToken> {
        panic!("rearm must not allocate")
    }
    fn initialize(&mut self, _: &StateToken) -> Result<()> {
        panic!("rearm must not reconstruct atomics")
    }
    fn observe(&mut self, _: &StateToken) -> Result<[u32; STATE_WORDS]> {
        self.step("observe")?;
        Ok(self.words)
    }
}

impl RearmBackend for Fake {
    fn rearm(&mut self, _: &StateToken) -> Result<()> {
        self.step("rearm")?;
        self.words = initial_state();
        if self.corrupt {
            self.words[5] = 1;
        }
        Ok(())
    }
}

#[test]
fn rearm_orders_exact_terminal_observation_atomic_stores_readback_and_idle() {
    let mut backend = Fake::new();
    rearm_terminal(&mut backend, &token(), &complete()).unwrap();
    assert_eq!(
        backend.calls,
        ["check", "observe", "rearm", "observe", "check"]
    );
    assert_eq!(backend.words, initial_state());
    assert!(rearm_terminal(&mut backend, &token(), &complete()).is_err());
}

#[test]
fn malformed_or_changed_terminal_state_cannot_reach_a_write() {
    for index in 0..STATE_WORDS {
        let mut expected = complete();
        expected[index] = if index == 5 { 1 } else { 0 };
        if expected == complete() {
            continue;
        }
        let mut backend = Fake::new();
        assert!(rearm_terminal(&mut backend, &token(), &expected).is_err());
        assert!(!backend.calls.contains(&"rearm"));
    }
    let mut backend = Fake::new();
    backend.words[STATE_WORDS - 1] = 63;
    assert!(rearm_terminal(&mut backend, &token(), &complete()).is_err());
    assert_eq!(backend.calls, ["check", "observe"]);
}

#[test]
fn every_rearm_failure_is_quarantined_and_corrupt_readback_refused() {
    for index in 0..5 {
        let mut backend = Fake::new();
        backend.fail = Some(index);
        let mut group = Gfx950EngineeringPeerGroupV1 {
            incarnation: 7,
            contexts: vec![],
            buffers: Default::default(),
            next_buffer: 1,
            poisoned: false,
            closed: false,
            shared_full_currentness: false,
            projection_mlp_scratch: None,
        };
        assert!(
            group
                .finish(rearm_terminal(&mut backend, &token(), &complete()))
                .is_err()
        );
        assert!(group.poisoned);
    }
    let mut backend = Fake::new();
    backend.corrupt = true;
    assert!(rearm_terminal(&mut backend, &token(), &complete()).is_err());
}
