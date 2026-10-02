//! Route a Context-authorized capture without advancing any child's work.

use super::*;

type CaptureResult = Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>>;

impl KfdMultiDeviceRuntimeBackendV1 {
    fn capture_has_pending_custody_v1(&self) -> bool {
        self.compute_xgmi_children.iter().any(Option::is_some)
            || !self.cooperative_allocation_owners.is_empty()
            || !self.cooperative_dependency_retain_counts.is_empty()
            || !self.cooperative_stream_pending_counts.is_empty()
            || !self.deferred_compute_retains.is_empty()
            || self.children.iter().any(|child| {
                child.capture_has_pending_custody_v1()
                    || child.native_reconciliations.iter().any(Option::is_some)
            })
            || self
                .submissions
                .values()
                .any(|submission| match submission {
                    RoutedSubmissionV1::Native { route, .. } => self
                        .children
                        .get(route.child)
                        .is_none_or(|child| !child.exact_submission_quiescent_v1(route.local)),
                    RoutedSubmissionV1::DeferredCompute(root) => {
                        root.status == BackendPollV1::Pending
                            || root.route.is_some_and(|route| {
                                self.children.get(route.child).is_none_or(|child| {
                                    !child.exact_submission_quiescent_v1(route.local)
                                })
                            })
                    }
                    RoutedSubmissionV1::CooperativeCopy(copy) => {
                        !copy.is_quiescent()
                            || !copy.dependencies.is_empty()
                            || !copy.staging.is_empty()
                            || copy
                                .compute_xgmi
                                .as_ref()
                                .is_some_and(|root| !root.is_quiescent())
                            || copy.sdma_leaf.as_ref().is_some_and(|leaf| {
                                self.children
                                    .get(leaf.child())
                                    .is_none_or(|child| !leaf.is_quiescent(child))
                            })
                    }
                })
    }

    pub(in crate::kfd_backend) fn capture_coherent_host_range_impl_v1(
        &mut self,
        mut request: BackendHostCaptureV1<'_>,
    ) -> CaptureResult {
        self.capture_coherent_host_range_with_v1(
            request.device(),
            request.allocation(),
            request.byte_offset(),
            request.destination_mut(),
            KfdRuntimeBackendV1::capture_coherent_host_range_into_v1,
        )
    }

    fn capture_coherent_host_range_with_v1(
        &mut self,
        device: u64,
        allocation: u64,
        offset: u64,
        destination: &mut [u8],
        read: impl FnOnce(&mut KfdRuntimeBackendV1, u64, u64, u64, &mut [u8]) -> CaptureResult,
    ) -> CaptureResult {
        use RuntimeHostCaptureErrorV1 as Error;
        if self.terminal
            || self.children.iter().any(|child| {
                child.terminal
                    || child.terminal_memory.is_some()
                    || child.terminal_sdma_custody.is_some()
            })
        {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(Error::ContextTerminal));
        }
        let child = self
            .device_children
            .get(&device)
            .copied()
            .filter(|index| {
                self.children
                    .get(*index)
                    .is_some_and(|child| child.description.backend_device == device)
            })
            .ok_or(RuntimeBackendFailureV1::Rejected(Error::ForeignContext))?;
        let route = self
            .allocations
            .get(&allocation)
            .copied()
            .ok_or(RuntimeBackendFailureV1::Rejected(Error::UnknownAllocation))?;
        if route.child != child {
            return Err(RuntimeBackendFailureV1::Rejected(Error::ForeignContext));
        }
        if self.capture_has_pending_custody_v1() {
            return Err(RuntimeBackendFailureV1::Rejected(Error::Pending));
        }
        // Completed results/events may remain retained. Only execution custody
        // blocks capture; the leaf still authenticates live native host storage.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            read(
                &mut self.children[child],
                device,
                route.local,
                offset,
                destination,
            )
        }));
        match result {
            Ok(result) => {
                if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.terminal = true;
                } else if self.children[child].terminal {
                    self.terminal = true;
                    return Err(RuntimeBackendFailureV1::Terminal(Error::ContextTerminal));
                }
                result
            }
            Err(payload) => {
                self.terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kfd_backend::drain_capture::tests::counted;

    fn backend() -> KfdMultiDeviceRuntimeBackendV1 {
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right])
            .unwrap()
    }

    fn allocate(backend: &mut KfdMultiDeviceRuntimeBackendV1, device: u64) -> u64 {
        backend
            .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
            .unwrap()
    }

    #[test]
    fn multi_capture_translates_exact_allocation_and_preserves_device_without_heap() {
        let mut backend = backend();
        let first = allocate(&mut backend, 7);
        let second = allocate(&mut backend, 8);
        assert_eq!(
            backend.allocations[&first].local,
            backend.allocations[&second].local
        );
        assert_ne!(first, second);
        let local = backend.allocations[&second].local;
        let mut destination = [0xa5; 12];
        let (result, allocations) = counted(|| {
            backend.capture_coherent_host_range_with_v1(
                8,
                second,
                4,
                &mut destination[2..10],
                |child, device, allocation, offset, destination| {
                    assert_eq!(child.description.backend_device, 8);
                    assert_eq!((device, allocation, offset), (8, local, 4));
                    assert!(child.allocations.contains_key(&local));
                    destination.fill(0x5a);
                    Ok(())
                },
            )
        });
        assert!(result.is_ok());
        assert_eq!(allocations, 0);
        assert_eq!(
            destination,
            [
                0xa5, 0xa5, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0xa5, 0xa5
            ]
        );
        backend.release_allocation_v1(first).unwrap();
        backend.release_allocation_v1(second).unwrap();
    }

    #[test]
    fn multi_capture_rejects_global_pending_terminal_and_wrong_routes_before_reader() {
        use RuntimeHostCaptureErrorV1 as Error;
        for case in 0..11 {
            let mut backend = backend();
            let allocation = allocate(&mut backend, 7);
            let route = backend.allocations[&allocation];
            let mut device = 7;
            let mut selected = allocation;
            let expected = match case {
                0 => {
                    device = 8;
                    Error::ForeignContext
                }
                1 => {
                    device = 9;
                    Error::ForeignContext
                }
                2 => {
                    selected = u64::MAX;
                    Error::UnknownAllocation
                }
                3 => {
                    backend.children[1].compute_completion_reservations = 1;
                    Error::Pending
                }
                4 => {
                    backend.children[1].sdma_completion_reservations = 1;
                    Error::Pending
                }
                5 => {
                    backend.compute_xgmi_children[1] = Some(99);
                    Error::Pending
                }
                6 => {
                    backend
                        .cooperative_allocation_owners
                        .insert(route, vec![99]);
                    Error::Pending
                }
                7 => {
                    backend.cooperative_dependency_retain_counts.insert(99, 1);
                    Error::Pending
                }
                8 => {
                    backend.cooperative_stream_pending_counts.insert(99, 1);
                    Error::Pending
                }
                9 => {
                    backend.children[1].terminal = true;
                    Error::ContextTerminal
                }
                _ => {
                    backend.terminal = true;
                    Error::ContextTerminal
                }
            };
            let mut destination = [0xa5; 8];
            let (result, allocations) = counted(|| {
                backend.capture_coherent_host_range_with_v1(
                    device,
                    selected,
                    0,
                    &mut destination,
                    |_, _, _, _, _| panic!("preflight rejection reached reader"),
                )
            });
            assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(error) | RuntimeBackendFailureV1::Terminal(error)) if error == expected)
            );
            assert_eq!(destination, [0xa5; 8]);
            assert_eq!(allocations, 0);
            assert_eq!(backend.terminal, expected == Error::ContextTerminal);
            // Test-only injected indexes have no native owners.
            backend.terminal = false;
            backend.children[1].terminal = false;
            backend.children[1].compute_completion_reservations = 0;
            backend.children[1].sdma_completion_reservations = 0;
            backend.compute_xgmi_children[1] = None;
            backend.cooperative_allocation_owners.clear();
            backend.cooperative_dependency_retain_counts.clear();
            backend.cooperative_stream_pending_counts.clear();
            backend.release_allocation_v1(allocation).unwrap();
        }
    }

    #[test]
    fn multi_capture_allows_completed_native_results_and_retained_events() {
        let mut backend = backend();
        let allocation = allocate(&mut backend, 7);
        let stream = backend.create_stream_v1(8).unwrap();
        let stream_route = backend.streams[&stream];
        let child = &mut backend.children[1];
        let local = child.next_id().unwrap();
        child.submissions.insert(
            local,
            SubmissionRecordV1 {
                stream: stream_route.local,
                status: BackendPollV1::Succeeded,
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        let submission = backend.next_id().unwrap();
        backend.reserve_native_stream_submission_v1(stream).unwrap();
        backend.submissions.insert(
            submission,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 { child: 1, local },
                stream,
            },
        );
        backend.retain_native_stream_submission_v1(stream);
        let event = backend.record_event_v1(stream, submission).unwrap();
        let mut destination = [0; 8];
        assert!(
            backend
                .capture_coherent_host_range_with_v1(
                    7,
                    allocation,
                    0,
                    &mut destination,
                    |_, _, _, _, destination| {
                        destination.fill(0x5a);
                        Ok(())
                    },
                )
                .is_ok()
        );
        assert_eq!(destination, [0x5a; 8]);
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(submission).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
    }

    #[test]
    fn multi_capture_latches_reader_terminal_and_preserves_unwind_payload() {
        for unwind in [false, true] {
            let mut backend = backend();
            let allocation = allocate(&mut backend, 7);
            let mut destination = [0xa5; 8];
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                backend.capture_coherent_host_range_with_v1(
                    7,
                    allocation,
                    0,
                    &mut destination,
                    |_, _, _, _, destination| {
                        destination[0] = 0x5a;
                        if unwind {
                            std::panic::panic_any(0x4d55_4c54_u32);
                        }
                        Err(RuntimeBackendFailureV1::Terminal(
                            RuntimeHostCaptureErrorV1::NativeUncertain,
                        ))
                    },
                )
            }));
            if unwind {
                assert_eq!(
                    outcome.unwrap_err().downcast_ref::<u32>(),
                    Some(&0x4d55_4c54)
                );
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    Err(RuntimeBackendFailureV1::Terminal(
                        RuntimeHostCaptureErrorV1::NativeUncertain
                    ))
                ));
            }
            assert!(backend.terminal);
            assert_eq!(
                destination,
                [0x5a, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]
            );
            backend.terminal = false;
            backend.release_allocation_v1(allocation).unwrap();
        }
    }

    #[test]
    fn multi_capture_child_terminal_cannot_publish_reader_success() {
        let mut backend = backend();
        let allocation = allocate(&mut backend, 7);
        let mut destination = [0; 8];
        let result = backend.capture_coherent_host_range_with_v1(
            7,
            allocation,
            0,
            &mut destination,
            |child, _, _, _, _| {
                child.terminal = true;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeHostCaptureErrorV1::ContextTerminal
            ))
        ));
        assert!(backend.terminal);
        backend.terminal = false;
        backend.children[0].terminal = false;
        backend.release_allocation_v1(allocation).unwrap();
    }

    #[test]
    fn multi_capture_allows_completed_cooperative_result_before_release() {
        let mut backend = backend();
        let source = allocate(&mut backend, 7);
        let destination = allocate(&mut backend, 8);
        let stream = backend.create_stream_v1(8).unwrap();
        backend.write_allocation_v1(source, 0, &[0x5a; 32]).unwrap();
        let submission = backend
            .peer_copy_v1(
                stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 32,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 32,
                },
                &[],
            )
            .unwrap();
        for _ in 0..8 {
            backend.flush_stream_v1(stream).unwrap();
            if backend.poll_v1(submission).unwrap() == BackendPollV1::Succeeded {
                break;
            }
        }
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        let event = backend.record_event_v1(stream, submission).unwrap();
        let mut output = [0; 8];
        assert!(
            backend
                .capture_coherent_host_range_with_v1(
                    8,
                    destination,
                    0,
                    &mut output,
                    |_, _, _, _, output| {
                        output.fill(0x5a);
                        Ok(())
                    },
                )
                .is_ok()
        );
        assert_eq!(output, [0x5a; 8]);
        backend.release_event_v1(event).unwrap();
        backend.release_submission_v1(submission).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
    }
}
