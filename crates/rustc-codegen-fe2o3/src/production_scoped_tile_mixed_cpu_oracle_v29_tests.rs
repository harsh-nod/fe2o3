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

fn cases() -> [Case; 25] {
    [
        (0, 64, 0, 1),
        (1, 64, 0, 1),
        (2, 64, 0, 1),
        (3, 64, 0, 1),
        (63, 64, 0, 1),
        (64, 64, 0, 1),
        (65, 64, 0, 1),
        (127, 64, 0, 1),
        (128, 64, 0, 1),
        (129, 64, 0, 1),
        (191, 64, 0, 1),
        (192, 64, 0, 1),
        (193, 64, 0, 1),
        (193, 128, 1, 2),
        (193, 128, 64, 2),
        (193, 128, 192, 2),
        (193, 128, 193, 2),
        (193, 128, u64::MAX - 1, 2),
        (193, 128, u64::MAX, 2),
        (193, 63, 0, 2),
        (193, 1, 0, 2),
        (193, 0, 0, 2),
        (0, 128, 0, 2),
        (65, 65, 0, 2),
        (193, 128, 0, 2),
    ]
    .map(|(input, output, base, groups)| Case {
        input,
        output,
        base,
        groups,
    })
}

fn input_value(index: usize) -> u32 {
    match index % 5 {
        0 => u32::MAX,
        1 => u32::MAX - 17,
        2 => 0,
        3 => 0x8000_0001,
        _ => 11 + 17 * u32::try_from(index).unwrap(),
    }
}

fn active_index(case: Case, order: Order, global: u64, element: u64) -> Option<usize> {
    let lane = global % 64;
    let relative = match order {
        Order::Blocked => lane * 3 + element,
        Order::Striped => element * 64 + lane,
    };
    let index = usize::try_from(case.base.checked_add(relative)?).ok()?;
    (index < case.input).then_some(index)
}

fn expected(case: Case, order: Order, global: u64) -> Option<u32> {
    let mut any = false;
    let mut sum = 0_u32;
    for (element, (weight, bias)) in [(3_u32, 11_u32), (5, 13), (7, 17)].into_iter().enumerate() {
        if let Some(index) = active_index(case, order, global, element as u64) {
            any = true;
            sum = sum.wrapping_add(input_value(index).wrapping_mul(weight).wrapping_add(bias));
        }
    }
    any.then_some(sum)
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
    // The authentic source usize parameter is U64, not the simulator's Index type.
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

fn check(
    execution: &sim::SimulationExecutionV1,
    request: &sim::SimulationRequestV1,
    case: Case,
    order: Order,
) {
    assert!(!execution.grants_execution_authority());
    assert_eq!(
        execution.shared_buffer(INPUT).unwrap(),
        &request.shared_buffers[0].buffer,
        "all input bytes and initialization bits remain unchanged"
    );
    let output = execution.shared_buffer(OUTPUT).unwrap();
    assert_eq!(output.bytes().len(), 4 * (PREFIX + case.output + SUFFIX));
    for (physical, word) in output.bytes().chunks_exact(4).enumerate() {
        let logical = physical
            .checked_sub(PREFIX)
            .filter(|&index| index < case.output);
        let stored = logical
            .filter(|&global| (global as u64) < 64 * case.groups)
            .and_then(|global| expected(case, order, global as u64));
        assert_eq!(
            word,
            stored.unwrap_or(CANARY).to_le_bytes(),
            "{order:?} {case:?} physical={physical}"
        );
        let initialized = stored.is_some() || logical.is_none();
        assert!(
            output.initialized()[4 * physical..4 * physical + 4]
                .iter()
                .all(|&actual| actual == initialized),
            "{order:?} {case:?} initialization at {physical}"
        );
    }
}

struct Events {
    case: Case,
    order: Order,
    reads: [u8; 128],
    writes: [u8; 128],
    input_allocation: Option<u64>,
    output_allocation: Option<u64>,
}

impl Events {
    fn new(case: Case, order: Order) -> Self {
        Self {
            case,
            order,
            reads: [0; 128],
            writes: [0; 128],
            input_allocation: None,
            output_allocation: None,
        }
    }

    fn check(&self) {
        for global in 0..128 {
            let launched = (global as u64) < 64 * self.case.groups;
            let reads = (0..3)
                .filter(|&element| {
                    launched
                        && active_index(self.case, self.order, global as u64, element).is_some()
                })
                .count();
            assert_eq!(
                usize::from(self.reads[global]),
                reads,
                "{:?} {:?} lane={global}",
                self.order,
                self.case
            );
            assert_eq!(
                self.writes[global],
                u8::from(reads > 0 && global < self.case.output)
            );
        }
        if let (Some(input), Some(output)) = (self.input_allocation, self.output_allocation) {
            assert_ne!(
                input, output,
                "input and sink backings must remain distinct"
            );
        }
    }
}

impl sim::SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &sim::SimulationEventV1,
    ) -> Result<(), sim::SimulationEventSinkErrorV1> {
        let global = usize::try_from(event.invocation.global[0]).unwrap();
        match event.kind {
            sim::SimulationEventKindV1::MemoryRead {
                allocation,
                offset,
                bytes,
            } => {
                assert!((global as u64) < 64 * self.case.groups);
                let index = (0..3)
                    .filter_map(|element| {
                        active_index(self.case, self.order, global as u64, element)
                    })
                    .nth(usize::from(self.reads[global]))
                    .expect("extra or inactive tile element read");
                assert_eq!((offset, bytes), (4 * (PREFIX + index), 4));
                assert_eq!(self.writes[global], 0, "tile reads precede the SIMT sink");
                assert_eq!(*self.input_allocation.get_or_insert(allocation), allocation);
                self.reads[global] += 1;
            }
            sim::SimulationEventKindV1::MemoryWrite {
                allocation,
                offset,
                bytes,
            } => {
                assert!((global as u64) < 64 * self.case.groups);
                let reads = (0..3)
                    .filter(|&element| {
                        active_index(self.case, self.order, global as u64, element).is_some()
                    })
                    .count();
                assert!(reads > 0 && global < self.case.output);
                assert_eq!(usize::from(self.reads[global]), reads);
                assert_eq!(
                    self.writes[global], 0,
                    "exactly one sink write per active lane"
                );
                assert_eq!((offset, bytes), (4 * (PREFIX + global), 4));
                assert_eq!(
                    *self.output_allocation.get_or_insert(allocation),
                    allocation
                );
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
        "foreign profile or changed request replay must refuse: {result:?}"
    );
}

pub(super) fn run(
    simulation: &sim::AdmittedSimulationModuleV1,
    limits: sim::SimulationLimitsV1,
    order: Order,
) -> (usize, usize, usize, usize) {
    let mut totals = (0, 0, 0, 0);
    let targets = ["gfx942:xnack-", "gfx950:xnack-"]
        .map(|name| sim::SimulationTargetV1::amdgpu_from_device_target(name).unwrap());
    // Explicit CPU replay contexts provide no GPU target custody or launch authority.
    for (target_index, target) in targets.into_iter().enumerate() {
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
            let mut events = Events::new(case, order);
            let execution = simulation
                .simulate_observed_with_sink(&request, target, limits, &mut events)
                .expect("actual mixed source scalar candidate CPU execution");
            check(&execution, &request, case, order);
            drop(execution);
            events.check();
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
                    max_decisions: 262_144,
                },
                sim::SimulationScheduleRequestV1::RecordSeeded {
                    seed: 37,
                    max_decisions: 262_144,
                },
            ] {
                let execution = simulation
                    .simulate_scheduled(&request, target, limits, schedule)
                    .unwrap();
                check(&execution, &request, case, order);
                let record = execution.schedule_record().unwrap();
                let replay = simulation
                    .simulate_scheduled(
                        &request,
                        target,
                        limits,
                        sim::SimulationScheduleRequestV1::Replay(record),
                    )
                    .unwrap();
                check(&replay, &request, case, order);
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
                    targets[1 - target_index],
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
            assert_eq!(
                request, original,
                "simulation must not mutate caller-owned requests"
            );
            totals.0 += 1;
        }
    }
    totals
}
