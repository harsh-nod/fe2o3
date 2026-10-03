//! Nonexecuting access to the actual retained device, including lazy bootstrap.

use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(crate) fn with_retained_preparation_device_v1<R>(
        &mut self,
        backend_device: u64,
        prepare: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_preparation_child_v1(backend_device, |child| {
            child.with_retained_preparation_device_v1(backend_device, prepare)
        })
    }

    fn with_preparation_child_v1<R>(
        &mut self,
        backend_device: u64,
        prepare: impl FnOnce(
            &mut KfdRuntimeBackendV1,
        ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let index = self.child_for_device(backend_device)?;
        if self
            .children
            .get(index)
            .map(|child| child.description.backend_device)
            != Some(backend_device)
            || self.compute_xgmi_children.len() != self.children.len()
        {
            return self.latch(Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "retained preparation child routing is inconsistent",
                ),
            )));
        }
        let live = self.children[index].require_live();
        self.latch(live)?;
        self.require_compute_xgmi_child_available_v1(index)?;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepare(&mut self.children[index])
        })) {
            Ok(result) => self.latch(result),
            Err(payload) => {
                // The child scope seals native custody; seal the router as well.
                self.terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn mock_preparation_v1() -> Self {
        let children = (7..=9)
            .map(|device| {
                let mut child = KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1();
                child.description.backend_device = device;
                child
            })
            .collect();
        Self::from_backends(children).unwrap()
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn with_retained_preparation_device_v1<R>(
        &mut self,
        backend_device: u64,
        prepare: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_default_dispatch_capacity_v1()?;
        if self.queue_retired {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "retained-device preparation requires a live backend",
            ));
        }
        if backend_device != self.description.backend_device || !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "retained-device preparation requires the exact native device",
            ));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let prepare = |device: &CheckedGfx942XnackMinusDevice| {
                if device.observation().unique_id() != backend_device {
                    Err("retained device does not match the backend description")
                } else {
                    Ok(prepare(device))
                }
            };
            match (&mut self.admitted_device, &mut self.queue) {
                (Some(device), None) => device
                    .with_retained_device_v1(prepare)
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(str::to_owned)),
                (None, Some(queue)) => queue
                    .with_retained_device_v1(prepare)
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(str::to_owned)),
                _ => Err("missing or duplicated retained-device owner".to_owned()),
            }
        }));
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(self.terminal_error(format!("KFD preparation scope: {error}"))),
            Err(payload) => {
                // Native scopes already poison their exact owner before unwinding.
                self.terminal = true;
                if let Some(queue) = self.queue.as_mut() {
                    queue.poison_after_runtime_owner_failure_v1();
                }
                std::panic::resume_unwind(payload)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_multi_routes_exact_child_without_changing_handles_or_custody() {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        for device in [9, 7, 8] {
            let actual = backend
                .with_preparation_child_v1(device, |child| Ok(child.description.backend_device))
                .unwrap();
            assert_eq!(actual, device);
        }
        backend.compute_xgmi_children[0] = Some(1);
        backend.compute_xgmi_children[1] = Some(1);
        assert_eq!(
            backend.with_preparation_child_v1(9, |_| Ok(42)).unwrap(),
            42
        );
        assert!(
            matches!(backend.with_preparation_child_v1::<()>(7, |_| panic!("occupied")),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert_eq!(backend.next_handle, 1);
        assert!(backend.streams.is_empty());
        for child in &backend.children {
            assert_eq!(child.next_handle, 1);
            assert!(child.queue.is_none());
            assert!(child.admitted_device.is_none());
        }
        backend.compute_xgmi_children.fill(None);
    }

    #[test]
    fn preparation_multi_rejects_unknown_synthetic_and_retired_without_callback() {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        for device in [7, 8, 9, 10] {
            assert!(matches!(
                backend.with_retained_preparation_device_v1(device, |_| panic!("rejected")),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            assert!(!backend.terminal);
        }
        backend.children[0].queue_retired = true;
        assert!(matches!(
            backend.with_retained_preparation_device_v1(7, |_| panic!("retired")),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(!backend.terminal);
    }

    #[test]
    fn preparation_multi_corrupt_routes_and_terminal_child_seal_root_before_callback() {
        for mode in 0..6 {
            let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
            match mode {
                0 => backend.terminal = true,
                1 => {
                    backend.device_children.insert(7, usize::MAX);
                }
                2 => {
                    backend.device_children.insert(7, 1);
                }
                3 => {
                    backend.compute_xgmi_children.pop();
                }
                4 => backend.children[0].terminal = true,
                _ => {
                    backend.children[0].terminal = true;
                    backend.compute_xgmi_children[0] = Some(1);
                }
            }
            assert!(matches!(
                backend.with_preparation_child_v1::<()>(7, |_| panic!("rejected")),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(backend.terminal);
            core::mem::forget(backend);
        }
    }

    #[test]
    fn preparation_multi_delegate_errors_and_unwind_preserve_failure_isolation() {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let result: Result<(), _> = backend.with_preparation_child_v1(7, |_| {
            Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "test",
            ))
        });
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))));
        assert!(!backend.terminal);
        assert_eq!(backend.with_preparation_child_v1(8, |_| Ok(8)).unwrap(), 8);
        let result: Result<(), _> = backend.with_preparation_child_v1(7, |child| {
            Err(child.terminal_error("test closing failure"))
        });
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
        assert!(backend.terminal);
        core::mem::forget(backend);

        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.with_preparation_child_v1::<()>(8, |child| {
                child.terminal = true;
                std::panic::panic_any(42u32)
            })
        }))
        .unwrap_err();
        assert_eq!(*panic.downcast::<u32>().unwrap(), 42);
        assert!(backend.terminal);
        assert!(!backend.children[0].terminal);
        assert!(backend.children[1].terminal);
        assert!(!backend.children[2].terminal);
        core::mem::forget(backend);
    }

    #[test]
    fn preparation_backend_rejects_terminal_retired_synthetic_and_foreign_before_callback() {
        for mode in 0..4 {
            let mut backend = KfdRuntimeBackendV1::mock();
            if mode == 0 {
                backend.terminal = true;
            }
            if mode == 1 {
                backend.queue_retired = true;
            }
            let device = if mode == 3 { 8 } else { 7 };
            let result = backend
                .with_retained_preparation_device_v1(device, |_| panic!("rejected callback"));
            if mode == 0 {
                assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
            } else {
                assert!(matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))));
            }
            assert!(backend.queue.is_none());
            assert!(backend.admitted_device.is_none());
            assert_eq!(backend.next_handle, 1);
            if mode == 0 {
                // Terminal backends must be retained even in a no-device fixture.
                core::mem::forget(backend);
            }
        }
    }
}
