use super::*;
use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;
use crate::RuntimeResourceVectorV1;

#[derive(Default)]
pub(super) struct Profile {
    pub(super) entries: Vec<Entry>,
    pub(super) outcome: u8,
    pub(super) calls: usize,
}
impl fmt::Debug for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("qualification profile")
    }
}

#[test]
fn qualification_context_outcomes_preserve_settlement_and_quarantine() {
    for outcome in 0..7 {
        let root = Entry::qualification_root_v1();
        let a = Entry::qualification_entry_v1(&root, 10);
        let b = Entry::qualification_entry_v1(&root, 20);
        let backend = MockBackend {
            request_profile: Some(Profile {
                entries: vec![b, a.clone()],
                outcome,
                calls: 0,
            }),
            ..Default::default()
        };
        let mut context = RuntimeContextV1::open_with_version_journal_v1(backend, 8, 2).unwrap();
        let device = context.devices()[0].id();
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 4)
        }));
        assert_eq!(context.backend.request_profile.as_ref().unwrap().calls, 1);
        let usage = a.account().usage_v1();
        match outcome {
            0 => {
                assert!(matches!(
                    result.unwrap(),
                    Err(RuntimeErrorV1::Validation(
                        RuntimeValidationErrorV1::Unsupported
                    ))
                ));
                assert_eq!(context.backend.allocation_calls, 0);
                assert_eq!(usage.used, RuntimeResourceVectorV1::ZERO);
            }
            1 => {
                let id = result.unwrap().unwrap();
                assert_eq!(usage.retained_records, 1);
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                    32
                );
                context.release_allocation(id).unwrap();
            }
            2 | 3 => {
                let error = result.unwrap().unwrap_err();
                assert!(if outcome == 2 {
                    matches!(error, RuntimeErrorV1::BackendQuiescent(_))
                } else {
                    matches!(error, RuntimeErrorV1::BackendRejected(_))
                });
                assert_eq!(usage.used, RuntimeResourceVectorV1::ZERO);
            }
            4..=6 => {
                if outcome == 6 {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(usage.quarantined_records, 1);
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
                    32
                );
                assert!(
                    context
                        .allocate(device, RuntimeMemoryKindV1::HostVisible, 1, 1)
                        .is_err()
                );
                assert_eq!(context.backend.request_profile.as_ref().unwrap().calls, 1);
                assert!(!context.cleanup().is_complete());
            }
            _ => unreachable!(),
        }
        if outcome <= 3 {
            assert_eq!(
                context
                    .version_journal_usage_v1()
                    .unwrap()
                    .allocation_records,
                0
            );
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn qualification_open_returns_original_backend_on_alias_roster() {
    let root = Entry::qualification_root_v1();
    let a = Entry::qualification_entry_v1(&root, 10);
    let alias = Entry::qualification_v1(20, a.model(), a.account().clone());
    let retained = a.account().reserve_v1(23).unwrap().retain();
    let before = a.account().usage_v1();
    let backend = MockBackend {
        request_profile: Some(Profile {
            entries: vec![a.clone(), alias],
            ..Default::default()
        }),
        ..Default::default()
    };
    let failure = RuntimeContextV1::open_with_version_journal_v1(backend, 8, 2)
        .err()
        .unwrap();
    let (backend, _) = failure.into_parts();
    assert_eq!(backend.enumeration_calls, 1);
    assert_eq!(backend.allocation_calls, 0);
    assert_eq!(a.account().usage_v1(), before);
    retained.release_after_rejection().unwrap();
}
