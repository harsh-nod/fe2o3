use super::*;
use layer_duration::tests::ScriptClock;

#[derive(Default)]
struct Measured {
    inner: Recording,
    phase_ns: Option<[u64; 6]>,
    body_ns: Option<u64>,
    reject: bool,
    unwind: bool,
}
impl ClosedBackend for Measured {
    type Prefix = u8;
    type Pending = u8;
    type Hidden = u8;
    type Counts = u8;
    type Output = [u8; 4];
    fn enter(&mut self) -> Result<()> {
        self.inner.enter()
    }
    fn prefix(&mut self) -> Result<u8> {
        self.inner.prefix()
    }
    fn mlp(&mut self) -> Result<u8> {
        self.inner.mlp()
    }
    fn hidden(&mut self) -> Result<u8> {
        self.inner.hidden()
    }
    fn exit(&mut self) -> Result<u8> {
        self.inner.exit()
    }
    fn commit(&mut self, a: u8, b: u8, c: u8, d: u8) -> Result<Self::Output> {
        self.inner.event("commit")?;
        Ok([a, b, c, d])
    }
    fn accept_layer_durations(
        &mut self,
        _: &mut Self::Output,
        phases: [u64; 6],
        body: u64,
    ) -> Result<()> {
        self.inner.event("duration_accept")?;
        assert!(!self.unwind, "injected duration acceptance unwind");
        if self.reject {
            return Err("late metric or deadline refusal".into());
        }
        let data = Gfx950EngineeringPeerScopedLayerDurationsV1 {
            phase_ns: phases,
            layer_body_ns: body,
            paired_mlp_phase_ns: [1; 7],
            paired_mlp_body_ns: 7,
        };
        data.validate()?;
        self.phase_ns = Some(phases);
        self.body_ns = Some(body);
        self.inner.published = true;
        Ok(())
    }
    fn quarantine(&mut self) {
        self.inner.quarantine();
    }
}

#[test]
fn scoped_layer_duration_exact_six_stages_before_acceptance() {
    let mut backend = Measured::default();
    let mut clock = ScriptClock::new(7);
    assert_eq!(
        closed_layer_recorded(&mut backend, &mut clock).unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(&backend.inner.events[..6], &TRACE);
    assert_eq!(backend.inner.events[6], "duration_accept");
    assert_eq!(backend.phase_ns, Some([10; 6]));
    assert_eq!(backend.body_ns, Some(60));
    assert_eq!(clock.index, 7);
    assert!(backend.inner.published);
    assert_eq!(backend.inner.poisoned, [false; 5]);
}

#[test]
fn scoped_layer_duration_every_clock_error_and_unwind_quarantines() {
    for boundary in 0..7 {
        for unwind in [false, true] {
            let mut backend = Measured::default();
            let mut clock = ScriptClock::new(7);
            if unwind {
                clock.unwind = Some(boundary);
            } else {
                clock.fail = Some(boundary);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                closed_layer_recorded(&mut backend, &mut clock)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(backend.inner.poisoned, [true; 5]);
            assert!(!backend.inner.published);
            assert_eq!(backend.inner.events.last(), Some(&"quarantine"));
        }
    }
}

#[test]
fn scoped_layer_duration_late_clock_backward_and_overflow_never_publish() {
    for offset in [
        Duration::ZERO,
        Duration::from_secs(u64::MAX / 1_000_000_000 + 1),
    ] {
        let mut backend = Measured::default();
        let mut clock = ScriptClock::new(7);
        clock.offsets[6] = offset;
        assert!(closed_layer_recorded(&mut backend, &mut clock).is_err());
        assert_eq!(&backend.inner.events[..6], &TRACE);
        assert_eq!(backend.inner.poisoned, [true; 5]);
        assert!(!backend.inner.published);
    }
}

#[test]
fn scoped_layer_duration_acceptance_error_and_unwind_keep_owner_armed() {
    for unwind in [false, true] {
        let mut backend = Measured {
            reject: !unwind,
            unwind,
            ..Measured::default()
        };
        let mut clock = ScriptClock::new(7);
        let result = catch_unwind(AssertUnwindSafe(|| {
            closed_layer_recorded(&mut backend, &mut clock)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(backend.inner.poisoned, [true; 5]);
        assert!(!backend.inner.published);
        assert!(backend.phase_ns.is_none());
    }
}

#[test]
fn scoped_layer_duration_preserves_every_effect_refusal_and_unwind() {
    for name in TRACE {
        for unwind in [false, true] {
            let mut backend = Measured::default();
            if unwind {
                backend.inner.unwind = Some(name);
            } else {
                backend.inner.fail = Some(name);
            }
            let mut clock = ScriptClock::new(7);
            let result = catch_unwind(AssertUnwindSafe(|| {
                closed_layer_recorded(&mut backend, &mut clock)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(backend.inner.poisoned, [true; 5]);
            assert!(!backend.inner.published);
        }
    }
}

#[test]
fn scoped_layer_duration_native_acceptance_preserves_census_and_final_deadline() {
    let native = include_str!("engineering_gfx950_peer_scoped_layer_v1.rs");
    let compact: String = native.chars().filter(|c| !c.is_whitespace()).collect();
    let acceptance = compact.rsplit("fnaccept_layer_durations(").next().unwrap();
    let validate = acceptance.find("value.validate()?").unwrap();
    let callbacks = acceptance
        .find("currentness_durations.checked_sum_ns()")
        .unwrap();
    let deadline = acceptance.find("deadline_check(Instant::now()").unwrap();
    let disarm = acceptance.find("self.op.committed=true").unwrap();
    assert!(validate < callbacks && callbacks < deadline && deadline < disarm);
    let census = include_str!("engineering_gfx950_peer_scoped_census_layer_v1.rs");
    let census: String = census.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(census.contains("self.inner.accept_layer_durations(&mutoutput.0,phase_ns,body_ns)"));
    assert!(census.contains("letbefore=self.inner.census()?"));
    assert!(census.contains("letafter=self.inner.census()?"));
}
