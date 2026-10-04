use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Work,
    Storage,
    Allocation,
}

struct Sink {
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    live: usize,
    peak: usize,
    findings: Vec<TensorLayoutFindingV1>,
}

impl Sink {
    fn new(work_limit: usize, storage_limit: usize, floor: usize) -> Self {
        Self {
            work_limit,
            storage_limit,
            work: 0,
            live: floor,
            peak: floor,
            findings: Vec::new(),
        }
    }

    fn reserve(&mut self, cells: usize) -> Result<(), Failure> {
        let next = self.live.checked_add(cells).ok_or(Failure::Storage)?;
        if next > self.storage_limit {
            return Err(Failure::Storage);
        }
        self.live = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
}

impl TensorLayoutFindingSinkV1 for Sink {
    type Error = Failure;

    fn charge_work(&mut self, amount: usize) -> Result<(), Failure> {
        let next = self.work.checked_add(amount).ok_or(Failure::Work)?;
        if next > self.work_limit {
            return Err(Failure::Work);
        }
        self.work = next;
        Ok(())
    }

    fn allocate_coordinates(&mut self, count: usize) -> Result<Vec<[u64; 2]>, Failure> {
        self.charge_work(6)?;
        self.reserve(count * 2)?;
        let mut coordinates = Vec::new();
        if coordinates.try_reserve_exact(count).is_err() {
            self.live -= count * 2;
            return Err(Failure::Allocation);
        }
        Ok(coordinates)
    }

    fn release_coordinates(
        &mut self,
        coordinates: Vec<[u64; 2]>,
        count: usize,
    ) -> Result<(), Failure> {
        drop(coordinates);
        self.live = self.live.checked_sub(count * 2).ok_or(Failure::Storage)?;
        Ok(())
    }

    fn emit(&mut self, finding: TensorLayoutFindingV1) -> Result<(), Failure> {
        self.charge_work(1)?;
        self.reserve(1)?;
        if self.findings.try_reserve_exact(1).is_err() {
            self.live -= 1;
            return Err(Failure::Allocation);
        }
        self.findings.push(finding);
        Ok(())
    }
}

// Independent old-style census, with no use of the new sorting/run code.
fn oracle(
    fragment: &TensorFragmentLayoutV1,
    role: TensorOperandRoleV1,
    subgroup_width: u16,
) -> Vec<TensorLayoutFindingV1> {
    let mut coordinates = BTreeMap::<[u64; 2], usize>::new();
    for lane in 0..subgroup_width.min(64) {
        for component in 0..fragment.fragment_elements {
            let Some(coordinate) = fragment.logical_coordinate(lane, component) else {
                return vec![TensorLayoutFindingV1::MalformedSymbolicMap { role }];
            };
            if coordinate[0] >= u64::from(fragment.shape[0])
                || coordinate[1] >= u64::from(fragment.shape[1])
            {
                return vec![TensorLayoutFindingV1::CoordinateOutOfBounds { role }];
            }
            *coordinates.entry(coordinate).or_default() += 1;
        }
    }
    let expected = usize::from(fragment.shape[0]) * usize::from(fragment.shape[1]);
    let mut findings = Vec::new();
    match fragment.multiplicity {
        TensorMultiplicityV1::Unique => {
            if coordinates.values().any(|count| *count != 1) {
                findings.push(TensorLayoutFindingV1::DuplicateCoordinate { role });
            }
            if coordinates.len() != expected {
                findings.push(TensorLayoutFindingV1::IncompleteCoverage { role });
            }
        }
        TensorMultiplicityV1::Broadcast { factor } => {
            if factor == 0
                || coordinates.len() != expected
                || coordinates
                    .values()
                    .any(|count| *count != usize::from(factor))
            {
                findings.push(TensorLayoutFindingV1::BroadcastContractMismatch { role });
            }
        }
    }
    findings
}

#[test]
fn mutated_2048_sample_multiplicity_has_literal_logarithmic_bounds() {
    let expected = TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64().a;
    assert_eq!(expected.fragment_elements, 32);
    assert_eq!(expected.multiplicity, TensorMultiplicityV1::Unique);
    let mut fragment = expected;
    fragment.multiplicity = TensorMultiplicityV1::Broadcast { factor: 2 };
    let role = TensorOperandRoleV1::A;
    // Validation 64+3*2048; allocate/release 6; fill 3*2048;
    // sort 16*2048*11; census 4*2048+1; one finding 1.
    const WORK: usize = 381_000;
    const FLOOR: usize = 7;
    const PEAK: usize = 4_104; // floor + 2*2048 coordinate cells + one finding.
    let mut exact = Sink::new(WORK, PEAK, FLOOR);
    assert_eq!(
        try_verify_tensor_coordinates_with_sink_v1(&fragment, &expected, role, 64, &mut exact,),
        Ok(())
    );
    assert_eq!(exact.work, WORK);
    assert_eq!(exact.peak, PEAK);
    assert_eq!(exact.live, FLOOR + 1);
    assert_eq!(exact.findings, oracle(&fragment, role, 64));
    assert_eq!(
        exact.findings,
        [TensorLayoutFindingV1::BroadcastContractMismatch { role }]
    );

    for (work, storage, failure) in [
        (WORK - 1, PEAK, Failure::Work),
        (WORK, PEAK - 1, Failure::Storage),
    ] {
        let mut rejected = Sink::new(work, storage, FLOOR);
        assert_eq!(
            try_verify_tensor_coordinates_with_sink_v1(
                &fragment,
                &expected,
                role,
                64,
                &mut rejected,
            ),
            Err(failure)
        );
        assert_eq!(rejected.live, FLOOR);
        assert!(rejected.findings.is_empty());
    }
    let mut fast = Sink::new(0, FLOOR, FLOOR);
    assert_eq!(
        try_verify_tensor_coordinates_with_sink_v1(&expected, &expected, role, 64, &mut fast,),
        Ok(())
    );
    assert_eq!((fast.work, fast.live, fast.peak), (0, FLOOR, FLOOR));
}

#[test]
fn coordinate_findings_match_independent_census_and_error_precedence() {
    for contract in [
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
    ] {
        for (role, expected) in [
            (TensorOperandRoleV1::A, contract.a),
            (TensorOperandRoleV1::B, contract.b),
            (TensorOperandRoleV1::Accumulator, contract.accumulator),
        ] {
            for width in [0, 1, 16, 32, 63, 64, 65] {
                for multiplicity in [
                    TensorMultiplicityV1::Unique,
                    TensorMultiplicityV1::Broadcast { factor: 0 },
                    TensorMultiplicityV1::Broadcast { factor: 1 },
                    TensorMultiplicityV1::Broadcast { factor: 2 },
                    TensorMultiplicityV1::Broadcast { factor: 4 },
                ] {
                    for shape in [expected.shape, [1, 1], [0, 0], [1024, 1024]] {
                        let mut fragment = expected;
                        fragment.shape = shape;
                        fragment.multiplicity = multiplicity;
                        let mut sink = Sink::new(usize::MAX, usize::MAX, 0);
                        try_verify_tensor_coordinates_with_sink_v1(
                            &fragment, &expected, role, width, &mut sink,
                        )
                        .unwrap();
                        assert_eq!(sink.findings, oracle(&fragment, role, width));
                        assert_eq!(sink.live, sink.findings.len());
                    }
                }
            }
            let mut malformed = expected;
            malformed.mapping = TensorSymbolicMapV1::Opaque(17);
            malformed.shape = [0, 0];
            malformed.multiplicity = TensorMultiplicityV1::Broadcast { factor: 0 };
            let mut sink = Sink::new(usize::MAX, 1, 0);
            try_verify_tensor_coordinates_with_sink_v1(&malformed, &expected, role, 64, &mut sink)
                .unwrap();
            assert_eq!(
                sink.findings,
                [TensorLayoutFindingV1::MalformedSymbolicMap { role }]
            );
            assert_eq!((sink.live, sink.peak), (1, 1));
        }
    }
}

#[test]
fn bounded_sort_matches_oracle_and_denies_before_mutation() {
    for count in [0_usize, 1, 2, 3, 15, 16, 17, 2048] {
        let input: Vec<_> = (0..count)
            .rev()
            .map(|index| [(index % 7) as u64, index as u64])
            .collect();
        let mut expected = input.clone();
        expected.sort_unstable();
        let height = if count < 2 {
            0
        } else {
            usize::BITS as usize - (count - 1).leading_zeros() as usize
        };
        let work = 16 * count * height;
        let mut actual = input.clone();
        let mut sink = Sink::new(work, 0, 0);
        sort_coordinates_v1(&mut actual, &mut sink).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(sink.work, work);
        if work != 0 {
            let mut rejected = input.clone();
            let mut sink = Sink::new(work - 1, 0, 0);
            assert_eq!(
                sort_coordinates_v1(&mut rejected, &mut sink),
                Err(Failure::Work)
            );
            assert_eq!(rejected, input);
        }
    }
}
