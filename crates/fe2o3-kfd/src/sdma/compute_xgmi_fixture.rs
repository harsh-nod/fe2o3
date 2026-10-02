//! Constructed mapped controls and production submit/poll; no GPU or Linux route checks.

use super::*;
use crate::queue_linux::doorbell_release_tests::{cleanup_local_doorbell, local_doorbell_value};
use crate::shared_memory::PreparationMemoryFixtureV1;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ComputeXgmiQueueSnapshotV1 {
    pub(crate) poisoned: bool,
    pub(crate) generations: Vec<u32>,
    pub(crate) retained: Vec<Gfx942DeviceMemoryIdentityV1>,
    pub(crate) uncertain: Option<Gfx942SdmaCopyTicketV1>,
    pub(crate) ring: Vec<u8>,
    pub(crate) control: Vec<u8>,
    pub(crate) completions: Vec<u8>,
    pub(crate) doorbell: u64,
}

pub(crate) struct ComputeXgmiQueueFixtureV1 {
    owner: Gfx942SdmaQueueOwnerV1,
    binding: Option<(u64, u64, u32)>,
}

impl ComputeXgmiQueueFixtureV1 {
    pub(crate) fn new(memory: &mut PreparationMemoryFixtureV1, key: QueueKeyV1) -> Self {
        let Gfx942SdmaQueueSetV1::Generic(mut owners) =
            retained_release::fixture::generic(memory, key, Some(0))
        else {
            unreachable!("generic fixture")
        };
        assert_eq!(owners.len(), 1);
        memory.enable_sdma_mapped_bytes_v1();
        Self {
            owner: owners.pop().unwrap(),
            binding: None,
        }
    }

    pub(crate) fn retained_identities(&self) -> Vec<Gfx942DeviceMemoryIdentityV1> {
        self.owner
            .xgmi_records
            .iter()
            .filter_map(Option::as_ref)
            .flat_map(|record| {
                [
                    record.source.lease().storage_identity(),
                    record.destination.lease().storage_identity(),
                ]
            })
            .collect()
    }

    pub(crate) fn snapshot(
        &self,
        memory: &PreparationMemoryFixtureV1,
    ) -> ComputeXgmiQueueSnapshotV1 {
        ComputeXgmiQueueSnapshotV1 {
            poisoned: self.owner.poisoned,
            generations: self.owner.generations.to_vec(),
            retained: self.retained_identities(),
            uncertain: self.owner.uncertain_xgmi_ticket,
            ring: memory.sdma_resource_bytes_v1(self.owner.ring.as_ref().unwrap()),
            control: memory.sdma_resource_bytes_v1(self.owner.control.as_ref().unwrap()),
            completions: memory.sdma_mapped_bytes_v1(self.owner.completions.as_ref().unwrap()),
            doorbell: local_doorbell_value(self.owner.doorbell.as_ref().unwrap()),
        }
    }

    pub(crate) fn fault(
        &mut self,
        memory: &mut PreparationMemoryFixtureV1,
        operation: &'static str,
        panic: bool,
    ) {
        memory.sdma_arm_access_fault_v1(operation, panic);
    }

    pub(crate) fn start_at_ring_tail(&mut self, memory: &mut PreparationMemoryFixtureV1) {
        assert!(self.retained_identities().is_empty() && self.binding.is_none());
        let write = u64::from(GFX942_SDMA_RING_BYTES_V1) - GFX942_SDMA_SUBMISSION_BYTES_V1 as u64;
        memory.sdma_fixture_counters_v1(self.owner.control.as_ref().unwrap(), write, write);
        self.owner.generations[0] = 7;
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn submit(
        &mut self,
        memory: &mut PreparationMemoryFixtureV1,
        destination_memory: &PreparationMemoryFixtureV1,
        source: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        destination: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        offset: u64,
        copy_bytes: u32,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        if custody.ticket.is_some() || custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI copy custody",
            ));
        }
        memory.check_queue_operational_currentness()?;
        let source_address = memory
            .compute_xgmi_facts_v1(source.as_ref().unwrap())?
            .checked_gpu_subrange(offset, u64::from(copy_bytes), 1)
            .ok_or(Gfx942SdmaErrorV1::Contract("fixture source extent"))?;
        let destination_address = destination_memory
            .compute_xgmi_facts_v1(destination.as_ref().unwrap())?
            .checked_gpu_subrange(offset, u64::from(copy_bytes), 1)
            .ok_or(Gfx942SdmaErrorV1::Contract("fixture destination extent"))?;
        assert_ne!(
            source.as_ref().unwrap().lease().storage_identity(),
            destination.as_ref().unwrap().lease().storage_identity()
        );
        self.binding = Some((source_address, destination_address, copy_bytes));
        match self.owner.submit_xgmi(
            memory,
            source,
            source_address,
            destination,
            destination_address,
            copy_bytes,
        ) {
            Ok(ticket) => custody.ticket = Some(ticket),
            Err(error) => {
                custody.ticket = self.owner.uncertain_xgmi_ticket();
                return Err(error);
            }
        }
        memory
            .check_queue_operational_currentness()
            .map_err(Into::into)
    }

    pub(crate) fn poll(
        &mut self,
        memory: &mut PreparationMemoryFixtureV1,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        memory.check_queue_operational_currentness()?;
        if custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI completion",
            ));
        }
        let ticket = custody
            .ticket
            .ok_or(Gfx942SdmaErrorV1::Contract("missing compute-XGMI ticket"))?;
        let polled = self.owner.poll_xgmi_in_current_scope(memory, ticket);
        custody.retain_poll_then_check(polled, || {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        })
    }

    pub(crate) fn wait(
        &mut self,
        memory: &mut PreparationMemoryFixtureV1,
        timeout: Duration,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        memory.check_queue_operational_currentness()?;
        if custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI completion",
            ));
        }
        let ticket = custody
            .ticket
            .ok_or(Gfx942SdmaErrorV1::Contract("missing compute-XGMI ticket"))?;
        let completed = self.owner.wait_xgmi_for_in_current_scope(
            memory,
            ticket,
            XgmiSingleDeadlineV1::Relative(timeout),
        );
        custody.retain_completion_then_check(completed, || {
            memory
                .check_queue_operational_currentness()
                .map_err(Into::into)
        })
    }

    pub(crate) fn complete(
        &mut self,
        memory: &mut PreparationMemoryFixtureV1,
        custody: &ComputeXgmiCopyCustodyV1,
    ) {
        self.owner.require_live().unwrap();
        let ticket = custody.ticket.unwrap();
        let slot = self.owner.validate_xgmi_ticket(ticket).unwrap();
        assert!(custody.completed.is_none() && self.owner.uncertain_xgmi_ticket.is_none());
        let (source, destination, bytes) = self.binding.unwrap();
        let record = self.owner.xgmi_records[slot].as_ref().unwrap();
        assert_eq!(record.copy_bytes, bytes);
        let completion_address = memory
            .single_host_facts(self.owner.completions.as_ref().unwrap())
            .unwrap()
            .gpu_va()
            + (slot * 8) as u64;
        let packet = Gfx942SdmaCopySubmissionV1::new(
            source,
            destination,
            bytes,
            completion_address,
            ticket.generation,
        )
        .unwrap();
        let observed = self.snapshot(memory);
        assert_eq!(&observed.ring[slot * 64..slot * 64 + 64], packet.bytes());
        let (write, read) = memory
            .observe_aql_control_counters_in_current_scope(self.owner.control.as_mut().unwrap())
            .unwrap();
        assert_eq!(observed.doorbell, write);
        assert_eq!(write - read, 64);
        assert_eq!(&observed.completions[slot * 8..slot * 8 + 8], &[0; 8]);
        memory
            .overwrite_mapped_host_visible_subrange_in_current_scope(
                self.owner.completions.as_mut().unwrap(),
                (slot * 8) as u64,
                &i64::from(ticket.generation).to_le_bytes(),
            )
            .unwrap();
        memory.sdma_fixture_counters_v1(self.owner.control.as_ref().unwrap(), write, write);
    }

    pub(crate) fn poison(&mut self) {
        self.owner.poisoned = true;
    }
}

impl Drop for ComputeXgmiQueueFixtureV1 {
    fn drop(&mut self) {
        if let Some(doorbell) = self.owner.doorbell.as_mut() {
            cleanup_local_doorbell(doorbell);
        }
    }
}
