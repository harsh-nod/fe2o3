use super::*;

impl<'a> DirectionalSdmaOpsV1<'a> {
    pub(in crate::kfd_backend) fn allocate_host(
        &mut self,
        byte_len: usize,
    ) -> Result<SdmaBufferOwnerV1, SdmaAllocationFailureV1> {
        match self {
            Self::Native(queue) => queue
                .allocate_sdma_pooled_host_buffer_classified_v1(byte_len)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(SdmaAllocationFailureV1::from),
            #[cfg(test)]
            Self::Scripted(driver) => driver.allocate_buffer(byte_len, ScriptedBufferKindV1::Host),
        }
    }

    pub(in crate::kfd_backend) fn allocate_device_buffer(
        &mut self,
        byte_len: u64,
        alignment: u64,
    ) -> Result<SdmaBufferOwnerV1, SdmaAllocationFailureV1> {
        self.allocate_device_buffer_with_visibility_v1(byte_len, alignment, false)
    }

    pub(in crate::kfd_backend) fn allocate_public_device_buffer(
        &mut self,
        byte_len: u64,
        alignment: u64,
    ) -> Result<SdmaBufferOwnerV1, SdmaAllocationFailureV1> {
        self.allocate_device_buffer_with_visibility_v1(byte_len, alignment, true)
    }

    pub(super) fn allocate_device_buffer_with_visibility_v1(
        &mut self,
        byte_len: u64,
        alignment: u64,
        peer_visible: bool,
    ) -> Result<SdmaBufferOwnerV1, SdmaAllocationFailureV1> {
        match self {
            Self::Native(queue) if peer_visible => queue
                .allocate_sdma_pooled_public_device_buffer_classified_v1(byte_len, alignment)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(SdmaAllocationFailureV1::from),
            Self::Native(queue) => queue
                .allocate_sdma_pooled_device_buffer_classified_v1(byte_len, alignment)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(SdmaAllocationFailureV1::from),
            #[cfg(test)]
            Self::Scripted(driver) => {
                let len = usize::try_from(byte_len).map_err(|_| {
                    SdmaAllocationFailureV1::scripted_protocol(
                        "scripted device allocation length overflow".to_owned(),
                    )
                })?;
                driver.allocate_buffer(
                    len,
                    if peer_visible {
                        ScriptedBufferKindV1::PublicDevice
                    } else {
                        ScriptedBufferKindV1::Device
                    },
                )
            }
        }
    }

    pub(in crate::kfd_backend) fn write_host(
        &mut self,
        buffer: &mut SdmaBufferOwnerV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .write_sdma_host_buffer(buffer, offset, bytes)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.write_host(buffer, offset, bytes)
            }
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while writing {:?}",
                buffer
            )),
        }
    }

    pub(in crate::kfd_backend) fn write_full_host_authenticated(
        &mut self,
        buffer: &mut SdmaBufferOwnerV1,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .write_full_sdma_host_buffer_authenticated_v1(buffer, bytes)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver
                .write_full_host_authenticated(buffer, bytes)
                .map(Some),
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while writing authenticated content to {:?}",
                buffer
            )),
        }
    }

    pub(in crate::kfd_backend) fn read_host_into(
        &mut self,
        buffer: &SdmaBufferOwnerV1,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .read_sdma_host_buffer_into(buffer, offset, destination)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.read_host_into(buffer, offset, destination)
            }
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while reading {:?}",
                buffer
            )),
        }
    }

    pub(in crate::kfd_backend) fn read_host_into_v1(
        &mut self,
        buffer: &SdmaBufferOwnerV1,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), fe2o3_kfd::Gfx942SdmaHostReadIntoErrorV1> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => {
                queue.read_sdma_host_buffer_into_v1(buffer, offset, destination)
            }
            #[cfg(test)]
            _ => Err(fe2o3_kfd::Gfx942SdmaHostReadIntoErrorV1::InvalidBuffer),
        }
    }

    pub(in crate::kfd_backend) fn promote(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<
        DirectionalSdmaDeviceOwnerV1,
        SdmaTransitionFailureV1<SdmaBufferOwnerV1, SdmaOwnerDiagnosticV1>,
    > {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .promote_sdma_device_buffer_to_directional_persistent_allocation_v1(buffer)
                .map(DirectionalSdmaDeviceOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(buffer) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: SdmaOwnerDiagnosticV1::Native(error),
                                custody: SdmaBufferOwnerV1::Native(buffer),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Promotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.promote(buffer).map_err(|failure| match failure {
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
            (_, buffer) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Scripted(
                    "directional SDMA owner/driver mismatch during promotion".to_owned(),
                ),
                custody: scripted_mismatch_buffer(buffer, "promotion"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn promote_initialized_storage(
        &mut self,
        device: DirectionalSdmaDeviceOwnerV1,
    ) -> Result<
        InitializedStorageAttemptV1,
        SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1, SdmaOwnerDiagnosticV1>,
    > {
        match (self, device) {
            (Self::Native(queue), DirectionalSdmaDeviceOwnerV1::Native(device)) => {
                queue
                    .try_promote_initialized_persistent_allocation_for_compute_v1(device)
                    .map(|attempt| match attempt {
                        Gfx942PersistentComputeStorageAttemptV1::Promoted(ready) => {
                            InitializedStorageAttemptV1::Promoted(
                                InitializedStorageOwnerV1::Native(ready),
                            )
                        }
                        Gfx942PersistentComputeStorageAttemptV1::NotEligible {
                            allocation,
                            reason,
                        } => InitializedStorageAttemptV1::NotEligible {
                            device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                            reason,
                        },
                    })
                    .map_err(|failure| {
                        let (error, custody) = failure.into_parts();
                        let detail = SdmaOwnerDiagnosticV1::Native(error);
                        match custody {
                            Gfx942PersistentComputeStoragePromotionCustodyV1::Retryable(device)
                            | Gfx942PersistentComputeStoragePromotionCustodyV1::ForeignQueue(
                                device,
                            ) => SdmaTransitionFailureV1::Retryable {
                                detail,
                                custody: DirectionalSdmaDeviceOwnerV1::Native(device),
                            },
                            Gfx942PersistentComputeStoragePromotionCustodyV1::ProcessTeardown(
                                custody,
                            ) => SdmaTransitionFailureV1::ProcessTeardown {
                                detail,
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::StoragePromotion(
                                        custody,
                                    ),
                                ),
                            },
                        }
                    })
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaDeviceOwnerV1::Scripted(device)) => {
                driver.promote_initialized_storage(device)
            }
            #[cfg(test)]
            (_, device) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Static("initialized-storage owner/driver mismatch"),
                custody: scripted_mismatch_device(device, "initialized storage"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn demote(
        &mut self,
        device: DirectionalSdmaDeviceOwnerV1,
    ) -> Result<
        SdmaBufferOwnerV1,
        SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1, SdmaOwnerDiagnosticV1>,
    > {
        match (self, device) {
            (Self::Native(queue), DirectionalSdmaDeviceOwnerV1::Native(device)) => queue
                .demote_directional_persistent_allocation_to_sdma_device_buffer_v1(device)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(device) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: SdmaOwnerDiagnosticV1::Native(error),
                                custody: DirectionalSdmaDeviceOwnerV1::Native(device),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Demotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaDeviceOwnerV1::Scripted(device)) => {
                driver.demote(device).map_err(|failure| match failure {
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
            (_, device) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Scripted(
                    "directional SDMA owner/driver mismatch during demotion".to_owned(),
                ),
                custody: scripted_mismatch_device(device, "demotion"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn submit(
        &mut self,
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: DirectionalSdmaRequestPlanV1,
    ) -> Result<DirectionalSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>>
    {
        let request_slice = requests.as_slice();
        let (host_offset, device_offset, copy_bytes) =
            match validate_window_requests_v1(request_slice) {
                Ok(window) => window,
                Err(detail) => {
                    return Err(SdmaTransitionFailureV1::Retryable {
                        detail,
                        custody: pair,
                    });
                }
            };
        if matches!(&requests, DirectionalSdmaRequestPlanV1::Window(requests) if requests.len() == 1)
        {
            return Err(SdmaTransitionFailureV1::Retryable {
                detail: "one directional SDMA packet must use single-request custody".to_owned(),
                custody: pair,
            });
        }
        let packet_count = requests.packet_count();
        match (self, pair.device, pair.host) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(device),
                SdmaBufferOwnerV1::Native(host),
            ) => {
                match requests {
                    DirectionalSdmaRequestPlanV1::Single(_) => {
                        match queue.submit_directional_persistent_sdma_copy_v1(
                        device,
                        direction,
                        host,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    ) {
                        Ok(submission)
                            if submission.direction() == direction
                                && submission.copy_bytes() == copy_bytes =>
                        {
                            Ok(DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                submission,
                                host_offset,
                                device_offset,
                            })
                        }
                        Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                            detail: "directional SDMA published-single metadata changed unexpectedly"
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
                        Err(failure) => {
                            let (error, custody) = failure.into_parts();
                            Err(match custody {
                                Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                                    allocation,
                                    host,
                                } => SdmaTransitionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    custody: DirectionalSdmaPairOwnerV1 {
                                        device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                                        host: SdmaBufferOwnerV1::Native(host),
                                    },
                                },
                                Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                                    custody,
                                ) => SdmaTransitionFailureV1::ProcessTeardown {
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
                    DirectionalSdmaRequestPlanV1::Window(_) => {
                        match queue.submit_directional_persistent_sdma_window_v1(
                        device,
                        direction,
                        host,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    ) {
                        Ok(submission)
                            if submission.direction() == direction
                                && submission.host_offset() == host_offset
                                && submission.device_offset() == device_offset
                                && submission.copy_bytes() == copy_bytes
                                && submission.packet_count() == packet_count =>
                        {
                            Ok(DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission })
                        }
                        Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                            detail: "directional SDMA published-window metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                                ),
                            ),
                        }),
                        Err(failure) => {
                            let (error, custody) = failure.into_parts();
                            Err(match custody {
                                Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                                    allocation,
                                    host,
                                } => SdmaTransitionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    custody: DirectionalSdmaPairOwnerV1 {
                                        device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                                        host: SdmaBufferOwnerV1::Native(host),
                                    },
                                },
                                Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                                    custody,
                                ) => SdmaTransitionFailureV1::ProcessTeardown {
                                    detail: error.to_string(),
                                    custody: SdmaTerminalCustodyV1::Native(
                                        NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(
                                            custody,
                                        ),
                                    ),
                                },
                            })
                        }
                    }
                    }
                }
            }
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                SdmaBufferOwnerV1::Scripted(host),
            ) => driver.submit(device, host, direction, requests),
            #[cfg(test)]
            (_, device, host) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during publication".to_owned(),
                custody: scripted_mismatch_pair(device, host, "submission"),
            }),
        }
    }

    pub(in crate::kfd_backend) fn execute_synchronous_single(
        &mut self,
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        request: DirectionalSdmaCopyRequestV1,
        timeout: Duration,
    ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaSynchronousExecutionFailureV1> {
        if let Err(detail) = validate_window_requests_v1(core::slice::from_ref(&request)) {
            return Err(
                DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                    detail: SdmaOwnerDiagnosticV1::Message(detail),
                    pair,
                },
            );
        }
        match (self, pair.device, pair.host) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(device),
                SdmaBufferOwnerV1::Native(host),
            ) => match queue.execute_synchronous_directional_persistent_sdma_copy_for_v1(
                device,
                direction,
                host,
                request.host_offset,
                request.device_offset,
                request.copy_bytes,
                timeout,
            ) {
                Ok(completed) => Ok(DirectionalSdmaCompletedOwnerV1::NativeSingle {
                    completed,
                    host_offset: request.host_offset,
                    device_offset: request.device_offset,
                }),
                Err(Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Submission(
                    failure,
                )) => {
                    let (error, custody) = failure.into_parts();
                    Err(match custody {
                        Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                            allocation,
                            host,
                        } => DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            pair: DirectionalSdmaPairOwnerV1 {
                                device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                                host: SdmaBufferOwnerV1::Native(host),
                            },
                        },
                        Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(custody),
                            ),
                        },
                    })
                }
                Err(Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1::Execution(
                    failure,
                )) => {
                    let (error, custody) = failure.into_parts();
                    let is_timeout = is_exact_sdma_timeout_v1(&error);
                    Err(match custody {
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                            if submission.direction() == direction
                                && submission.copy_bytes() == request.copy_bytes
                                && is_timeout =>
                        {
                            DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                                detail: SdmaOwnerDiagnosticV1::Native(error),
                                submission: DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                    submission,
                                    host_offset: request.host_offset,
                                    device_offset: request.device_offset,
                                },
                            }
                        }
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                            if submission.direction() == direction
                                && submission.copy_bytes() == request.copy_bytes =>
                        {
                            DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                                detail: SdmaOwnerDiagnosticV1::UnexpectedRetryable(error),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset: request.host_offset,
                                            device_offset: request.device_offset,
                                        },
                                    ),
                                ),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission) => {
                            DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                                detail: SdmaOwnerDiagnosticV1::Static(
                                    "directional SDMA synchronous wait metadata changed unexpectedly",
                                ),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset: request.host_offset,
                                            device_offset: request.device_offset,
                                        },
                                    ),
                                ),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(custody),
                            ),
                        },
                    })
                }
            },
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                SdmaBufferOwnerV1::Scripted(host),
            ) => {
                let submission = match driver.submit(
                    device,
                    host,
                    direction,
                    DirectionalSdmaRequestPlanV1::Single(request),
                ) {
                    Ok(DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => submission,
                    Ok(_) => unreachable!("scripted submit returns scripted custody"),
                    Err(SdmaTransitionFailureV1::Retryable { detail, custody }) => {
                        return Err(
                            DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                                detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                                pair: custody,
                            },
                        );
                    }
                    Err(SdmaTransitionFailureV1::ProcessTeardown { detail, custody }) => {
                        return Err(
                            DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                                detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                                custody,
                            },
                        );
                    }
                };
                match driver.wait(submission) {
                    Ok(DirectionalSdmaWaitV1::Completed(completed)) => Ok(completed),
                    Ok(DirectionalSdmaWaitV1::Timeout(submission)) => Err(
                        DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                            detail: SdmaOwnerDiagnosticV1::Static("scripted wait timed out"),
                            submission,
                        },
                    ),
                    Err(DirectionalSdmaExecutionFailureV1::Retryable { detail, submission }) => {
                        Err(
                            DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                                detail: SdmaOwnerDiagnosticV1::UnexpectedScriptedRetryable(detail),
                                custody: scripted_mismatch_submission(
                                    submission,
                                    "synchronous non-timeout retryable wait",
                                ),
                            },
                        )
                    }
                    Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown { detail, custody }) => {
                        Err(
                            DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                                detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                                custody,
                            },
                        )
                    }
                }
            }
            #[cfg(test)]
            (_, device, host) => Err(
                DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                    detail: SdmaOwnerDiagnosticV1::Static(
                        "directional SDMA owner/driver mismatch during synchronous execution",
                    ),
                    custody: scripted_mismatch_pair(device, host, "synchronous execution"),
                },
            ),
        }
    }
}
