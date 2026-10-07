use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn detach_recycled_dispatch(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if self.cpu_queue.is_some() {
            return self.detach_cpu_recycled_dispatch_v1();
        }
        self.synchronize_recycled_dispatch_data_v1()?;
        if self.recycled_dispatch.is_none() {
            return Ok(());
        }
        if self.resident_data.is_some() {
            return Err(
                self.terminal_error("recycled detach conflicts with retained resident DATA")
            );
        }
        let native_lane = self.selected_native_compute_lane_v1()?;
        let recycled = &mut self.recycled_dispatch;
        let resident = &mut self.resident_data;
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD recycled dispatch exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        let detached = queue
                            .detach_recycled_fixed_dispatch()
                            .map_err(|error| format!("KFD recycled dispatch detach: {error}"))?;
                        // Returned DATA must be rooted before the lane loan closes.
                        *resident = Some(ResidentDataRosterV1 {
                            descriptors: recycled
                                .take()
                                .expect("indexed recycled descriptors")
                                .descriptors,
                            data: detached.into_data(),
                        });
                        Ok(())
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
            });
        match result {
            Ok(()) => Ok(()),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    pub(in crate::kfd_backend) fn release_resident_data(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.resident_data.is_none() {
            return Ok(());
        }
        let native_lane = self.selected_native_compute_lane_v1()?;
        let resident = self.resident_data.take().expect("checked resident data");
        let result = with_resident_release_custody_v1(
            resident.descriptors,
            resident.data,
            |custody| {
                self.queue
                    .as_mut()
                    .ok_or_else(|| "KFD resident data exists without a native queue".to_owned())?
                    .with_compute_lane_v1(native_lane, |queue| {
                        release_resident_data_v1(queue, custody)
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
            },
            core::mem::forget,
        );
        match result {
            Ok(()) => Ok(()),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }
}
