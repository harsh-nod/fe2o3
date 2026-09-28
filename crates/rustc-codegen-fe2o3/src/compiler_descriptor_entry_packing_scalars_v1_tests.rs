//! Genuine captured scalar roots, including ABI arguments unused by the body.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

pub(crate) const SYMBOLS: [&str; 6] = [
    "entry_packing_empty",
    "entry_packing_mixed",
    "entry_packing_three_u32",
    "entry_packing_u16",
    "entry_packing_u32",
    "entry_packing_u8",
];

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Row {
    pub(crate) name: String,
    pub(crate) sizes: Vec<u16>,
    pub(crate) source_offsets: Vec<u64>,
    pub(crate) device_offsets: Vec<u32>,
    pub(crate) explicit_bytes: u32,
    pub(crate) hidden_start: u32,
    pub(crate) segment_bytes: u32,
    pub(crate) segment_alignment: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Qualification {
    pub(crate) rows: Vec<Row>,
    pub(crate) roots: usize,
    pub(crate) logical: usize,
    pub(crate) physical: usize,
    pub(crate) source_paths: usize,
    pub(crate) resource_cuts: usize,
}

fn expected(name: &str) -> (&'static [u16], &'static [u32], u32, u32) {
    match name {
        "entry_packing_empty" => (&[], &[], 0, 256),
        "entry_packing_u8" => (&[1], &[0], 1, 264),
        "entry_packing_u16" => (&[2], &[0], 2, 264),
        "entry_packing_u32" => (&[4], &[0], 4, 264),
        "entry_packing_three_u32" => (&[4, 4, 4], &[0, 4, 8], 12, 272),
        "entry_packing_mixed" => (&[1, 2, 4, 8], &[0, 2, 4, 8], 16, 272),
        other => panic!("foreign scalar fixture root {other}"),
    }
}

fn scalar(size: u16) -> ScalarTypeV1 {
    match size {
        1 => ScalarTypeV1::U8,
        2 => ScalarTypeV1::U16,
        4 => ScalarTypeV1::U32,
        8 => ScalarTypeV1::U64,
        _ => unreachable!(),
    }
}

fn inspect(
    view: &mut Schedule<'_, '_, '_>,
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
) -> R<Qualification> {
    view.check_subject(owner, roots, profile)?;
    assert!(!view.grants_authority());
    assert_eq!(view.counts()?, (6, 10, 10));
    let (semantic, endpoints) = match owner {
        OwnerRef::Direct(value) => {
            let source = value.source_semantic_kir();
            (
                source.semantic().semantic(),
                [
                    source.module(),
                    source.pre_ranked_executable().unwrap().module(),
                    value.bound().module(),
                    value.output().module(),
                ],
            )
        }
        OwnerRef::Erased(value) => (
            value.original_source().semantic_ssa().source_semantic(),
            [
                value.original_source().executable().module(),
                value.erased().module(),
                value.bound().module(),
                value.output().module(),
            ],
        ),
    };
    assert_eq!(semantic.roots().len(), 6);
    let mut rows = Vec::new();
    let mut total = 0;
    for (index, captured) in roots.iter().enumerate() {
        let (sizes, offsets, explicit, segment) = expected(captured.logical_name());
        let row = view.root(index)?;
        assert_eq!(
            (row.first_argument, row.end_argument),
            (total, total + sizes.len())
        );
        assert_eq!(
            (row.first_component, row.end_component),
            (total, total + sizes.len())
        );
        assert_eq!(
            (
                row.physical_slots,
                row.explicit_bytes,
                row.segment_bytes,
                row.alignment
            ),
            (sizes.len(), explicit, segment, 8)
        );
        assert_eq!(captured.arguments.len(), sizes.len());
        let selected = semantic
            .select_kernel_body_for_root_v1(semantic.roots()[index])
            .unwrap();
        let source = &semantic.functions()[selected.body().index() as usize];
        assert_eq!(source.abi().source_input_types().len(), sizes.len());
        let mut source_offsets = Vec::new();
        let mut device_offsets = Vec::new();
        for (ordinal, (&size, &offset)) in sizes.iter().zip(offsets).enumerate() {
            let argument = view.argument(total + ordinal)?;
            let part = view.component(total + ordinal)?;
            let capture = &captured.arguments.as_slice()[ordinal];
            let source_type =
                &semantic.types()[source.abi().source_input_types()[ordinal].index() as usize];
            assert_eq!(argument.root, index);
            assert_eq!(argument.ordinal, ordinal);
            assert_eq!(
                (argument.first_slot, argument.end_slot),
                (ordinal, ordinal + 1)
            );
            assert_eq!(
                (argument.first_component, argument.end_component),
                (total + ordinal, total + ordinal + 1)
            );
            assert_eq!(argument.source_type, capture.semantic_type_identity);
            assert_eq!(argument.source_type, source_type.identity());
            assert_eq!(argument.source_layout, capture.semantic_layout_identity);
            assert_eq!(
                (argument.source_size, argument.source_alignment),
                (u64::from(size), u32::from(size))
            );
            assert_eq!(source_type.layout().size_bytes(), Some(u64::from(size)));
            assert_eq!(capture.kind, DescriptorArgumentKindV1::Scalar(scalar(size)));
            assert_eq!(
                (part.root, part.argument, part.slot),
                (index, ordinal, ordinal)
            );
            assert_eq!(part.source_type, argument.source_type);
            assert_eq!(
                (part.source_offset, part.offset, part.size, part.alignment),
                (0, offset, size, size)
            );
            assert_eq!(part.kind, Kind::Scalar(scalar(size)));
            for endpoint in endpoints {
                let kernel = &endpoint.kernels[index];
                assert_eq!(kernel.id.as_str(), captured.entry_symbol());
                let function = endpoint.function(&kernel.entry).unwrap();
                assert_eq!(function.role, FunctionRole::KernelEntry);
                assert_eq!(function.signature.parameters.len(), sizes.len());
                assert_eq!(
                    function.body.as_ref().unwrap().parameters.len(),
                    sizes.len()
                );
                assert_eq!(
                    function.body.as_ref().unwrap().parameters[ordinal],
                    part.value
                );
            }
            source_offsets.push(part.source_offset);
            device_offsets.push(part.offset);
        }
        // Empty entry signatures are checked too, without relying on a leaf loop.
        for endpoint in endpoints {
            let kernel = &endpoint.kernels[index];
            assert_eq!(kernel.id.as_str(), captured.entry_symbol());
            let function = endpoint.function(&kernel.entry).unwrap();
            assert_eq!(function.signature.parameters.len(), sizes.len());
            assert_eq!(
                function.body.as_ref().unwrap().parameters.len(),
                sizes.len()
            );
        }
        rows.push(Row {
            name: captured.logical_name().into(),
            sizes: sizes.to_vec(),
            source_offsets,
            device_offsets,
            explicit_bytes: explicit,
            hidden_start: row.segment_bytes.checked_sub(256).unwrap(),
            segment_bytes: row.segment_bytes,
            segment_alignment: row.alignment,
        });
        total += sizes.len();
    }
    assert_eq!(total, 10);
    let mut source_paths = 0;
    view.visit_paths(|path| {
        let argument = path.argument();
        assert!(path.source_path().is_empty());
        assert!(!path.grants_authority());
        assert_eq!(path.source_type(), argument.source_type);
        assert_eq!(path.source_layout(), argument.source_layout);
        assert_eq!(path.components().len(), 1);
        assert_eq!(path.components()[0].source_offset, 0);
        assert_eq!(
            path.coverage(),
            paths::Coverage::Slots {
                first: argument.first_slot,
                end: argument.end_slot
            }
        );
        source_paths += 1;
        Ok(())
    })?;
    assert_eq!(source_paths, 10);
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        SYMBOLS
    );
    Ok(Qualification {
        rows,
        roots: 6,
        logical: 10,
        physical: 10,
        source_paths,
        resource_cuts: 0,
    })
}

fn measure(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, Option<Qualification>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let mut observed = None;
    let result = with_schedule(owner, roots, profile, &mut budget, |view| {
        observed = Some(inspect(view, owner, roots, profile)?);
        Ok(())
    });
    assert_eq!(budget.storage(), floor, "{result:?}");
    (result, observed, budget.work(), budget.peak_storage())
}

pub(crate) fn qualify(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Qualification> {
    let floor = budget.storage();
    let (result, observed, work, storage) =
        measure(owner, roots, profile, floor, usize::MAX, usize::MAX);
    result?;
    let mut observed = observed.unwrap();
    let (result, replay, used, peak) = measure(owner, roots, profile, floor, work, storage);
    result?;
    assert_eq!(replay.as_ref(), Some(&observed));
    assert_eq!((used, peak), (work, storage));
    for (wl, sl, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let (result, value, _, _) = measure(owner, roots, profile, floor, wl, sl);
        assert!(value.is_none());
        match (resource(&result.unwrap_err()), is_work) {
            (Some(Resource::Work(error)), true) => assert_eq!(error.limit(), wl),
            (Some(Resource::Storage(error)), false) => assert_eq!(error.limit(), sl),
            other => panic!("scalar packing exact resource cut: {other:?}"),
        }
        observed.resource_cuts += 1;
    }
    with_schedule(owner, roots, profile, budget, |view| {
        let mut replay = inspect(view, owner, roots, profile)?;
        replay.resource_cuts = observed.resource_cuts;
        assert_eq!(replay, observed);
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    Ok(observed)
}
