use super::*;

#[test]
fn handles_and_queue_epochs_must_both_match() {
    assert!(require_identity(1, 2, 1, 2).is_ok());
    for values in [(0, 2, 0, 2), (1, 2, 2, 2), (1, 2, 1, 3)] {
        assert!(require_identity(values.0, values.1, values.2, values.3).is_err());
    }
}

#[test]
fn frontier_requires_idle_exact_completion_and_whole_program_ring_budget() {
    assert!(require_frontier(64, 64, 64, 0, 1024).is_ok());
    for values in [
        (65, 64, 64, 0, 1),
        (64, 64, 63, 0, 1),
        (64, 64, 64, 65, 1),
        (64, 64, 64, 0, 0),
        (64, 64, 64, 0, 1025),
        (u64::MAX, u64::MAX, u64::MAX, 0, 1),
        (
            MAX_UNRETIRED_RING_PACKETS_V1,
            MAX_UNRETIRED_RING_PACKETS_V1,
            MAX_UNRETIRED_RING_PACKETS_V1,
            0,
            1,
        ),
    ] {
        assert!(require_frontier(values.0, values.1, values.2, values.3, values.4).is_err());
    }
}

#[test]
fn scalar_updates_cannot_touch_pointer_partial_or_implicit_fields() {
    let metadata = KernelMetadataV1 {
        symbol: "kernel".into(),
        object_sha256: [0; 32],
        kernarg_bytes: 272,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(16),
        implicit_argument_bytes: 256,
        explicit_arguments: vec![
            ExplicitArgumentV1 {
                offset: 0,
                bytes: 4,
                global_buffer: false,
                pointee_alignment: None,
                access: None,
            },
            ExplicitArgumentV1 {
                offset: 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(4),
                access: Some(BufferAccessV1::Read),
            },
        ],
    };
    assert!(require_scalar_field(&metadata, 0).is_ok());
    for offset in [1, 4, 8, 12, 16, 20, u32::MAX] {
        assert!(require_scalar_field(&metadata, offset).is_err());
    }
}

#[test]
fn groups_are_at_most_64_and_share_one_nonrenewing_deadline() {
    let deadline = Instant::now() + Duration::from_secs(60);
    for count in [1, 64, 65, 1024] {
        let mut groups = Vec::new();
        let retired =
            run_prepared_groups((0..count).collect(), deadline, |group, actual_deadline| {
                assert_eq!(actual_deadline, deadline);
                assert!((1..=64).contains(&group.len()));
                groups.extend(group);
                Ok(())
            })
            .unwrap();
        assert_eq!(retired, count);
        assert_eq!(groups, (0..count).collect::<Vec<_>>());
    }
}

#[test]
fn later_group_failure_never_reports_aggregate_success_or_runs_next_group() {
    let mut groups = 0;
    let result = run_prepared_groups(
        vec![(); 129],
        Instant::now() + Duration::from_secs(60),
        |_, _| {
            groups += 1;
            if groups == 2 {
                Err("uncertain second publication".into())
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(groups, 2);
}

#[test]
fn expired_deadline_cannot_publish_even_one_group() {
    let mut calls = 0;
    assert!(
        run_prepared_groups(vec![()], Instant::now(), |_, _| {
            calls += 1;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(calls, 0);
    assert!(
        run_prepared_groups::<()>(vec![], Instant::now() + Duration::from_secs(1), |_, _| Ok(
            ()
        ))
        .is_err()
    );
}

#[test]
fn deadline_expiry_after_first_retirement_prevents_second_group() {
    let deadline = Instant::now() + Duration::from_millis(5);
    let mut calls = 0;
    let result = run_prepared_groups(vec![(); 65], deadline, |_, actual| {
        calls += 1;
        assert_eq!(actual, deadline);
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
        Ok(())
    });
    assert!(result.is_err());
    assert!(calls <= 1);
}

#[test]
fn native_route_requires_default_off_explicit_mode_without_active_poll() {
    assert!(require_policy(true, Ordered64WaitPolicy::Sleep50usV1).is_ok());
    assert!(require_policy(false, Ordered64WaitPolicy::Sleep50usV1).is_err());
    assert!(require_policy(false, Ordered64WaitPolicy::ActivePoll10msV1).is_err());
    assert!(require_policy(true, Ordered64WaitPolicy::ActivePoll10msV1).is_err());
}

#[test]
fn invalid_binding_after_first_64_preparations_yields_zero_publications() {
    let metadata = KernelMetadataV1 {
        symbol: "kernel".into(),
        object_sha256: [0; 32],
        kernarg_bytes: 8,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: None,
        implicit_argument_bytes: 0,
        explicit_arguments: vec![ExplicitArgumentV1 {
            offset: 0,
            bytes: 8,
            global_buffer: true,
            pointee_alignment: Some(8),
            access: Some(BufferAccessV1::Read),
        }],
    };
    let command = OrderedBatchDispatchV1 {
        kernel: 1,
        payload_bytes: 8,
        workgroup: [64, 1, 1],
        grid: [64, 1, 1],
        pointers: vec![PointerFixupV1 {
            kernarg_offset: 0,
            buffer: 1,
            buffer_offset: 0,
            extent_bytes: 8,
            access: BufferAccessV1::Read,
        }],
    };
    for bad_index in [64, 128] {
        for bad_range in [false, true] {
            let mut commands = vec![command.clone(); 129];
            if bad_range {
                commands[bad_index].pointers[0].buffer_offset = 16;
            } else {
                commands[bad_index].pointers[0].buffer = 2;
            }
            let deadline = Instant::now() + Duration::from_secs(60);
            let mut preparations = 0;
            let mut publications = 0;
            let result = prepare_program_commands(
                commands,
                vec![0; 129 * 8],
                Some(deadline),
                |command, mut bytes| {
                    preparations += 1;
                    patch_pointer_arguments(&metadata, &mut bytes, &command.pointers, |id| {
                        (id == 1).then_some((0x1000, 16))
                    })?;
                    Ok(bytes)
                },
            )
            .and_then(|prepared| {
                run_prepared_groups(prepared, deadline, |_, _| {
                    publications += 1;
                    Ok(())
                })
            });
            assert!(result.is_err());
            assert_eq!(preparations, bad_index + 1);
            assert_eq!(publications, 0);
        }
    }
}

#[test]
fn worker_orchestration_orders_full_preparation_and_registration_lifecycle() {
    let compact = |source: &str| {
        source
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    let source = compact(include_str!("engineering_gfx950_token_program.rs"));
    let execute = source
        .split("pub(super)fnexecute_token_program(")
        .nth(1)
        .unwrap();
    assert!(
        execute.find(".materialize(&updates)").unwrap()
            < execute.find("self.prepare_token_dispatches(").unwrap()
    );
    assert!(
        execute.find("self.prepare_token_dispatches(").unwrap()
            < execute.find("run_prepared_groups(").unwrap()
    );
    assert!(
        execute.find("run_prepared_groups(").unwrap()
            < execute.find("ResponseV1::TokenProgramCompleted").unwrap()
    );
    assert!(execute.contains("ifresult.is_err(){self.ordered_batch_poisoned=true;}"));
    let register = source
        .split("pub(super)fnregister_token_program(")
        .nth(1)
        .unwrap()
        .split("pub(super)fnrelease_token_program(")
        .next()
        .unwrap();
    assert!(
        register
            .find("self.require_program_resource_mutation()?")
            .unwrap()
            < register.find("self.token_program=Some(").unwrap()
    );
    assert!(
        register.find("self.prepare_token_dispatches(").unwrap()
            < register.find("self.token_program=Some(").unwrap()
    );
    assert!(!register.contains("run_prepared_token_group"));
    let release = source
        .split("pub(super)fnrelease_token_program(")
        .nth(1)
        .unwrap()
        .split("pub(super)fnexecute_token_program(")
        .next()
        .unwrap();
    assert!(
        release.find("require_identity(").unwrap()
            < release.find("self.token_program=None").unwrap()
    );
    let worker = compact(include_str!("engineering_gfx950.rs"));
    for (operation, action) in [
        ("Allocate", "context.allocate("),
        ("Free", "context.free("),
        ("LoadKernel", "context.load("),
        ("RolloverQueue", "context.rollover_queue("),
    ] {
        let arm = worker
            .split(&format!("CommandV1::{operation}"))
            .nth(1)
            .unwrap()
            .split("CommandV1::")
            .next()
            .unwrap();
        assert!(
            arm.find("context.require_program_resource_mutation()?")
                .unwrap()
                < arm.find(action).unwrap()
        );
    }
}
