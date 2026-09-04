//! Private move-only adapter around the directional persistent SDMA owner API.
//!
//! The native branch is a type-preserving forwarding layer. Tests use an exact
//! FIFO script with opaque owners, so facade failure paths can be exercised
//! without a KFD device or constructors for native custody types.

// Error transitions return large native owners inline. Boxing after a native
// failure would add an allocation failure exactly where custody cannot be lost.
#![allow(
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::result_large_err
)]

use fe2o3_kfd::{
    ComputeAqlQueueSessionV1, Gfx942DirectionalPersistentSdmaCompletedV1,
    Gfx942DirectionalPersistentSdmaCopyPollV1, Gfx942DirectionalPersistentSdmaDemotionCustodyV1,
    Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaExecutionCustodyV1,
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaPromotionCustodyV1,
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaSubmissionCustodyV1,
    Gfx942DirectionalPersistentSdmaSubmissionV1, Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
    Gfx942DirectionalQueuePersistentAllocationV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942SdmaBufferV1,
};
use std::time::Duration;

#[derive(Debug)]
pub(super) enum SdmaBufferOwnerV1 {
    Native(Gfx942SdmaBufferV1),
    #[cfg(test)]
    Scripted(ScriptedBufferOwnerV1),
}

#[derive(Debug)]
pub(super) enum DirectionalSdmaDeviceOwnerV1 {
    Native(Gfx942DirectionalQueuePersistentAllocationV1),
    #[cfg(test)]
    Scripted(ScriptedDeviceOwnerV1),
}

#[cfg(test)]
impl DirectionalSdmaDeviceOwnerV1 {
    pub(crate) fn scripted_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Scripted(device) => Some(&device.bytes),
            Self::Native(_) => None,
        }
    }

    pub(crate) fn scripted_bytes_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            Self::Scripted(device) => Some(&mut device.bytes),
            Self::Native(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct DirectionalSdmaPairOwnerV1 {
    pub(super) device: DirectionalSdmaDeviceOwnerV1,
    pub(super) host: SdmaBufferOwnerV1,
}

#[derive(Debug)]
pub(super) enum DirectionalSdmaSubmissionOwnerV1 {
    Native(Gfx942DirectionalPersistentSdmaSubmissionV1),
    #[cfg(test)]
    Scripted(ScriptedSubmissionOwnerV1),
}

pub(super) enum DirectionalSdmaCompletedOwnerV1 {
    Native(Gfx942DirectionalPersistentSdmaCompletedV1),
    #[cfg(test)]
    Scripted(ScriptedCompletedOwnerV1),
}

impl core::fmt::Debug for DirectionalSdmaCompletedOwnerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DirectionalSdmaCompletedOwnerV1")
            .field("direction", &self.direction())
            .field("copy_bytes", &self.copy_bytes())
            .finish_non_exhaustive()
    }
}

impl DirectionalSdmaCompletedOwnerV1 {
    pub(super) fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        match self {
            Self::Native(completed) => completed.direction(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.direction,
        }
    }

    pub(super) fn copy_bytes(&self) -> u32 {
        match self {
            Self::Native(completed) => completed.copy_bytes(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.copy_bytes,
        }
    }
}

pub(super) enum NativeDirectionalSdmaTerminalCustodyV1 {
    Promotion(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1),
    Demotion(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1),
    Submission(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
    Retirement {
        failure: Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
        host: Gfx942SdmaBufferV1,
    },
}

pub(super) enum SdmaTerminalCustodyV1 {
    Native(NativeDirectionalSdmaTerminalCustodyV1),
    #[cfg(test)]
    Scripted(ScriptedTerminalCustodyV1),
}

pub(super) enum SdmaTransitionFailureV1<R> {
    Retryable {
        detail: String,
        custody: R,
    },
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum DirectionalSdmaExecutionFailureV1 {
    Retryable {
        detail: String,
        submission: DirectionalSdmaSubmissionOwnerV1,
    },
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum SdmaRecycleFailureV1 {
    Recovered {
        detail: String,
        buffer: SdmaBufferOwnerV1,
    },
    Ambiguous {
        detail: String,
    },
    #[cfg(test)]
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum DirectionalSdmaPollV1 {
    Pending(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
}

pub(super) enum DirectionalSdmaOpsV1<'a> {
    Native(&'a mut ComputeAqlQueueSessionV1),
    #[cfg(test)]
    Scripted(&'a mut ScriptedSdmaDriverV1),
}

impl<'a> DirectionalSdmaOpsV1<'a> {
    pub(super) fn allocate_host(&mut self, byte_len: usize) -> Result<SdmaBufferOwnerV1, String> {
        match self {
            Self::Native(queue) => queue
                .allocate_sdma_pooled_host_buffer(byte_len)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Scripted(driver) => driver.allocate_buffer(byte_len, ScriptedBufferKindV1::Host),
        }
    }

    pub(super) fn allocate_device_buffer(
        &mut self,
        byte_len: u64,
        alignment: u64,
    ) -> Result<SdmaBufferOwnerV1, String> {
        match self {
            Self::Native(queue) => queue
                .allocate_sdma_pooled_device_buffer(byte_len, alignment)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Scripted(driver) => {
                let len = usize::try_from(byte_len)
                    .map_err(|_| "scripted device allocation length overflow".to_owned())?;
                driver.allocate_buffer(len, ScriptedBufferKindV1::Device)
            }
        }
    }

    pub(super) fn write_host(
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

    pub(super) fn read_host(
        &mut self,
        buffer: &SdmaBufferOwnerV1,
        offset: u64,
        byte_len: u64,
    ) -> Result<Box<[u8]>, String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .read_sdma_host_buffer(buffer, offset, byte_len)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.read_host(buffer, offset, byte_len)
            }
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while reading {:?}",
                buffer
            )),
        }
    }

    pub(super) fn promote(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<DirectionalSdmaDeviceOwnerV1, SdmaTransitionFailureV1<SdmaBufferOwnerV1>> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .promote_sdma_device_buffer_to_directional_persistent_allocation_v1(buffer)
                .map(DirectionalSdmaDeviceOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(buffer) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: error.to_string(),
                                custody: SdmaBufferOwnerV1::Native(buffer),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Promotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver.promote(buffer),
            #[cfg(test)]
            (_, buffer) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during promotion".to_owned(),
                custody: scripted_mismatch_buffer(buffer, "promotion"),
            }),
        }
    }

    pub(super) fn demote(
        &mut self,
        device: DirectionalSdmaDeviceOwnerV1,
    ) -> Result<SdmaBufferOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1>> {
        match (self, device) {
            (Self::Native(queue), DirectionalSdmaDeviceOwnerV1::Native(device)) => queue
                .demote_directional_persistent_allocation_to_sdma_device_buffer_v1(device)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(device) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: error.to_string(),
                                custody: DirectionalSdmaDeviceOwnerV1::Native(device),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Demotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaDeviceOwnerV1::Scripted(device)) => {
                driver.demote(device)
            }
            #[cfg(test)]
            (_, device) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during demotion".to_owned(),
                custody: scripted_mismatch_device(device, "demotion"),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn submit(
        &mut self,
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    ) -> Result<DirectionalSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>>
    {
        match (self, pair.device, pair.host) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(device),
                SdmaBufferOwnerV1::Native(host),
            ) => queue
                .submit_directional_persistent_sdma_copy_v1(
                    device,
                    direction,
                    host,
                    host_offset,
                    device_offset,
                    copy_bytes,
                )
                .map(DirectionalSdmaSubmissionOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
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
                                NativeDirectionalSdmaTerminalCustodyV1::Submission(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                SdmaBufferOwnerV1::Scripted(host),
            ) => driver.submit(
                device,
                host,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
            ),
            #[cfg(test)]
            (_, device, host) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during publication".to_owned(),
                custody: scripted_mismatch_pair(device, host, "submission"),
            }),
        }
    }

    pub(super) fn poll(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), DirectionalSdmaSubmissionOwnerV1::Native(submission)) => queue
                .poll_directional_persistent_sdma_copy_v1(submission)
                .map(|poll| match poll {
                    Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission) => {
                        DirectionalSdmaPollV1::Pending(DirectionalSdmaSubmissionOwnerV1::Native(
                            submission,
                        ))
                    }
                    Gfx942DirectionalPersistentSdmaCopyPollV1::Completed(completed) => {
                        DirectionalSdmaPollV1::Completed(DirectionalSdmaCompletedOwnerV1::Native(
                            completed,
                        ))
                    }
                })
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission) => {
                            DirectionalSdmaExecutionFailureV1::Retryable {
                                detail: error.to_string(),
                                submission: DirectionalSdmaSubmissionOwnerV1::Native(submission),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Submission(custody),
                            ),
                        },
                    }
                }),
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

    pub(super) fn wait(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
        timeout: Duration,
    ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), DirectionalSdmaSubmissionOwnerV1::Native(submission)) => queue
                .wait_directional_persistent_sdma_copy_for_v1(submission, timeout)
                .map(DirectionalSdmaCompletedOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission) => {
                            DirectionalSdmaExecutionFailureV1::Retryable {
                                detail: error.to_string(),
                                submission: DirectionalSdmaSubmissionOwnerV1::Native(submission),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Submission(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.wait(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during wait".to_owned(),
                custody: scripted_mismatch_submission(submission, "wait"),
            }),
        }
    }

    pub(super) fn retire(
        &mut self,
        completed: DirectionalSdmaCompletedOwnerV1,
    ) -> Result<DirectionalSdmaPairOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>>
    {
        match (self, completed) {
            (Self::Native(_), DirectionalSdmaCompletedOwnerV1::Native(completed)) => {
                let (device, host, frontier) = completed.into_parts();
                match device.retire_settled_frontier_v1(frontier) {
                    Ok(device) => Ok(DirectionalSdmaPairOwnerV1 {
                        device: DirectionalSdmaDeviceOwnerV1::Native(device),
                        host: SdmaBufferOwnerV1::Native(host),
                    }),
                    Err(failure) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "frontier retirement failed".to_owned(),
                        custody: SdmaTerminalCustodyV1::Native(
                            NativeDirectionalSdmaTerminalCustodyV1::Retirement { failure, host },
                        ),
                    }),
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaCompletedOwnerV1::Scripted(completed)) => {
                driver.retire(completed)
            }
            #[cfg(test)]
            (_, completed) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during retirement".to_owned(),
                custody: scripted_mismatch_completed(completed, "retirement"),
            }),
        }
    }

    pub(super) fn recycle(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<(), SdmaRecycleFailureV1> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => {
                queue.recycle_sdma_buffer(buffer).map_err(|failure| {
                    let (error, recovered) = failure.into_parts();
                    match recovered {
                        Some(buffer) => SdmaRecycleFailureV1::Recovered {
                            detail: error.to_string(),
                            buffer: SdmaBufferOwnerV1::Native(buffer),
                        },
                        None => SdmaRecycleFailureV1::Ambiguous {
                            detail: error.to_string(),
                        },
                    }
                })
            }
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver.recycle(buffer),
            #[cfg(test)]
            (_, buffer) => Err(SdmaRecycleFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during recycle".to_owned(),
                custody: scripted_mismatch_buffer(buffer, "recycle"),
            }),
        }
    }
}

#[cfg(test)]
mod scripted {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedFailureModeV1 {
        Success,
        Retryable,
        ProcessTeardown,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedExecutionOutcomeV1 {
        Pending,
        Completed {
            direction: Option<Gfx942PersistentSdmaDirectionV1>,
            copy_bytes: Option<u32>,
        },
        Retryable,
        ProcessTeardown,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedRecycleOutcomeV1 {
        Success,
        Recovered,
        Ambiguous,
    }

    #[derive(Debug)]
    pub(crate) enum ScriptedSdmaStepV1 {
        Allocate {
            kind: ScriptedBufferKindV1,
            byte_len: usize,
        },
        Write {
            offset: u64,
            byte_len: usize,
        },
        Read {
            offset: u64,
            byte_len: u64,
        },
        Promote(ScriptedFailureModeV1),
        Demote(ScriptedFailureModeV1),
        Submit {
            direction: Gfx942PersistentSdmaDirectionV1,
            host_offset: u64,
            device_offset: u64,
            copy_bytes: u32,
            outcome: ScriptedFailureModeV1,
        },
        Poll(ScriptedExecutionOutcomeV1),
        Wait(ScriptedExecutionOutcomeV1),
        Retire(ScriptedFailureModeV1),
        Recycle(ScriptedRecycleOutcomeV1),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedBufferKindV1 {
        Host,
        Device,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ScriptedOwnerRoleV1 {
        Buffer(ScriptedBufferKindV1),
        Device,
    }

    #[derive(Debug, Default)]
    struct ScriptedCustodyLedgerV1 {
        next_id: u64,
        owners: HashMap<u64, ScriptedOwnerRoleV1>,
        unexpected_drops: usize,
    }

    #[derive(Debug)]
    struct ScriptedOwnerTokenV1 {
        id: u64,
        role: ScriptedOwnerRoleV1,
        ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
        armed: bool,
    }

    impl ScriptedOwnerTokenV1 {
        fn new(role: ScriptedOwnerRoleV1, ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>) -> Self {
            let id = {
                let mut ledger = ledger.borrow_mut();
                ledger.next_id += 1;
                let id = ledger.next_id;
                assert!(ledger.owners.insert(id, role).is_none());
                id
            };
            Self {
                id,
                role,
                ledger,
                armed: true,
            }
        }

        fn transition(mut self, role: ScriptedOwnerRoleV1) -> Self {
            let prior = self.ledger.borrow_mut().owners.insert(self.id, role);
            assert_eq!(prior, Some(self.role));
            self.role = role;
            self
        }

        fn release(mut self) {
            let prior = self.ledger.borrow_mut().owners.remove(&self.id);
            assert_eq!(prior, Some(self.role));
            self.armed = false;
        }
    }

    impl Drop for ScriptedOwnerTokenV1 {
        fn drop(&mut self) {
            if self.armed {
                let _ = self.ledger.borrow_mut().owners.remove(&self.id);
                self.ledger.borrow_mut().unexpected_drops += 1;
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedBufferOwnerV1 {
        token: ScriptedOwnerTokenV1,
        kind: ScriptedBufferKindV1,
        bytes: Vec<u8>,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedDeviceOwnerV1 {
        token: ScriptedOwnerTokenV1,
        pub(super) bytes: Vec<u8>,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedSubmissionOwnerV1 {
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedCompletedOwnerV1 {
        pair: DirectionalSdmaPairOwnerV1,
        pub(super) direction: Gfx942PersistentSdmaDirectionV1,
        pub(super) copy_bytes: u32,
    }

    #[allow(dead_code)]
    pub(crate) enum ScriptedTerminalCustodyV1 {
        Buffer(SdmaBufferOwnerV1),
        Device(DirectionalSdmaDeviceOwnerV1),
        Pair(DirectionalSdmaPairOwnerV1),
        Submission(DirectionalSdmaSubmissionOwnerV1),
        Completed(DirectionalSdmaCompletedOwnerV1),
    }

    pub(crate) struct ScriptedSdmaDriverV1 {
        steps: VecDeque<ScriptedSdmaStepV1>,
        ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
    }

    impl core::fmt::Debug for ScriptedSdmaDriverV1 {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter
                .debug_struct("ScriptedSdmaDriverV1")
                .field("steps", &self.steps)
                .field("owners", &self.ledger.borrow().owners)
                .field("unexpected_drops", &self.ledger.borrow().unexpected_drops)
                .finish()
        }
    }

    impl ScriptedSdmaDriverV1 {
        pub(crate) fn new(steps: impl IntoIterator<Item = ScriptedSdmaStepV1>) -> Self {
            Self {
                steps: steps.into_iter().collect(),
                ledger: Rc::new(RefCell::new(ScriptedCustodyLedgerV1::default())),
            }
        }

        fn pop(&mut self) -> Result<ScriptedSdmaStepV1, String> {
            self.steps
                .pop_front()
                .ok_or_else(|| "scripted directional SDMA operation was not expected".to_owned())
        }

        pub(crate) fn is_exhausted(&self) -> bool {
            self.steps.is_empty()
        }

        pub(crate) fn remaining_steps(&self) -> usize {
            self.steps.len()
        }

        pub(crate) fn live_owner_count(&self) -> usize {
            self.ledger.borrow().owners.len()
        }

        pub(crate) fn unexpected_drops(&self) -> usize {
            self.ledger.borrow().unexpected_drops
        }

        pub(crate) fn test_host_owner(&self, byte_len: usize) -> SdmaBufferOwnerV1 {
            SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Host),
                    Rc::clone(&self.ledger),
                ),
                kind: ScriptedBufferKindV1::Host,
                bytes: vec![0; byte_len],
            })
        }

        pub(crate) fn test_device_owner(&self, byte_len: usize) -> DirectionalSdmaDeviceOwnerV1 {
            DirectionalSdmaDeviceOwnerV1::Scripted(ScriptedDeviceOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Device,
                    Rc::clone(&self.ledger),
                ),
                bytes: vec![0; byte_len],
            })
        }

        fn owns_token(&self, token: &ScriptedOwnerTokenV1) -> bool {
            Rc::ptr_eq(&self.ledger, &token.ledger)
        }

        fn owns_buffer(&self, buffer: &ScriptedBufferOwnerV1) -> bool {
            self.owns_token(&buffer.token)
        }

        fn owns_device(&self, device: &ScriptedDeviceOwnerV1) -> bool {
            self.owns_token(&device.token)
        }

        fn owns_pair(&self, pair: &DirectionalSdmaPairOwnerV1) -> bool {
            matches!(
                (&pair.device, &pair.host),
                (
                    DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    SdmaBufferOwnerV1::Scripted(host)
                ) if self.owns_device(device) && self.owns_buffer(host)
            )
        }

        fn owns_submission(&self, submission: &ScriptedSubmissionOwnerV1) -> bool {
            self.owns_pair(&submission.pair)
        }

        fn owns_completed(&self, completed: &ScriptedCompletedOwnerV1) -> bool {
            self.owns_pair(&completed.pair)
        }

        pub(super) fn allocate_buffer(
            &mut self,
            byte_len: usize,
            kind: ScriptedBufferKindV1,
        ) -> Result<SdmaBufferOwnerV1, String> {
            match self.pop()? {
                ScriptedSdmaStepV1::Allocate {
                    kind: expected_kind,
                    byte_len: expected_len,
                } if expected_kind == kind && expected_len == byte_len => {}
                step => return Err(format!("scripted SDMA allocation mismatch: {step:?}")),
            }
            Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Buffer(kind),
                    Rc::clone(&self.ledger),
                ),
                kind,
                bytes: vec![0; byte_len],
            }))
        }

        pub(super) fn write_host(
            &mut self,
            buffer: &mut ScriptedBufferOwnerV1,
            offset: u64,
            bytes: &[u8],
        ) -> Result<(), String> {
            if !self.owns_buffer(buffer) {
                return Err("scripted SDMA write owner belongs to another driver".to_owned());
            }
            match self.pop()? {
                ScriptedSdmaStepV1::Write {
                    offset: expected_offset,
                    byte_len,
                } if expected_offset == offset && byte_len == bytes.len() => {}
                step => return Err(format!("scripted SDMA write mismatch: {step:?}")),
            }
            if buffer.kind != ScriptedBufferKindV1::Host {
                return Err("scripted SDMA write requires host storage".to_owned());
            }
            let start = usize::try_from(offset).map_err(|_| "scripted write offset overflow")?;
            let end = start
                .checked_add(bytes.len())
                .filter(|end| *end <= buffer.bytes.len())
                .ok_or("scripted write exceeds buffer")?;
            buffer.bytes[start..end].copy_from_slice(bytes);
            Ok(())
        }

        pub(super) fn read_host(
            &mut self,
            buffer: &ScriptedBufferOwnerV1,
            offset: u64,
            byte_len: u64,
        ) -> Result<Box<[u8]>, String> {
            if !self.owns_buffer(buffer) {
                return Err("scripted SDMA read owner belongs to another driver".to_owned());
            }
            match self.pop()? {
                ScriptedSdmaStepV1::Read {
                    offset: expected_offset,
                    byte_len: expected_len,
                } if expected_offset == offset && expected_len == byte_len => {}
                step => return Err(format!("scripted SDMA read mismatch: {step:?}")),
            }
            if buffer.kind != ScriptedBufferKindV1::Host {
                return Err("scripted SDMA read requires host storage".to_owned());
            }
            let start = usize::try_from(offset).map_err(|_| "scripted read offset overflow")?;
            let len = usize::try_from(byte_len).map_err(|_| "scripted read length overflow")?;
            let end = start
                .checked_add(len)
                .filter(|end| *end <= buffer.bytes.len())
                .ok_or("scripted read exceeds buffer")?;
            Ok(buffer.bytes[start..end].into())
        }

        pub(super) fn promote(
            &mut self,
            buffer: ScriptedBufferOwnerV1,
        ) -> Result<DirectionalSdmaDeviceOwnerV1, SdmaTransitionFailureV1<SdmaBufferOwnerV1>>
        {
            if !self.owns_buffer(&buffer) {
                return Err(scripted_buffer_mismatch(
                    buffer,
                    "promotion owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Promote(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_buffer_mismatch(
                        buffer,
                        format!("promotion mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_buffer_mismatch(buffer, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(DirectionalSdmaDeviceOwnerV1::Scripted(
                    ScriptedDeviceOwnerV1 {
                        token: buffer.token.transition(ScriptedOwnerRoleV1::Device),
                        bytes: buffer.bytes,
                    },
                )),
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted promotion retryable".to_owned(),
                    custody: SdmaBufferOwnerV1::Scripted(buffer),
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted promotion teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    })
                }
            }
        }

        pub(super) fn demote(
            &mut self,
            device: ScriptedDeviceOwnerV1,
        ) -> Result<SdmaBufferOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1>>
        {
            if !self.owns_device(&device) {
                return Err(scripted_device_mismatch(
                    device,
                    "demotion owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Demote(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_device_mismatch(
                        device,
                        format!("demotion mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_device_mismatch(device, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => {
                    Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                        token: device
                            .token
                            .transition(ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Device)),
                        kind: ScriptedBufferKindV1::Device,
                        bytes: device.bytes,
                    }))
                }
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted demotion retryable".to_owned(),
                    custody: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted demotion teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Device(
                                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                            ),
                        ),
                    })
                }
            }
        }

        pub(super) fn submit(
            &mut self,
            device: ScriptedDeviceOwnerV1,
            host: ScriptedBufferOwnerV1,
            direction: Gfx942PersistentSdmaDirectionV1,
            host_offset: u64,
            device_offset: u64,
            copy_bytes: u32,
        ) -> Result<
            DirectionalSdmaSubmissionOwnerV1,
            SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>,
        > {
            let pair = DirectionalSdmaPairOwnerV1 {
                device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                host: SdmaBufferOwnerV1::Scripted(host),
            };
            if !self.owns_pair(&pair) {
                return Err(scripted_pair_mismatch(
                    pair,
                    "submission owners belong to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Submit {
                    direction: expected_direction,
                    host_offset: expected_host_offset,
                    device_offset: expected_device_offset,
                    copy_bytes: expected_copy_bytes,
                    outcome,
                }) if expected_direction == direction
                    && expected_host_offset == host_offset
                    && expected_device_offset == device_offset
                    && expected_copy_bytes == copy_bytes =>
                {
                    outcome
                }
                Ok(step) => {
                    return Err(scripted_pair_mismatch(
                        pair,
                        format!("submission mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_pair_mismatch(pair, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(DirectionalSdmaSubmissionOwnerV1::Scripted(
                    ScriptedSubmissionOwnerV1 {
                        pair,
                        direction,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    },
                )),
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted submission retryable".to_owned(),
                    custody: pair,
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted submission teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(
                            pair,
                        )),
                    })
                }
            }
        }

        pub(super) fn poll(
            &mut self,
            submission: ScriptedSubmissionOwnerV1,
        ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
            if !self.owns_submission(&submission) {
                return Err(scripted_submission_mismatch(
                    submission,
                    "poll owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Poll(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_submission_mismatch(
                        submission,
                        format!("poll mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_submission_mismatch(submission, detail)),
            };
            execute_outcome(submission, outcome, "poll")
        }

        pub(super) fn wait(
            &mut self,
            submission: ScriptedSubmissionOwnerV1,
        ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaExecutionFailureV1> {
            if !self.owns_submission(&submission) {
                return Err(scripted_submission_mismatch(
                    submission,
                    "wait owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Wait(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_submission_mismatch(
                        submission,
                        format!("wait mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_submission_mismatch(submission, detail)),
            };
            match execute_outcome(submission, outcome, "wait")? {
                DirectionalSdmaPollV1::Completed(completed) => Ok(completed),
                DirectionalSdmaPollV1::Pending(submission) => {
                    Err(DirectionalSdmaExecutionFailureV1::Retryable {
                        detail: "scripted wait timed out".to_owned(),
                        submission,
                    })
                }
            }
        }

        pub(super) fn retire(
            &mut self,
            completed: ScriptedCompletedOwnerV1,
        ) -> Result<
            DirectionalSdmaPairOwnerV1,
            SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>,
        > {
            if !self.owns_completed(&completed) {
                return Err(scripted_completed_mismatch(
                    completed,
                    "retirement owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Retire(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_completed_mismatch(
                        completed,
                        format!("retirement mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_completed_mismatch(completed, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(completed.pair),
                ScriptedFailureModeV1::Retryable | ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted retirement failure".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                            ),
                        ),
                    })
                }
            }
        }

        pub(super) fn recycle(
            &mut self,
            buffer: ScriptedBufferOwnerV1,
        ) -> Result<(), SdmaRecycleFailureV1> {
            if !self.owns_buffer(&buffer) {
                return Err(SdmaRecycleFailureV1::ProcessTeardown {
                    detail: "recycle owner belongs to another driver".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                        SdmaBufferOwnerV1::Scripted(buffer),
                    )),
                });
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Recycle(outcome)) => outcome,
                Ok(step) => {
                    return Err(SdmaRecycleFailureV1::ProcessTeardown {
                        detail: format!("recycle mismatch: {step:?}"),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    });
                }
                Err(detail) => {
                    return Err(SdmaRecycleFailureV1::ProcessTeardown {
                        detail,
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    });
                }
            };
            match outcome {
                ScriptedRecycleOutcomeV1::Success => {
                    buffer.token.release();
                    Ok(())
                }
                ScriptedRecycleOutcomeV1::Recovered => Err(SdmaRecycleFailureV1::Recovered {
                    detail: "scripted recycle recovered".to_owned(),
                    buffer: SdmaBufferOwnerV1::Scripted(buffer),
                }),
                ScriptedRecycleOutcomeV1::Ambiguous => {
                    buffer.token.release();
                    Err(SdmaRecycleFailureV1::Ambiguous {
                        detail: "scripted recycle ambiguous".to_owned(),
                    })
                }
            }
        }
    }

    fn execute_outcome(
        mut submission: ScriptedSubmissionOwnerV1,
        outcome: ScriptedExecutionOutcomeV1,
        operation: &'static str,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        match outcome {
            ScriptedExecutionOutcomeV1::Pending => Ok(DirectionalSdmaPollV1::Pending(
                DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
            )),
            ScriptedExecutionOutcomeV1::Retryable => {
                Err(DirectionalSdmaExecutionFailureV1::Retryable {
                    detail: format!("scripted {operation} retryable"),
                    submission: DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
                })
            }
            ScriptedExecutionOutcomeV1::ProcessTeardown => {
                Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                    detail: format!("scripted {operation} teardown"),
                    custody: SdmaTerminalCustodyV1::Scripted(
                        ScriptedTerminalCustodyV1::Submission(
                            DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
                        ),
                    ),
                })
            }
            ScriptedExecutionOutcomeV1::Completed {
                direction,
                copy_bytes,
            } => {
                let len = usize::try_from(submission.copy_bytes).expect("u32 fits usize");
                let host_start = usize::try_from(submission.host_offset)
                    .expect("admitted scripted host offset fits usize");
                let device_start = usize::try_from(submission.device_offset)
                    .expect("admitted scripted device offset fits usize");
                let (device, host) = match (&mut submission.pair.device, &mut submission.pair.host)
                {
                    (
                        DirectionalSdmaDeviceOwnerV1::Scripted(device),
                        SdmaBufferOwnerV1::Scripted(host),
                    ) => (device, host),
                    _ => unreachable!("scripted submission retains scripted pair"),
                };
                match submission.direction {
                    Gfx942PersistentSdmaDirectionV1::HostToDevice => device.bytes
                        [device_start..device_start + len]
                        .copy_from_slice(&host.bytes[host_start..host_start + len]),
                    Gfx942PersistentSdmaDirectionV1::DeviceToHost => host.bytes
                        [host_start..host_start + len]
                        .copy_from_slice(&device.bytes[device_start..device_start + len]),
                }
                Ok(DirectionalSdmaPollV1::Completed(
                    DirectionalSdmaCompletedOwnerV1::Scripted(ScriptedCompletedOwnerV1 {
                        pair: submission.pair,
                        direction: direction.unwrap_or(submission.direction),
                        copy_bytes: copy_bytes.unwrap_or(submission.copy_bytes),
                    }),
                ))
            }
        }
    }

    fn scripted_buffer_mismatch(
        buffer: ScriptedBufferOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<SdmaBufferOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                SdmaBufferOwnerV1::Scripted(buffer),
            )),
        }
    }

    fn scripted_device_mismatch(
        device: ScriptedDeviceOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
            )),
        }
    }

    fn scripted_pair_mismatch(
        pair: DirectionalSdmaPairOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(pair)),
        }
    }

    fn scripted_submission_mismatch(
        submission: ScriptedSubmissionOwnerV1,
        detail: String,
    ) -> DirectionalSdmaExecutionFailureV1 {
        DirectionalSdmaExecutionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(
                DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
            )),
        }
    }

    fn scripted_completed_mismatch(
        completed: ScriptedCompletedOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(
                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
            )),
        }
    }
}

#[cfg(test)]
#[allow(unused_imports)]
pub(super) use scripted::{
    ScriptedBufferKindV1, ScriptedBufferOwnerV1, ScriptedCompletedOwnerV1, ScriptedDeviceOwnerV1,
    ScriptedExecutionOutcomeV1, ScriptedFailureModeV1, ScriptedRecycleOutcomeV1,
    ScriptedSdmaDriverV1, ScriptedSdmaStepV1, ScriptedSubmissionOwnerV1, ScriptedTerminalCustodyV1,
};

#[cfg(test)]
fn scripted_mismatch_buffer(
    buffer: SdmaBufferOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(buffer))
}

#[cfg(test)]
fn scripted_mismatch_device(
    device: DirectionalSdmaDeviceOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(device))
}

#[cfg(test)]
fn scripted_mismatch_pair(
    device: DirectionalSdmaDeviceOwnerV1,
    host: SdmaBufferOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(
        DirectionalSdmaPairOwnerV1 { device, host },
    ))
}

#[cfg(test)]
fn scripted_mismatch_submission(
    submission: DirectionalSdmaSubmissionOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(submission))
}

#[cfg(test)]
fn scripted_mismatch_completed(
    completed: DirectionalSdmaCompletedOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(completed))
}
