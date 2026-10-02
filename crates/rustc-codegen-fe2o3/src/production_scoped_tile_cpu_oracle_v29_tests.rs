use super::*;

const INPUT: sim::BufferBackingIdV1 = sim::BufferBackingIdV1(7);
const OUTPUT: sim::BufferBackingIdV1 = sim::BufferBackingIdV1(11);
const CANARY: u32 = 0xdad0_0057;
const PREFIX: usize = 3;
const SUFFIX: usize = 2;

#[derive(Clone, Copy, Debug)]
struct Case {
    input: usize,
    output: usize,
    base: u64,
    groups: u64,
}

fn cases() -> [Case; 14] {
    [
        (0, 0, 0, 1),
        (1, 1, 0, 1),
        (63, 63, 0, 1),
        (64, 64, 0, 1),
        (65, 65, 0, 2),
        (65, 128, 1, 2),
        (65, 63, 3, 1),
        (65, 65, 65, 2),
        (65, 128, 66, 2),
        (65, 65, u64::MAX, 2),
        (1, 0, 0, 1),
        (0, 65, 0, 2),
        (65, 1, 0, 2),
        (65, 128, 64, 2),
    ]
    .map(|(input, output, base, groups)| Case {
        input,
        output,
        base,
        groups,
    })
}

fn input_value(index: usize) -> u32 {
    11 + 17 * u32::try_from(index).unwrap()
}

fn active_index(case: Case, global: u64) -> Option<usize> {
    let index = case.base.checked_add(global % 64)?;
    let index = usize::try_from(index).ok()?;
    (index < case.input).then_some(index)
}

fn backing(length: usize, input: bool, target: sim::SimulationTargetV1) -> sim::BufferArgumentV1 {
    let words: Vec<u32> = (0..PREFIX + length + SUFFIX)
        .map(|index| {
            if input && (PREFIX..PREFIX + length).contains(&index) {
                input_value(index - PREFIX)
            } else {
                CANARY
            }
        })
        .collect();
    let initialized = (0..words.len())
        .flat_map(|index| [input || index < PREFIX || index >= PREFIX + length; 4])
        .collect();
    sim::BufferArgumentV1::new(
        kir::ScalarType::U32,
        if input {
            kir::AccessMode::ReadOnly
        } else {
            kir::AccessMode::ReadWrite
        },
        4,
        words.into_iter().flat_map(u32::to_le_bytes).collect(),
        initialized,
        target,
    )
    .unwrap()
}

fn base_argument(base: u64, target: sim::SimulationTargetV1) -> sim::SimulationArgumentV1 {
    // The source usize parameter has a fixed u64 ABI on these compilation targets.
    sim::SimulationArgumentV1::Scalar(
        sim::ScalarBitsV1::new(kir::ScalarType::U64, u128::from(base), target).unwrap(),
    )
}

fn request(
    kernel: &kir::KernelId,
    case: Case,
    target: sim::SimulationTargetV1,
) -> sim::SimulationRequestV1 {
    let view = |id, length, access| {
        sim::SimulationArgumentV1::BufferView(
            sim::BufferViewArgumentV1::new(
                id,
                kir::ScalarType::U32,
                access,
                4,
                4 * PREFIX,
                length,
                target,
            )
            .unwrap(),
        )
    };
    sim::SimulationRequestV1::new(
        kernel.clone(),
        [64 * case.groups, 1, 1],
        [64, 1, 1],
        vec![
            view(INPUT, case.input, kir::AccessMode::ReadOnly),
            base_argument(case.base, target),
            view(OUTPUT, case.output, kir::AccessMode::ReadWrite),
        ],
    )
    .with_shared_buffers(vec![
        sim::SharedBufferV1 {
            id: INPUT,
            buffer: backing(case.input, true, target),
        },
        sim::SharedBufferV1 {
            id: OUTPUT,
            buffer: backing(case.output, false, target),
        },
    ])
}

fn check(execution: &sim::SimulationExecutionV1, request: &sim::SimulationRequestV1, case: Case) {
    assert!(!execution.grants_execution_authority());
    let input = execution.shared_buffer(INPUT).unwrap();
    assert_eq!(
        input, &request.shared_buffers[0].buffer,
        "source input remains unchanged"
    );
    let output = execution.shared_buffer(OUTPUT).unwrap();
    assert_eq!(output.bytes().len(), 4 * (PREFIX + case.output + SUFFIX));
    for (physical, word) in output.bytes().chunks_exact(4).enumerate() {
        let logical = physical
            .checked_sub(PREFIX)
            .filter(|&index| index < case.output);
        let stored = logical
            .filter(|&global| (global as u64) < 64 * case.groups)
            .and_then(|global| active_index(case, global as u64));
        let value = stored.map_or(CANARY, input_value);
        assert_eq!(word, value.to_le_bytes(), "{case:?} physical={physical}");
        let initialized = stored.is_some() || logical.is_none();
        assert!(
            output.initialized()[4 * physical..4 * physical + 4]
                .iter()
                .all(|&actual| actual == initialized),
            "{case:?} initialization at {physical}"
        );
    }
}

struct Events {
    case: Case,
    reads: [u8; 128],
    writes: [u8; 128],
}
impl sim::SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &sim::SimulationEventV1,
    ) -> Result<(), sim::SimulationEventSinkErrorV1> {
        let global = usize::try_from(event.invocation.global[0]).unwrap();
        match event.kind {
            sim::SimulationEventKindV1::MemoryRead { offset, bytes, .. } => {
                let index = active_index(self.case, global as u64)
                    .expect("inactive tile lane performed a memory read");
                assert_eq!((offset, bytes), (4 * (PREFIX + index), 4));
                self.reads[global] += 1;
            }
            sim::SimulationEventKindV1::MemoryWrite { offset, bytes, .. } => {
                assert!(active_index(self.case, global as u64).is_some());
                assert!(global < self.case.output);
                assert_eq!((offset, bytes), (4 * (PREFIX + global), 4));
                self.writes[global] += 1;
            }
            _ => {}
        }
        Ok(())
    }
}

fn mismatch(result: Result<sim::SimulationExecutionV1, sim::SimulationErrorV1>) {
    assert!(
        matches!(
            result,
            Err(sim::SimulationErrorV1::Execution(
                sim::SimulationExecutionErrorV1 {
                    kind: sim::SimulationExecutionErrorKindV1::ScheduleReplay(
                        sim::SimulationScheduleReplayErrorV1::ContextMismatch
                    ),
                    ..
                }
            ))
        ),
        "foreign target/request replay must refuse: {result:?}"
    );
}

pub(super) fn run(
    simulation: &sim::AdmittedSimulationModuleV1,
    limits: sim::SimulationLimitsV1,
) -> (usize, usize, usize, usize) {
    let mut totals = (0, 0, 0, 0);
    // These explicit CPU execution/replay contexts do not add target custody
    // to the target-neutral canonical owner or claim target-matched GPU output.
    for target in ["gfx942:xnack-", "gfx950:xnack-"]
        .map(|name| sim::SimulationTargetV1::amdgpu_from_device_target(name).unwrap())
    {
        for case in cases() {
            let request = request(&simulation.module().kernels[0].id, case, target);
            let original = request.clone();
            let mut wrong_type = request.clone();
            wrong_type.arguments[1] = sim::SimulationArgumentV1::Scalar(
                sim::ScalarBitsV1::index(case.base, target).unwrap(),
            );
            assert!(matches!(
                simulation.simulate(&wrong_type, target, limits),
                Err(sim::SimulationErrorV1::Preflight(
                    sim::SimulationPreflightErrorV1::ArgumentType {
                        argument: 1,
                        expected: kir::Type::Scalar(kir::ScalarType::U64),
                    }
                ))
            ));
            let mut events = Events {
                case,
                reads: [0; 128],
                writes: [0; 128],
            };
            let execution = simulation
                .simulate_observed_with_sink(&request, target, limits, &mut events)
                .expect("actual source scalar candidate CPU execution");
            check(&execution, &request, case);
            drop(execution);
            for global in 0..128 {
                let active = (global as u64) < 64 * case.groups
                    && active_index(case, global as u64).is_some();
                assert_eq!(
                    events.reads[global],
                    u8::from(active),
                    "{case:?} lane={global}"
                );
                assert_eq!(
                    events.writes[global],
                    u8::from(active && global < case.output)
                );
            }
            totals.2 += events
                .reads
                .iter()
                .map(|&count| usize::from(count))
                .sum::<usize>();
            totals.3 += events
                .writes
                .iter()
                .map(|&count| usize::from(count))
                .sum::<usize>();
            for schedule in [
                sim::SimulationScheduleRequestV1::RecordCanonical {
                    max_decisions: 65_536,
                },
                sim::SimulationScheduleRequestV1::RecordSeeded {
                    seed: 37,
                    max_decisions: 65_536,
                },
            ] {
                let execution = simulation
                    .simulate_scheduled(&request, target, limits, schedule)
                    .unwrap();
                check(&execution, &request, case);
                let record = execution.schedule_record().unwrap();
                let replay = simulation
                    .simulate_scheduled(
                        &request,
                        target,
                        limits,
                        sim::SimulationScheduleRequestV1::Replay(record),
                    )
                    .unwrap();
                check(&replay, &request, case);
                assert!(replay.schedule_record().is_none());
                assert_eq!(record.schedule(), replay.schedule());
                assert_eq!(record.coverage(), replay.schedule_coverage());
                assert_eq!(
                    record.transcript_identity(),
                    replay.schedule_transcript_identity()
                );
                assert_eq!(
                    execution.schedule_transcript_identity(),
                    replay.schedule_transcript_identity()
                );
                for other in [
                    sim::SimulationTargetV1::amdgpu_64(),
                    sim::SimulationTargetV1::amdgpu_from_device_target(
                        if target
                            == sim::SimulationTargetV1::amdgpu_from_device_target("gfx942:xnack-")
                                .unwrap()
                        {
                            "gfx950:xnack-"
                        } else {
                            "gfx942:xnack-"
                        },
                    )
                    .unwrap(),
                ] {
                    mismatch(simulation.simulate_scheduled(
                        &request,
                        other,
                        limits,
                        sim::SimulationScheduleRequestV1::Replay(record),
                    ));
                }
                let mut changed = request.clone();
                changed.arguments[1] = base_argument(case.base.wrapping_add(1), target);
                mismatch(simulation.simulate_scheduled(
                    &changed,
                    target,
                    limits,
                    sim::SimulationScheduleRequestV1::Replay(record),
                ));
                totals.1 += 1;
            }
            assert_eq!(request, original);
            totals.0 += 1;
        }
    }
    totals
}
