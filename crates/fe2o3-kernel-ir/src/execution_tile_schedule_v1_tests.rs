use super::*;

#[test]
fn explicit_tile_schedule_matches_wide_integer_oracle() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        for lanes in [1, 2, 3, 32, 64, 128, 255, 256] {
            for elements in [1, 2, 3, 31, 64, 124, 125] {
                let schedule = ExecutionTileScheduleV1::new(layout, lanes, elements).unwrap();
                for lane in 0..lanes {
                    for element in 0..elements {
                        let offset = match layout {
                            ExecutionTileLayoutV1::Blocked => {
                                u128::from(lane) * u128::from(elements) + u128::from(element)
                            }
                            ExecutionTileLayoutV1::Striped => {
                                u128::from(element) * u128::from(lanes) + u128::from(lane)
                            }
                        };
                        assert_eq!(u128::from(schedule.offset(lane, element).unwrap()), offset);
                        for (base, length) in [
                            (0, 0),
                            (0, 1),
                            (7, 97),
                            (u64::MAX - 2, u64::MAX),
                            (u64::MAX, u64::MAX),
                        ] {
                            let index = u128::from(base) + offset;
                            let expected = (index < u128::from(length)).then_some(index as u64);
                            assert_eq!(
                                schedule.address(lane, element, base, length).unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn explicit_tile_schedule_inverse_boundaries_cover_every_admitted_geometry() {
    for lanes in 1..=crate::MAX_EXECUTION_LANES_V15 {
        for elements in 1..=crate::MAX_EXECUTION_ELEMENTS_V15 {
            for layout in [
                ExecutionTileLayoutV1::Blocked,
                ExecutionTileLayoutV1::Striped,
            ] {
                let schedule = ExecutionTileScheduleV1::new(layout, lanes, elements).unwrap();
                // Exercise an independent inverse at the boundaries of every geometry.
                for offset in [
                    0,
                    u64::from(lanes) * u64::from(elements) / 2,
                    u64::from(lanes) * u64::from(elements) - 1,
                ] {
                    let (lane, element) = match layout {
                        ExecutionTileLayoutV1::Blocked => {
                            (offset / u64::from(elements), offset % u64::from(elements))
                        }
                        ExecutionTileLayoutV1::Striped => {
                            (offset % u64::from(lanes), offset / u64::from(lanes))
                        }
                    };
                    assert_eq!(
                        schedule.offset(lane as u16, element as u16).unwrap(),
                        offset
                    );
                }
            }
        }
    }
}

#[test]
fn explicit_tile_schedule_rejects_geometry_and_coordinates_without_reading() {
    for (lanes, elements) in [(0, 1), (1, 0), (257, 1), (1, 126), (u16::MAX, u16::MAX)] {
        assert!(matches!(
            ExecutionTileScheduleV1::new(ExecutionTileLayoutV1::Blocked, lanes, elements),
            Err(ExecutionTileScheduleErrorV1::Geometry(_))
        ));
    }
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let schedule = ExecutionTileScheduleV1::new(layout, 64, 3).unwrap();
        for (lane, element) in [(64, 0), (0, 3), (u16::MAX, u16::MAX)] {
            assert_eq!(
                schedule.load_u32(lane, element, 0, u64::MAX, |_| panic!("invalid read")),
                Err(ExecutionTileScheduleErrorV1::Coordinate { lane, element })
            );
        }
        for base in [17, u64::MAX - 1, u64::MAX] {
            assert_eq!(
                schedule
                    .load_u32(63, 2, base, 17, |_| panic!("inactive read"))
                    .unwrap(),
                (0, false)
            );
        }
    }
}

#[test]
fn explicit_tile_schedule_reads_active_components_once_in_component_order() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let schedule = ExecutionTileScheduleV1::new(layout, 4, 3).unwrap();
        let mut reads = Vec::new();
        let components: Vec<_> = (0..3)
            .map(|element| {
                schedule
                    .load_u32(1, element, 2, 8, |index| {
                        reads.push(index);
                        (index as u32).wrapping_mul(17)
                    })
                    .unwrap()
            })
            .collect();
        match layout {
            ExecutionTileLayoutV1::Blocked => {
                assert_eq!(reads, [5, 6, 7]);
                assert_eq!(components, [(85, true), (102, true), (119, true)]);
            }
            ExecutionTileLayoutV1::Striped => {
                assert_eq!(reads, [3, 7]);
                assert_eq!(components, [(51, true), (119, true), (0, false)]);
            }
        }
    }
}

#[test]
fn explicit_tile_layout_is_observable_in_component_dependent_arithmetic() {
    let result = |layout| {
        let schedule = ExecutionTileScheduleV1::new(layout, 64, 3).unwrap();
        [3_u32, 5, 7]
            .into_iter()
            .enumerate()
            .map(|(element, scale)| {
                let (value, active) = schedule
                    .load_u32(1, element as u16, 0, 192, |index| index as u32)
                    .unwrap();
                assert!(active);
                value.wrapping_mul(scale)
            })
            .fold(0_u32, u32::wrapping_add)
    };
    assert_eq!(result(ExecutionTileLayoutV1::Blocked), 64);
    assert_eq!(result(ExecutionTileLayoutV1::Striped), 1231);
}
