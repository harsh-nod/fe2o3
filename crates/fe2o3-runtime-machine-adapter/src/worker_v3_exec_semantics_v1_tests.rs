use super::*;

const ARTIFACT: &[u8] = &[10, 11, 12, 13, 14, 15, 12, 13];

fn subject() -> MachineSubject<'static> {
    MachineSubject {
        payload: ARTIFACT,
        symbol: "entry",
        offset: 2,
        size: 2,
    }
}

#[test]
fn exact_artifact_and_selected_entry_match() {
    assert_eq!(check_exact_subject(subject(), subject(), &[12, 13]), Ok(()));
}

#[test]
fn same_isa_in_another_artifact_does_not_match() {
    for changed in [0, 7] {
        let mut payload = ARTIFACT.to_vec();
        payload[changed] ^= 1;
        let actual = MachineSubject {
            payload: &payload,
            ..subject()
        };
        assert_eq!(
            check_exact_subject(subject(), actual, &[12, 13]),
            Err(WorkerV3ExecSemanticsErrorV1::Artifact),
        );
    }
}

#[test]
fn equal_isa_at_a_different_offset_does_not_match() {
    let actual = MachineSubject {
        offset: 6,
        ..subject()
    };
    assert_eq!(
        check_exact_subject(subject(), actual, &[12, 13]),
        Err(WorkerV3ExecSemanticsErrorV1::Range),
    );
}

#[test]
fn wrong_entry_size_and_selected_isa_are_distinct_refusals() {
    let actual = MachineSubject {
        symbol: "another",
        ..subject()
    };
    assert_eq!(
        check_exact_subject(subject(), actual, &[12, 13]),
        Err(WorkerV3ExecSemanticsErrorV1::Entry),
    );
    let actual = MachineSubject {
        size: 1,
        ..subject()
    };
    assert_eq!(
        check_exact_subject(subject(), actual, &[12, 13]),
        Err(WorkerV3ExecSemanticsErrorV1::Range),
    );
    for isa in [&[][..], &[12][..], &[12, 14][..], &[12, 13, 14][..]] {
        assert_eq!(
            check_exact_subject(subject(), subject(), isa),
            Err(WorkerV3ExecSemanticsErrorV1::Isa),
        );
    }
}

#[test]
fn empty_out_of_bounds_and_overflowing_ranges_reject() {
    for (offset, size) in [(2, 0), (9, 1), (7, 2), (u64::MAX, 2), (1, u64::MAX)] {
        let make = || MachineSubject {
            offset,
            size,
            ..subject()
        };
        assert_eq!(
            check_exact_subject(make(), make(), &[]),
            Err(WorkerV3ExecSemanticsErrorV1::Range),
        );
    }
}

#[test]
fn control_slice_must_remain_in_the_selected_entry() {
    let slice = Gfx942ExecSliceV1::new("entry", 8, 12).unwrap();
    assert_eq!(check_selected_slice("entry", 4, 12, slice), Ok(()));
    for (symbol, offset, size) in [("another", 4, 12), ("entry", 12, 12), ("entry", 4, 8)] {
        assert_eq!(
            check_selected_slice(symbol, offset, size, slice),
            Err(WorkerV3ExecSemanticsErrorV1::Slice),
        );
    }
    assert_eq!(
        check_selected_slice("entry", u64::MAX, 4, slice),
        Err(WorkerV3ExecSemanticsErrorV1::Range),
    );
}
