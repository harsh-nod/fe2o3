use super::*;

impl<'a> DirectionalSdmaOpsV1<'a> {
    pub(in crate::kfd_backend) fn submit_same_device(
        &mut self,
        pair: SameDeviceSdmaPairOwnerV1,
        requests: Box<[SameDeviceSdmaCopyRequestV1]>,
    ) -> Result<SameDeviceSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1>>
    {
        let (source_offset, destination_offset, copy_bytes) =
            match validate_same_device_window_requests_v1(&requests) {
                Ok(window) => window,
                Err(detail) => {
                    return Err(SdmaTransitionFailureV1::Retryable {
                        detail,
                        custody: pair,
                    });
                }
            };
        match (self, pair.source, pair.destination) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(source),
                DirectionalSdmaDeviceOwnerV1::Native(destination),
            ) => match queue.submit_same_device_persistent_sdma_window_v1(
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            ) {
                Ok(submission)
                    if submission.source_offset() == source_offset
                        && submission.destination_offset() == destination_offset
                        && submission.copy_bytes() == copy_bytes
                        && submission.packet_count() == requests.len() =>
                {
                    Ok(SameDeviceSdmaSubmissionOwnerV1::Native { submission })
                }
                Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "same-device SDMA published-window metadata changed unexpectedly"
                        .to_owned(),
                    custody: SdmaTerminalCustodyV1::NativeSameDevice(
                        NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                            SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                        ),
                    ),
                }),
                Err(failure) => {
                    let (error, custody) = failure.into_parts();
                    Err(match custody {
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                            source,
                            destination,
                        } => SdmaTransitionFailureV1::Retryable {
                            detail: error.to_string(),
                            custody: SameDeviceSdmaPairOwnerV1 {
                                source: DirectionalSdmaDeviceOwnerV1::Native(source),
                                destination: DirectionalSdmaDeviceOwnerV1::Native(destination),
                            },
                        },
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                            ),
                        },
                    })
                }
            },
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(source),
                DirectionalSdmaDeviceOwnerV1::Scripted(destination),
            ) => driver.submit_same_device(
                SameDeviceSdmaPairOwnerV1 {
                    source: DirectionalSdmaDeviceOwnerV1::Scripted(source),
                    destination: DirectionalSdmaDeviceOwnerV1::Scripted(destination),
                },
                requests,
            ),
            #[cfg(test)]
            (_, source, destination) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during publication".to_owned(),
                custody: scripted_mismatch_same_device_pair(
                    SameDeviceSdmaPairOwnerV1 {
                        source,
                        destination,
                    },
                    "submission",
                ),
            }),
        }
    }

    pub(in crate::kfd_backend) fn poll_same_device(
        &mut self,
        submission: SameDeviceSdmaSubmissionOwnerV1,
    ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), SameDeviceSdmaSubmissionOwnerV1::Native { submission }) => {
                let expected_source_request = submission.source_request();
                let expected_destination_request = submission.destination_request();
                let expected_descriptor = submission.descriptor();
                match queue.poll_same_device_persistent_sdma_window_v1(submission) {
                    Ok(poll) => Ok(match poll {
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission)
                            if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                        {
                            SameDeviceSdmaPollV1::Pending(SameDeviceSdmaSubmissionOwnerV1::Native {
                                submission,
                            })
                        }
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission) => {
                            return Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA pending-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            });
                        }
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Completed(completed) => {
                            SameDeviceSdmaPollV1::Completed(
                                SameDeviceSdmaCompletedOwnerV1::Native { completed },
                            )
                        }
                    }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                            {
                                SameDeviceSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: SameDeviceSdmaSubmissionOwnerV1::Native {
                                        submission,
                                    },
                                }
                            }
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA retryable-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            },
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                                ),
                            },
                        })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.poll_same_device(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during poll".to_owned(),
                custody: scripted_mismatch_same_device_submission(submission, "poll"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn wait_same_device(
        &mut self,
        submission: SameDeviceSdmaSubmissionOwnerV1,
        timeout: Duration,
    ) -> Result<SameDeviceSdmaWaitV1, SameDeviceSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), SameDeviceSdmaSubmissionOwnerV1::Native { submission }) => {
                let expected_source_request = submission.source_request();
                let expected_destination_request = submission.destination_request();
                let expected_descriptor = submission.descriptor();
                match queue.wait_same_device_persistent_sdma_window_for_v1(submission, timeout) {
                    Ok(completed) => Ok(SameDeviceSdmaWaitV1::Completed(
                        SameDeviceSdmaCompletedOwnerV1::Native { completed },
                    )),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        let is_timeout = is_exact_sdma_timeout_v1(&error);
                        match custody {
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                            {
                                let submission = SameDeviceSdmaSubmissionOwnerV1::Native {
                                    submission,
                                };
                                if is_timeout {
                                    Ok(SameDeviceSdmaWaitV1::Timeout(submission))
                                } else {
                                    Err(SameDeviceSdmaExecutionFailureV1::Retryable {
                                        detail: error.to_string(),
                                        submission,
                                    })
                                }
                            }
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA retryable-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            }),
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                                ),
                            }),
                        }
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.wait_same_device(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during wait".to_owned(),
                custody: scripted_mismatch_same_device_submission(submission, "wait"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn retire_same_device(
        &mut self,
        completed: SameDeviceSdmaCompletedOwnerV1,
    ) -> Result<SameDeviceSdmaPairOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1>>
    {
        match (self, completed) {
            (Self::Native(_), SameDeviceSdmaCompletedOwnerV1::Native { completed }) => {
                match completed.retire_settled_frontiers_v1() {
                    Ok(pair) => {
                        let (source, destination) = pair.into_parts();
                        Ok(SameDeviceSdmaPairOwnerV1 {
                            source: DirectionalSdmaDeviceOwnerV1::Native(source),
                            destination: DirectionalSdmaDeviceOwnerV1::Native(destination),
                        })
                    }
                    Err(failure) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "paired same-device frontier retirement failed".to_owned(),
                        custody: SdmaTerminalCustodyV1::NativeSameDevice(
                            NativeSameDeviceSdmaTerminalCustodyV1::Completed(
                                SameDeviceSdmaCompletedOwnerV1::Native {
                                    completed: failure.into_completed(),
                                },
                            ),
                        ),
                    }),
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaCompletedOwnerV1::Scripted(completed)) => {
                driver.retire_same_device(completed)
            }
            #[cfg(test)]
            (_, completed) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during retirement".to_owned(),
                custody: scripted_mismatch_same_device_completed(completed, "retirement"),
            }),
        }
    }
}
