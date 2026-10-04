//! Exact launch observation for the closed masked-shift fixture, not authority.
use super::*;
use fe2o3_kernel_descriptor::{BlockSizeV1, DimensionsV1, LaunchConstraintsV1};

fn check_rows<'a>(
    report: &ShiftObservation,
    rows: impl IntoIterator<Item = (&'a str, &'a str, [u8; 32], &'a LaunchConstraintsV1)>,
) -> Result<(), SourceFailure> {
    let reject = || {
        SourceFailure::new(
            SourceStage::NativeHandoff,
            "masked shift descriptor root/binding or exact required/max launch differs",
        )
    };
    simulation::masked_shift::unique_root_ordinals(
        report.roots.iter().map(|row| row.root.as_str()),
    )
    .map_err(|_| reject())?;
    let mut seen = [false; ROOTS.len()];
    for (logical, entry, binding, launch) in rows {
        let ordinal = simulation::masked_shift::root_ordinal(logical).map_err(|_| reject())?;
        if std::mem::replace(&mut seen[ordinal], true) {
            return Err(reject());
        }
        let actual = report
            .roots
            .iter()
            .find(|row| row.root == logical)
            .ok_or_else(reject)?;
        if entry != actual.root
            || binding != actual.source_binding
            || launch.rank() != 1
            || launch.block_size() != BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap())
            || launch.max_flat_workgroup_size() != 64
        {
            return Err(reject());
        }
    }
    if seen.iter().any(|seen| !seen) {
        return Err(reject());
    }
    Ok(())
}

pub(in super::super) fn check_descriptor(
    report: Option<&ShiftObservation>,
    descriptor: &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
) -> Result<(), SourceFailure> {
    let Some(report) = report else { return Ok(()) };
    check_rows(
        report,
        descriptor.table().kernels().iter().map(|kernel| {
            (
                kernel.logical_name().as_str(),
                kernel.entry_name().as_str(),
                *kernel.kernel_id().as_bytes(),
                kernel.launch(),
            )
        }),
    )
}

#[test]
fn masked_shift_descriptor_observation_requires_exact_roots_bindings_and_launch() {
    let report = ShiftObservation {
        batch: Batch::all()[0],
        source_digest: [1; 32],
        original_digest: [2; 32],
        output_digest: [3; 32],
        roots: ROOTS
            .into_iter()
            .enumerate()
            .map(|(ordinal, root)| RootObservation {
                root: root.into(),
                source_function: [4; 32],
                source_binding: [ordinal as u8 + 1; 32],
                original_entry: root.into(),
                original_operation_owner: root.into(),
                entry: root.into(),
                operation_owner: root.into(),
                native_symbol: root.into(),
            })
            .collect(),
    };
    let dimensions = DimensionsV1::new(64, 1, 1).unwrap();
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(dimensions),
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        64,
        0,
        0,
    )
    .unwrap();
    let bindings = [[1; 32], [2; 32]];
    let rows = [
        (ROOTS[1], ROOTS[1], bindings[1], &launch),
        (ROOTS[0], ROOTS[0], bindings[0], &launch),
    ];
    check_rows(&report, rows).unwrap();
    assert!(check_rows(&report, rows[..1].iter().copied()).is_err());
    assert!(check_rows(&report, [rows[0], rows[0]]).is_err());
    for (logical, entry, binding) in [
        ("foreign", ROOTS[1], bindings[1]),
        (ROOTS[1], "foreign", bindings[1]),
        (ROOTS[1], ROOTS[1], bindings[0]),
    ] {
        assert!(check_rows(&report, [(logical, entry, binding, &launch), rows[1]]).is_err());
    }
    for (rank, block, maximum) in [
        (1, BlockSizeV1::Any, 64),
        (1, BlockSizeV1::AtMost(dimensions), 64),
        (
            1,
            BlockSizeV1::Exact(DimensionsV1::new(32, 1, 1).unwrap()),
            64,
        ),
        (1, BlockSizeV1::Exact(dimensions), 128),
        (2, BlockSizeV1::Exact(dimensions), 64),
        (
            2,
            BlockSizeV1::Exact(DimensionsV1::new(32, 2, 1).unwrap()),
            64,
        ),
    ] {
        let changed = LaunchConstraintsV1::new(
            rank,
            block,
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            maximum,
            0,
            0,
        )
        .unwrap();
        assert!(
            check_rows(
                &report,
                [(rows[0].0, rows[0].1, rows[0].2, &changed), rows[1],]
            )
            .is_err()
        );
    }
}
