use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Qualification {
    pub(crate) roots: usize,
    pub(crate) logical: usize,
    pub(crate) physical: usize,
    pub(crate) components: usize,
    pub(crate) later_root_refusals: usize,
    pub(crate) complete_first_roots_observed: usize,
    pub(crate) refused_consumers_entered: usize,
    pub(crate) resource_cuts: usize,
}

fn inspect(view: &mut Schedule<'_, '_, '_>, roots: &[TypedDescriptorRootV1]) -> R<Qualification> {
    assert!(!view.grants_authority());
    assert_eq!(view.counts()?, (2, 6, 12));
    let mut prior_argument = 0;
    let mut prior_component = 0;
    let mut physical = 0;
    for (index, captured) in roots.iter().enumerate() {
        let row = view.root(index)?;
        let (slots, bytes) = match captured.logical_name() {
            "aggregate_pair_struct" => (4, 40),
            "aggregate_nested" => (6, 48),
            other => panic!("foreign actual multi-root {other}"),
        };
        assert_eq!(
            (row.first_argument, row.end_argument),
            (prior_argument, prior_argument + 3)
        );
        assert_eq!(
            (row.first_component, row.end_component),
            (prior_component, prior_component + slots + 1)
        );
        assert_eq!(
            (
                row.physical_slots,
                row.explicit_bytes,
                row.segment_bytes,
                row.alignment
            ),
            (slots, bytes, bytes + 256, 8)
        );
        for at in row.first_argument..row.end_argument {
            assert_eq!(view.argument(at)?.root, index);
        }
        for at in row.first_component..row.end_component {
            let part = view.component(at)?;
            assert_eq!(part.root, index);
            assert!(part.slot < slots);
        }
        physical += slots;
        prior_argument = row.end_argument;
        prior_component = row.end_component;
    }
    assert_eq!((prior_argument, prior_component, physical), (6, 12, 10));
    let mut whole = [0usize; 2];
    let mut first_fields = [0usize; 2];
    view.visit_paths(|path| {
        assert!(path.root() < roots.len());
        assert_eq!(path.argument().root, path.root());
        if path.source_path().is_empty() {
            whole[path.root()] += 1;
        }
        if path.argument().ordinal == 0 && !path.source_path().is_empty() {
            first_fields[path.root()] += 1;
        }
        assert!(
            path.components()
                .iter()
                .all(|row| row.root == path.root() && row.argument == path.argument().ordinal)
        );
        assert!(!path.grants_authority());
        Ok(())
    })?;
    assert_eq!(whole, [3, 3]);
    for (index, root) in roots.iter().enumerate() {
        assert_eq!(
            first_fields[index],
            match root.logical_name() {
                "aggregate_pair_struct" => 2,
                "aggregate_nested" => 6,
                _ => unreachable!(),
            }
        );
    }
    Ok(Qualification {
        roots: 2,
        logical: 6,
        physical,
        components: 12,
        later_root_refusals: 0,
        complete_first_roots_observed: 0,
        refused_consumers_entered: 0,
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
        view.check_subject(owner, roots, profile)?;
        observed = Some(inspect(view, roots)?);
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    (result, observed, budget.work(), budget.peak_storage())
}

// This uses the real same-owner root visitor and Builder, not synthetic packing
// rows. It proves that the refusal occurs after one complete private root.
fn observe_partial_builder(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
    detail: &'static str,
) -> R<()> {
    let floor = budget.storage();
    budget.reserve_storage(headers::<()>()?)?;
    let mut rows = Builder {
        roots: vector(2, budget)?,
        arguments: vector(6, budget)?,
        components: vector(12, budget)?,
    };
    let mut visits = 0;
    let mut visit = |index,
                     root: &TypedDescriptorRootV1,
                     semantic: &AdmittedInertSemanticMirV1,
                     plan: &mut Plan<'_, '_, '_>| {
        rows.root(index, root, semantic, plan)?;
        visits += 1;
        Ok(())
    };
    let result = visit_checked(owner, roots, profile, budget, Some(&mut visit));
    drop(visit);
    assert!(
        matches!(result, Err(E::Mismatch(actual)) if actual == detail),
        "later root: {result:?}"
    );
    assert_eq!(visits, 1);
    assert_eq!(rows.roots.len(), 1);
    assert_eq!(rows.arguments.len(), 3);
    assert_eq!(rows.components.len(), rows.roots[0].physical_slots + 1);
    assert_eq!(rows.roots[0].end_component, rows.components.len());
    drop(rows);
    budget.release_storage(budget.storage() - floor)?;
    Ok(())
}

pub(crate) fn qualify(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Qualification> {
    let floor = budget.storage();
    assert_eq!(roots.len(), 2);
    let mut names: Vec<_> = roots
        .iter()
        .map(TypedDescriptorRootV1::logical_name)
        .collect();
    names.sort();
    assert_eq!(names, ["aggregate_nested", "aggregate_pair_struct"]);
    let (result, observed, work, storage) =
        measure(owner, roots, profile, floor, usize::MAX, usize::MAX);
    result?;
    let mut observed = observed.unwrap();
    let (exact, replay, used, peak) = measure(owner, roots, profile, floor, work, storage);
    exact?;
    assert_eq!(replay, Some(observed));
    assert_eq!((used, peak), (work, storage));
    for (wl, sl, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let (result, value, _, _) = measure(owner, roots, profile, floor, wl, sl);
        assert!(value.is_none());
        match (resource(&result.unwrap_err()), is_work) {
            (Some(Resource::Work(error)), true) => assert_eq!(error.limit(), wl),
            (Some(Resource::Storage(error)), false) => assert_eq!(error.limit(), sl),
            other => panic!("multi-root exact resource cut: {other:?}"),
        }
        observed.resource_cuts += 1;
    }
    for fault in 0..4 {
        let mut changed = roots.to_vec();
        let mut args = changed[1].arguments.as_slice().to_vec();
        let detail = match fault {
            0 => {
                let mut bytes = *args[0].semantic_layout_identity.as_bytes();
                bytes[0] ^= 1;
                args[0].semantic_layout_identity = SemanticLayoutIdentityV1::from_sha256(bytes);
                "exact fresh rustc source type/layout capture"
            }
            1 => {
                args[0].access = AccessMode::ReadOnly;
                "captured aggregate by-value source ownership"
            }
            2 => {
                args.pop();
                "complete logical source argument coverage"
            }
            3 => {
                changed[1].export_name.push('_');
                "captured original kernel binding/export"
            }
            _ => unreachable!(),
        };
        changed[1].arguments = TypedArgumentListV1::new(args).unwrap();
        observe_partial_builder(owner, &changed, profile, budget, detail)?;
        observed.complete_first_roots_observed += 1;
        let mut entered = false;
        let result = with_schedule(owner, &changed, profile, budget, |_| {
            entered = true;
            Ok(())
        });
        assert!(!entered);
        assert!(
            matches!(result, Err(E::Mismatch(actual)) if actual == detail),
            "packing late refusal: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
        observed.later_root_refusals += 1;
        observed.refused_consumers_entered += usize::from(entered);
        let mut recovered = None;
        with_schedule(owner, roots, profile, budget, |view| {
            recovered = Some(inspect(view, roots)?);
            Ok(())
        })?;
        assert!(recovered.is_some());
        assert_eq!(budget.storage(), floor);
    }
    Ok(observed)
}
