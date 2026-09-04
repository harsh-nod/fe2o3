#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestErasedRosterV1 {
        markers: Vec<[u8; 32]>,
        is_current: bool,
    }

    impl ErasedAuthenticatedWorkerV3RosterV1 for TestErasedRosterV1 {
        fn entry_count(&self) -> usize {
            self.markers.len()
        }

        fn generated_host_contract(&self, _ordinal: usize) -> [u8; 32] {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn exact_current_hsaco_bytes(&self) -> &[u8] {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn descriptor(&self, _ordinal: usize) -> &KernelDescriptorV1 {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn physical_kernel(&self, _ordinal: usize) -> &InspectedKernel {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn descriptor_binding(&self, _ordinal: usize) -> KernelDescriptorBinding {
            unreachable!("program materialization is outside this custody-only unit test")
        }

        fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
            if self.is_current {
                Ok(())
            } else {
                Err(RecoveredWorkerV3AdmissionErrorV1::InspectionChanged)
            }
        }
    }

    fn target(name: &str) -> fe2o3_amd_target::AmdTargetId {
        fe2o3_amd_target::AmdTargetId::parse(name).unwrap()
    }

    #[test]
    fn seven_heterogeneous_rosters_can_supply_twelve_unique_programs() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let marker_groups = [
            vec![[1; 32], [2; 32]],
            vec![[3; 32]],
            vec![[4; 32], [5; 32]],
            vec![[6; 32], [7; 32]],
            vec![[8; 32], [9; 32]],
            vec![[10; 32]],
            vec![[11; 32], [12; 32]],
        ];
        let mut retained = Vec::<[u8; 32]>::new();
        for markers in &marker_groups {
            validate_program_append(Some(gfx942), &retained, gfx942, markers).unwrap();
            retained.extend(markers);
        }
        let programs = AuthenticatedWorkerV3ProgramSetV1 {
            rosters: marker_groups
                .into_iter()
                .map(|markers| {
                    Box::new(TestErasedRosterV1 {
                        markers,
                        is_current: true,
                    }) as Box<dyn ErasedAuthenticatedWorkerV3RosterV1>
                })
                .collect(),
            target: gfx942,
            marker_bindings: retained,
        };
        assert_eq!(programs.roster_count(), 7);
        assert_eq!(programs.program_count(), 12);
        assert_eq!(programs.target(), gfx942);
    }

    #[test]
    fn superseded_retired_owners_do_not_block_current_active_reuse() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let set = |marker, is_current| AuthenticatedWorkerV3ProgramSetV1 {
            rosters: vec![Box::new(TestErasedRosterV1 {
                markers: vec![[marker; 32]],
                is_current,
            })],
            target: gfx942,
            marker_bindings: vec![[marker; 32]],
        };
        let current = AuthenticatedProgramCustodyV1 {
            active: Some(set(2, true)),
            retired: vec![set(1, false)],
        };
        assert!(current.revalidate_active().is_ok());

        let superseded = AuthenticatedProgramCustodyV1 {
            active: Some(set(3, false)),
            retired: Vec::new(),
        };
        assert!(matches!(
            superseded.revalidate_active(),
            Err(
                AuthenticatedWorkerV3ProgramMaterializationErrorV1::CurrentPublication(
                    RecoveredWorkerV3AdmissionErrorV1::InspectionChanged
                )
            )
        ));
    }

    #[test]
    fn retained_program_selection_restores_exact_history_order() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let set = |marker| AuthenticatedWorkerV3ProgramSetV1 {
            rosters: vec![Box::new(TestErasedRosterV1 {
                markers: vec![[marker; 32]],
                is_current: true,
            })],
            target: gfx942,
            marker_bindings: vec![[marker; 32]],
        };
        let mut custody = AuthenticatedProgramCustodyV1 {
            active: Some(set(3)),
            retired: vec![set(1), set(2)],
        };

        custody.retire_active();
        let retained = custody.take_most_recent_retired();
        assert_eq!(retained.marker_bindings, vec![[3; 32]]);
        assert_eq!(custody.retired.len(), 2);

        custody.restore_most_recent_retired(retained);
        let restored = custody.into_program_sets();
        assert_eq!(restored.len(), 3);
        assert_eq!(restored[0].marker_bindings, vec![[1; 32]]);
        assert_eq!(restored[1].marker_bindings, vec![[2; 32]]);
        assert_eq!(restored[2].marker_bindings, vec![[3; 32]]);
    }

    #[test]
    fn release_success_returns_every_program_owner_in_history_order() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let set = |marker| AuthenticatedWorkerV3ProgramSetV1 {
            rosters: vec![Box::new(TestErasedRosterV1 {
                markers: vec![[marker; 32]],
                is_current: true,
            })],
            target: gfx942,
            marker_bindings: vec![[marker; 32]],
        };
        let custody = AuthenticatedProgramCustodyV1 {
            active: Some(set(3)),
            retired: vec![set(1), set(2)],
        };

        let (observation, programs) = route_release_custody::<_, ()>(Ok(17_u64), custody).unwrap();

        assert_eq!(observation, 17);
        assert_eq!(programs.len(), 3);
        assert_eq!(programs[0].marker_bindings, vec![[1; 32]]);
        assert_eq!(programs[1].marker_bindings, vec![[2; 32]]);
        assert_eq!(programs[2].marker_bindings, vec![[3; 32]]);
    }

    #[test]
    fn release_failure_retains_every_program_owner_with_the_exact_error() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let set = |marker| AuthenticatedWorkerV3ProgramSetV1 {
            rosters: vec![Box::new(TestErasedRosterV1 {
                markers: vec![[marker; 32]],
                is_current: true,
            })],
            target: gfx942,
            marker_bindings: vec![[marker; 32]],
        };
        let custody = AuthenticatedProgramCustodyV1 {
            active: Some(set(3)),
            retired: vec![set(1), set(2)],
        };

        let (error, custody) = route_release_custody::<(), _>(Err(23_u64), custody).unwrap_err();
        let programs = custody.into_program_sets();

        assert_eq!(error, 23);
        assert_eq!(programs.len(), 3);
        assert_eq!(programs[0].marker_bindings, vec![[1; 32]]);
        assert_eq!(programs[1].marker_bindings, vec![[2; 32]]);
        assert_eq!(programs[2].marker_bindings, vec![[3; 32]]);
    }

    #[test]
    fn retained_currentness_ignores_older_stale_history_but_rejects_stale_newest() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let set = |marker, is_current| AuthenticatedWorkerV3ProgramSetV1 {
            rosters: vec![Box::new(TestErasedRosterV1 {
                markers: vec![[marker; 32]],
                is_current,
            })],
            target: gfx942,
            marker_bindings: vec![[marker; 32]],
        };
        let mut current = AuthenticatedProgramCustodyV1 {
            active: Some(set(2, true)),
            retired: vec![set(1, false)],
        };
        current.retire_active();
        let retained = current.take_most_recent_retired();
        assert!(retained.revalidate_currentness().is_ok());
        current.restore_most_recent_retired(retained);

        let mut stale = AuthenticatedProgramCustodyV1 {
            active: Some(set(4, false)),
            retired: vec![set(3, true)],
        };
        stale.retire_active();
        let retained = stale.take_most_recent_retired();
        assert!(matches!(
            retained.revalidate_currentness(),
            Err(
                AuthenticatedWorkerV3ProgramMaterializationErrorV1::CurrentPublication(
                    RecoveredWorkerV3AdmissionErrorV1::InspectionChanged
                )
            )
        ));
        stale.restore_most_recent_retired(retained);
        assert_eq!(stale.into_program_sets().len(), 2);
    }

    #[test]
    fn heterogeneous_program_summary_rejects_duplicates_targets_and_native_overflow() {
        let gfx942 = target("gfx942:sramecc+:xnack-");
        let gfx950 = target("gfx950:sramecc+:xnack-");
        assert!(matches!(
            validate_program_append(Some(gfx942), &[[1; 32]], gfx942, &[[1; 32]]),
            Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::DuplicateKernelBinding)
        ));
        assert!(matches!(
            validate_program_append(Some(gfx942), &[[1; 32]], gfx950, &[[2; 32]]),
            Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::TargetMismatch)
        ));
        let existing = vec![[1; 32]; MAX_AUTHENTICATED_SERVICE_PROGRAMS_V1];
        assert!(matches!(
            validate_program_append(Some(gfx942), &existing, gfx942, &[[2; 32]]),
            Err(AuthenticatedWorkerV3ProgramSetAdmissionErrorV1::TooManyPrograms { .. })
        ));
    }
}
