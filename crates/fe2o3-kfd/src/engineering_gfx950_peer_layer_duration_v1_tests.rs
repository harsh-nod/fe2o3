use super::*;

pub(crate) struct ScriptClock {
    pub(crate) base: Instant,
    pub(crate) offsets: Vec<Duration>,
    pub(crate) index: usize,
    pub(crate) fail: Option<usize>,
    pub(crate) unwind: Option<usize>,
}
impl ScriptClock {
    pub(crate) fn new(marks: usize) -> Self {
        Self {
            base: Instant::now(),
            offsets: (0..marks)
                .map(|i| Duration::from_nanos(i as u64 * 10))
                .collect(),
            index: 0,
            fail: None,
            unwind: None,
        }
    }
}
impl Clock for ScriptClock {
    fn now(&mut self) -> Result<Instant> {
        let index = self.index;
        self.index += 1;
        assert_ne!(self.unwind, Some(index), "injected duration clock unwind");
        if self.fail == Some(index) {
            return Err("injected duration clock error".into());
        }
        self.base
            .checked_add(
                *self
                    .offsets
                    .get(index)
                    .ok_or("unexpected duration clock call")?,
            )
            .ok_or_else(|| "test duration Instant overflow".into())
    }
}

#[test]
fn layer_duration_recorder_exact_fixed_boundaries_and_zero_intervals() {
    let mut clock = ScriptClock::new(7);
    let mut recording = Recorder::<6>::new(&mut clock);
    for _ in 0..7 {
        recording.mark().unwrap();
    }
    assert_eq!(recording.finish().unwrap(), ([10; 6], 60));
    assert_eq!(clock.index, 7);
    let mut clock = ScriptClock::new(8);
    clock.offsets.fill(Duration::ZERO);
    let mut recording = Recorder::<7>::new(&mut clock);
    for _ in 0..8 {
        recording.mark().unwrap();
    }
    assert_eq!(recording.finish().unwrap(), ([0; 7], 0));
}

#[test]
fn layer_duration_recorder_refuses_missing_extra_backward_and_overflow() {
    let mut clock = ScriptClock::new(8);
    let mut recording = Recorder::<6>::new(&mut clock);
    assert!(recording.finish().is_err());
    for _ in 0..7 {
        recording.mark().unwrap();
    }
    assert!(recording.mark().is_err());
    let mut clock = ScriptClock::new(2);
    clock.offsets = vec![Duration::from_nanos(1), Duration::ZERO];
    let mut recording = Recorder::<1>::new(&mut clock);
    recording.mark().unwrap();
    assert!(recording.mark().is_err());
    for duration in [
        Duration::from_nanos(LIMIT_NS + 1),
        Duration::from_secs(u64::MAX / 1_000_000_000 + 1),
    ] {
        let mut clock = ScriptClock::new(2);
        clock.offsets[1] = duration;
        let mut recording = Recorder::<1>::new(&mut clock);
        recording.mark().unwrap();
        assert!(recording.mark().is_err());
    }
}

#[test]
fn layer_duration_public_totals_refuse_overflow_bounds_and_pair_escape() {
    let valid = Gfx950EngineeringPeerScopedLayerDurationsV1 {
        phase_ns: [1, 2, 30, 4, 5, 6],
        layer_body_ns: 48,
        paired_mlp_phase_ns: [1; 7],
        paired_mlp_body_ns: 7,
    };
    valid.validate().unwrap();
    for value in [
        Gfx950EngineeringPeerScopedLayerDurationsV1 {
            layer_body_ns: 47,
            ..valid
        },
        Gfx950EngineeringPeerScopedLayerDurationsV1 {
            paired_mlp_body_ns: 8,
            ..valid
        },
        Gfx950EngineeringPeerScopedLayerDurationsV1 {
            paired_mlp_phase_ns: [5; 7],
            paired_mlp_body_ns: 35,
            ..valid
        },
        Gfx950EngineeringPeerScopedLayerDurationsV1 {
            phase_ns: [u64::MAX; 6],
            layer_body_ns: 0,
            ..valid
        },
        Gfx950EngineeringPeerScopedLayerDurationsV1 {
            phase_ns: [LIMIT_NS + 1, 0, 0, 0, 0, 0],
            layer_body_ns: LIMIT_NS + 1,
            paired_mlp_phase_ns: [0; 7],
            paired_mlp_body_ns: 0,
        },
    ] {
        assert!(value.validate().is_err());
    }
}

#[test]
fn layer_duration_recorder_refuses_total_hour_bound() {
    let mut clock = ScriptClock::new(3);
    clock.offsets = vec![
        Duration::ZERO,
        Duration::from_secs(1801),
        Duration::from_secs(3601),
    ];
    let mut recording = Recorder::<2>::new(&mut clock);
    for _ in 0..3 {
        recording.mark().unwrap();
    }
    assert!(recording.finish().is_err());
}
