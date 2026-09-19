use super::*;

fn compare(
    owner: &VerifiedCanonicalKernelIrModuleV15,
    candidate: &Module,
    floor: usize,
    work: usize,
    storage: usize,
    denied: bool,
) -> Probe<bool> {
    probe(floor, work, storage, denied, |budget| {
        owner.matches_module_with_budget_v15(candidate, budget)
    })
}

#[test]
fn exact_query_preserves_owner_and_compares_scope_tile_and_metadata() {
    for source in [scope_module(), tile_module(false), tile_module(true)] {
        let (owner, storage) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
            .result
            .unwrap();
        let pointer = owner.canonical_bytes().as_ptr();
        let identity = owner.identity().clone();
        let floor = STORAGE_PREFIX + storage.retained_storage();
        assert!(
            compare(&owner, &source, floor, LIMIT, LIMIT, false)
                .result
                .unwrap()
        );
        for name in ["scoped-othxr", "longer-different-candidate-name", ""] {
            let mut changed = source.clone();
            changed.id = crate::ModuleId::new(name);
            assert!(
                !compare(&owner, &changed, floor, LIMIT, LIMIT, false)
                    .result
                    .unwrap()
            );
        }
        let mut changed = source.clone();
        changed.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(65),
        };
        assert!(
            !compare(&owner, &changed, floor, LIMIT, LIMIT, false)
                .result
                .unwrap()
        );
        let mut changed = source.clone();
        operations(&mut changed)[1].kind = OperationKind::Execution(Execution::WorkgroupDerive {
            context: ValueId(999),
        });
        assert!(
            !compare(&owner, &changed, floor, LIMIT, LIMIT, false)
                .result
                .unwrap()
        );
        assert_eq!(owner.canonical_bytes().as_ptr(), pointer);
        assert_eq!(owner.identity(), &identity);
        assert_eq!(owner.module(), &source);
    }
}

#[test]
fn comparison_uses_only_encoder_scratch_with_exact_cumulative_limits() {
    for source in [scope_module(), tile_module(false), tile_module(true)] {
        let (owner, owner_storage) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
            .result
            .unwrap();
        let owner_floor = STORAGE_PREFIX + owner_storage.retained_storage();
        let (candidate, candidate_storage) = probe(owner_floor, LIMIT, LIMIT, false, |budget| {
            owner.copy_module_for_transformation_v15(budget)
        })
        .result
        .unwrap();
        let floor = owner_floor + candidate_storage.retained_storage();
        // The one-kernel role tree and largest SO3 payload, not another wire buffer.
        let scratch = (11 * std::mem::size_of::<&crate::FunctionId>()
            + 14 * std::mem::size_of::<usize>())
            * 3
            + 4 * std::mem::size_of::<&crate::FunctionId>()
            + 28;
        let measured = compare(&owner, &candidate, floor, LIMIT, LIMIT, false);
        assert!(measured.result.unwrap());
        assert_eq!(measured.peak, floor + scratch);
        assert!(
            compare(
                &owner,
                &candidate,
                floor,
                measured.work,
                measured.peak,
                false
            )
            .result
            .unwrap()
        );
        let short_work = compare(
            &owner,
            &candidate,
            floor,
            measured.work - 1,
            measured.peak,
            false,
        );
        assert!(matches!(
            short_work.result,
            Err(AdmissionError::Encode(
                crate::KernelIrEncodeError::WorkLimit(_)
            ))
        ));
        assert!(short_work.denied_work.is_some());
        let short_storage = compare(
            &owner,
            &candidate,
            floor,
            measured.work,
            measured.peak - 1,
            false,
        );
        assert!(matches!(
            short_storage.result,
            Err(AdmissionError::Resource(ResourceError::Storage(_)))
        ));
        assert_eq!(short_storage.denied_storage, Some(measured.peak));
        let inherited = compare(
            &owner,
            &candidate,
            floor,
            measured.work,
            measured.peak,
            true,
        );
        assert!(inherited.result.unwrap());
        assert_eq!(inherited.denied_work, Some(usize::MAX));
        assert_eq!(inherited.denied_storage, Some(usize::MAX));
    }
}

#[test]
fn comparison_does_not_hide_late_encoding_error_after_early_difference() {
    let source = scope_module();
    let (owner, storage) = admit_source(&source, STORAGE_PREFIX, LIMIT, LIMIT, false)
        .result
        .unwrap();
    let mut changed = source;
    changed.id = crate::ModuleId::new("different");
    changed.functions[0].role = crate::FunctionRole::DeviceFfiExport;
    let result = compare(
        &owner,
        &changed,
        STORAGE_PREFIX + storage.retained_storage(),
        LIMIT,
        LIMIT,
        false,
    );
    assert!(matches!(
        result.result,
        Err(AdmissionError::Encode(
            crate::KernelIrEncodeError::UnsupportedInVersion {
                version: 15,
                feature: "device-FFI export function roles",
            }
        ))
    ));
}
