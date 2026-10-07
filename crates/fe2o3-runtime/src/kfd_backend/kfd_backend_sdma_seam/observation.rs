use super::*;

impl<'a> DirectionalSdmaOpsV1<'a> {
    pub(in crate::kfd_backend) fn poll(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                    submission,
                    host_offset,
                    device_offset,
                },
            ) => {
                let expected_direction = submission.direction();
                let expected_copy_bytes = submission.copy_bytes();
                match queue.poll_directional_persistent_sdma_copy_v1(submission) {
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission))
                        if submission.direction() == expected_direction
                            && submission.copy_bytes() == expected_copy_bytes =>
                    {
                        Ok(DirectionalSdmaPollV1::Pending(
                            DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                submission,
                                host_offset,
                                device_offset,
                            },
                        ))
                    }
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission)) => {
                        Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: "directional SDMA pending-single metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                ),
                            ),
                        })
                    }
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Completed(completed)) => {
                        Ok(DirectionalSdmaPollV1::Completed(
                            DirectionalSdmaCompletedOwnerV1::NativeSingle {
                                completed,
                                host_offset,
                                device_offset,
                            },
                        ))
                    }
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                                if submission.direction() == expected_direction
                                    && submission.copy_bytes() == expected_copy_bytes =>
                            {
                                DirectionalSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                }
                            }
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                submission,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-single metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset,
                                            device_offset,
                                        },
                                    ),
                                ),
                            },
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                                        custody,
                                    ),
                                ),
                            },
                        })
                    }
                }
            }
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
            ) => {
                let expected_direction = submission.direction();
                let expected_host_offset = submission.host_offset();
                let expected_device_offset = submission.device_offset();
                let expected_copy_bytes = submission.copy_bytes();
                let expected_packet_count = submission.packet_count();
                match queue.poll_directional_persistent_sdma_window_v1(submission) {
                    Ok(poll) => Ok(match poll {
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission)
                            if submission.direction() == expected_direction
                                && submission.host_offset() == expected_host_offset
                                && submission.device_offset() == expected_device_offset
                                && submission.copy_bytes() == expected_copy_bytes
                                && submission.packet_count() == expected_packet_count =>
                        {
                            DirectionalSdmaPollV1::Pending(
                                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                            )
                        }
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission) => {
                            return Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA pending-window metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                            submission,
                                        },
                                    ),
                                ),
                            });
                        }
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Completed(completed) => {
                            DirectionalSdmaPollV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::NativeWindow { completed },
                            )
                        }
                    }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                            submission,
                        ) if submission.direction() == expected_direction
                            && submission.host_offset() == expected_host_offset
                            && submission.device_offset() == expected_device_offset
                            && submission.copy_bytes() == expected_copy_bytes
                            && submission.packet_count() == expected_packet_count => {
                            DirectionalSdmaExecutionFailureV1::Retryable {
                                detail: error.to_string(),
                                submission: DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                            }
                        }
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                            submission,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: "directional SDMA retryable-window metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                                ),
                            ),
                        },
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(custody),
                            ),
                        },
                    })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.poll(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during poll".to_owned(),
                custody: scripted_mismatch_submission(submission, "poll"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn wait(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
        timeout: Duration,
    ) -> Result<DirectionalSdmaWaitV1, DirectionalSdmaExecutionFailureV1> {
        self.wait_impl_v1(
            submission,
            timeout,
            #[cfg(feature = "hardware-diagnostic")]
            None,
        )
    }

    #[cfg(feature = "hardware-diagnostic")]
    pub(in crate::kfd_backend) fn wait_with_diagnostics_v1(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
        timeout: Duration,
        policy: fe2o3_kfd::Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
    ) -> (
        Result<DirectionalSdmaWaitV1, DirectionalSdmaExecutionFailureV1>,
        Option<fe2o3_kfd::Gfx942SdmaPersistentWaitDiagnosticsV1>,
    ) {
        let mut observed = None;
        let result = self.wait_impl_v1(submission, timeout, Some((policy, &mut observed)));
        if !matches!(result, Ok(DirectionalSdmaWaitV1::Completed(_))) {
            observed = None;
        }
        (result, observed)
    }

    pub(super) fn wait_impl_v1(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
        timeout: Duration,
        #[cfg(feature = "hardware-diagnostic")] diagnostic: Option<(
            fe2o3_kfd::Gfx942SdmaPersistentDiagnosticSleepCeilingV1,
            &mut Option<fe2o3_kfd::Gfx942SdmaPersistentWaitDiagnosticsV1>,
        )>,
    ) -> Result<DirectionalSdmaWaitV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                    submission,
                    host_offset,
                    device_offset,
                },
            ) => {
                let expected_direction = submission.direction();
                let expected_copy_bytes = submission.copy_bytes();
                match queue.wait_directional_persistent_sdma_copy_for_v1(submission, timeout) {
                    Ok(completed) => Ok(DirectionalSdmaWaitV1::Completed(
                        DirectionalSdmaCompletedOwnerV1::NativeSingle {
                            completed,
                            host_offset,
                            device_offset,
                        },
                    )),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        let is_timeout = is_exact_sdma_timeout_v1(&error);
                        match custody {
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                                if submission.direction() == expected_direction
                                    && submission.copy_bytes() == expected_copy_bytes =>
                            {
                                let submission = DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                    submission,
                                    host_offset,
                                    device_offset,
                                };
                                if is_timeout {
                                    Ok(DirectionalSdmaWaitV1::Timeout(submission))
                                } else {
                                    Err(DirectionalSdmaExecutionFailureV1::Retryable {
                                        detail: error.to_string(),
                                        submission,
                                    })
                                }
                            }
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                submission,
                            ) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-single metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset,
                                            device_offset,
                                        },
                                    ),
                                ),
                            }),
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                                        custody,
                                    ),
                                ),
                            }),
                        }
                    }
                }
            }
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
            ) => {
                let expected_direction = submission.direction();
                let expected_host_offset = submission.host_offset();
                let expected_device_offset = submission.device_offset();
                let expected_copy_bytes = submission.copy_bytes();
                let expected_packet_count = submission.packet_count();
                #[cfg(feature = "hardware-diagnostic")]
                let result = if let Some((policy, output)) = diagnostic {
                    queue
                        .wait_directional_persistent_sdma_window_profiled_for_v1(
                            submission, timeout, policy,
                        )
                        .map(|(completed, observed)| {
                            *output = Some(observed);
                            completed
                        })
                } else {
                    queue.wait_directional_persistent_sdma_window_for_v1(submission, timeout)
                };
                #[cfg(not(feature = "hardware-diagnostic"))]
                let result =
                    queue.wait_directional_persistent_sdma_window_for_v1(submission, timeout);
                match result {
                    Ok(completed) => Ok(DirectionalSdmaWaitV1::Completed(
                        DirectionalSdmaCompletedOwnerV1::NativeWindow { completed },
                    )),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        let is_timeout = is_exact_sdma_timeout_v1(&error);
                        match custody {
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.direction() == expected_direction
                                && submission.host_offset() == expected_host_offset
                                && submission.device_offset() == expected_device_offset
                                && submission.copy_bytes() == expected_copy_bytes
                                && submission.packet_count() == expected_packet_count => {
                                let submission = DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                    submission,
                                };
                                if is_timeout {
                                    Ok(DirectionalSdmaWaitV1::Timeout(submission))
                                } else {
                                    Err(DirectionalSdmaExecutionFailureV1::Retryable {
                                        detail: error.to_string(),
                                        submission,
                                    })
                                }
                            }
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-window metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                            submission,
                                        },
                                    ),
                                ),
                            }),
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(custody),
                                ),
                            }),
                        }
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => {
                let result = driver.wait(submission);
                #[cfg(feature = "hardware-diagnostic")]
                if matches!(result, Ok(DirectionalSdmaWaitV1::Completed(_)))
                    && let Some((_, output)) = diagnostic
                {
                    *output = driver.wait_diagnostics.pop_front();
                }
                result
            }
            #[cfg(test)]
            (_, submission) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during wait".to_owned(),
                custody: scripted_mismatch_submission(submission, "wait"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn retire(
        &mut self,
        completed: DirectionalSdmaCompletedOwnerV1,
    ) -> Result<
        DirectionalSdmaPairOwnerV1,
        SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1, SdmaOwnerDiagnosticV1>,
    > {
        match (self, completed) {
            (Self::Native(_), DirectionalSdmaCompletedOwnerV1::NativeSingle { completed, .. }) => {
                retire_native_directional_completed_v1(completed.into_parts())
            }
            (Self::Native(_), DirectionalSdmaCompletedOwnerV1::NativeWindow { completed }) => {
                retire_native_directional_completed_v1(completed.into_parts())
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaCompletedOwnerV1::Scripted(completed)) => {
                driver.retire(completed).map_err(|failure| match failure {
                    SdmaTransitionFailureV1::Retryable { detail, custody } => {
                        SdmaTransitionFailureV1::Retryable {
                            detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                            custody,
                        }
                    }
                    SdmaTransitionFailureV1::ProcessTeardown { detail, custody } => {
                        SdmaTransitionFailureV1::ProcessTeardown {
                            detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                            custody,
                        }
                    }
                })
            }
            #[cfg(test)]
            (_, completed) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Static(
                    "directional SDMA owner/driver mismatch during retirement",
                ),
                custody: scripted_mismatch_completed(completed, "retirement"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn promote_full_h2d_to_compute_ready(
        &mut self,
        completed: DirectionalSdmaCompletedOwnerV1,
        content: fe2o3_kfd::Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (PersistentComputeReadyOwnerV1, SdmaBufferOwnerV1),
        PersistentComputeReadyTransitionFailureV1,
    > {
        match (self, completed) {
            (
                Self::Native(queue),
                DirectionalSdmaCompletedOwnerV1::NativeSingle { completed, .. },
            ) => map_native_ready_promotion_v1(
                queue.promote_full_single_h2d_to_persistent_compute_ready_v1(completed, content),
            ),
            (Self::Native(queue), DirectionalSdmaCompletedOwnerV1::NativeWindow { completed }) => {
                map_native_ready_promotion_v1(
                    queue.promote_full_h2d_to_persistent_compute_ready_v1(completed, content),
                )
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaCompletedOwnerV1::Scripted(completed)) => {
                match driver.promote_full_h2d_to_compute_ready(completed, content) {
                    Ok((device, host)) => Ok((
                        PersistentComputeReadyOwnerV1::Scripted {
                            device,
                            authenticated_sha256: content.sha256(),
                        },
                        SdmaBufferOwnerV1::Scripted(host),
                    )),
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::Recovered(pair)) => {
                        Err(PersistentComputeReadyTransitionFailureV1::Recovered { pair })
                    }
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                        completed,
                        terminal_receiver,
                    }) => Err(PersistentComputeReadyTransitionFailureV1::ForeignQueue {
                        detail: "scripted persistent-compute ready promotion foreign queue"
                            .to_owned(),
                        terminal_receiver,
                        completed: DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                    }),
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                        completed,
                    )) => Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                        detail: "scripted persistent-compute ready promotion teardown".to_owned(),
                        custody: Some(SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                            ),
                        )),
                    }),
                }
            }
            #[cfg(test)]
            (_, completed) => Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during H2D promotion".to_owned(),
                custody: Some(scripted_mismatch_completed(completed, "H2D promotion")),
            }),
        }
    }
}
