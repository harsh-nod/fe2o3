use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};

const LIMIT: usize = 512 * 1024 * 1024;

fn run_classified_production_v40(
    work: usize,
    storage: usize,
    endianness: EndiannessV2,
    examine: impl FnOnce(&str, [usize; 6]),
) -> (Result<()>, usize, usize, usize) {
    super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        source_function::tests::with_slots(plan, out, |slots, out| {
            let relation = slots.correspondence(out)?;
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }; 2];
            let census = generate_refinement_v36(
                relation,
                &launches,
                FormalIndexWidth::Bits64,
                endianness,
                out,
            )?;
            examine(&out.text, census);
            Ok(())
        })
    })
}

#[test]
fn original_mir_production_installs_one_checked_registry_pair_for_every_root() {
    for endianness in [EndiannessV2::Little, EndiannessV2::Big] {
        let mut completed = false;
        let result = run_classified_production_v40(LIMIT, LIMIT, endianness, |text, census| {
            assert_eq!(census[0], 2);
            for function in [
                "open spec fn invocation_source_view_contracts_0_v39(",
                "open spec fn byte_target_view_contracts_1_v38(",
                "open spec fn invocation_source_target_tag_pair_0_1_v40(",
            ] {
                assert_eq!(text.matches(function).count(), 1, "{function}");
            }
            for root in 0..2 {
                let initial = text
                    .split(&format!(
                        "open spec fn invocation_paired_raw_initial_{root}_v36("
                    ))
                    .nth(1)
                    .unwrap()
                    .split("open spec fn")
                    .next()
                    .unwrap();
                let gate = format!(
                    "let admitted = invocation_paired_native_inputs_{root}_v38(arguments, external, execution)"
                );
                let installed = "let memory = if admitted { ByteMemoryV30 { view_contracts: byte_target_view_contracts_1_v38(invocation_runtime_little_endian_v36()), ..external } } else { external }";
                assert!(initial.find(&gate).unwrap() < initial.find(installed).unwrap());
                assert!(initial.contains("valid: admitted"));
                assert!(text.contains(&format!("open spec fn byte_block_step_{root}_v30(")));
            }
            let operations: Vec<_> = text
                .split("open spec fn byte_operation_")
                .skip(1)
                .map(|body| body.split("open spec fn").next().unwrap())
                .collect();
            assert_eq!(operations.len(), census[4]);
            for operation in operations {
                let gate = "|| !byte_target_view_contracts_match_1_v38(s.memory, little_endian)";
                let refused = "effect: MemoryOperationEffectV30::Refused";
                assert!(operation.find(gate).unwrap() < operation.find(refused).unwrap());
                assert!(operation.contains("let state = MemoryStateV30 { valid: false, ..s }"));
            }
            let controls: Vec<_> = text
                .split("open spec fn byte_control_")
                .skip(1)
                .map(|body| body.split("open spec fn").next().unwrap())
                .collect();
            assert!(!controls.is_empty());
            for control in controls {
                let gate = "|| !(byte_target_view_contracts_match_1_v38(done.memory, true) || byte_target_view_contracts_match_1_v38(done.memory, false))";
                let refused = "state: MemoryStateV30 { valid: false, ..done }";
                assert!(control.find(gate).unwrap() < control.find(refused).unwrap());
            }
            assert!(text.contains("invocation_view_registries_related_v40(source, target)"));
            assert!(text.contains("invocation_guard_pair_never_refreshes_stale_source_v40"));
            assert!(!text.contains("assume("));
            completed = true;
        });
        result.0.unwrap();
        assert!(completed);
    }
}

#[test]
fn original_mir_production_classified_context_keeps_exact_and_one_short_resources() {
    let execute = |work, storage| {
        run_classified_production_v40(work, storage, EndiannessV2::Little, |_, census| {
            assert_eq!(census[0], 2);
        })
    };
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (work_limit, storage_limit, work_failure) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let error = execute(work_limit, storage_limit).0.unwrap_err();
        let resource = match error {
            Error::Resource(resource)
            | Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                resource,
            )) => resource,
            other => panic!("expected exact classified-production resource refusal: {other:?}"),
        };
        match (work_failure, resource) {
            (true, Resource::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (work, work_limit));
            }
            (false, Resource::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (storage, storage_limit));
            }
            other => panic!("classified-production resource boundary: {other:?}"),
        }
    }
}
