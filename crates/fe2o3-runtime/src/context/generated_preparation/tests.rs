use fe2o3_runtime_model::*;
use std::cell::Cell;

use super::*;

mod shell_tests;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    // Descriptive IDs for backend metadata tests, never native device authority.
    pub(crate) fn generated_shell_test_binding_v1(
        backend: &mut KfdRuntimeBackendV1,
    ) -> (
        crate::kfd_backend::GeneratedShellBindingV1,
        Vec<RuntimeAllocationIdV1>,
    ) {
        (
            crate::kfd_backend::GeneratedShellBindingV1 {
                context_generation: 1,
                device: RuntimeDeviceIdV1::new(1, 1),
                stream: RuntimeStreamIdV1::new(1, 2),
                hold: 3,
                backend_device: 7,
                backend_stream: backend.create_stream_v1(7).unwrap(),
                native_device: admission(1, 1).1,
            },
            (4..7).map(|id| RuntimeAllocationIdV1::new(1, id)).collect(),
        )
    }
}

struct StagedReadback(std::rc::Rc<Cell<usize>>);
impl Drop for StagedReadback {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct StagedCarrier {
    installed: Option<StagedReadback>,
    install_calls: usize,
    panic_after_install: bool,
}

impl crate::RuntimeGfx942GeneratedCarrierV1 for StagedCarrier {
    type CurrentnessError = ();
    type Readback = StagedReadback;
    fn source(&self) -> crate::RuntimeGfx942GeneratedSourceV1<'_, ()> {
        panic!("this transaction fixture has no Worker/native authority")
    }
    fn prepare_readback(&self) -> Result<Self::Readback, crate::RuntimeGfx942ReadbackErrorV1> {
        panic!("test checked-stage callback supplies the destination")
    }
    fn install_readback(&mut self, readback: Self::Readback) {
        self.install_calls += 1;
        self.installed = Some(readback);
        assert!(
            !self.panic_after_install,
            "install panic after custody transfer"
        );
    }
}

fn staged_roster() -> crate::generated_source::GeneratedHostRosterV1 {
    crate::generated_source::GeneratedHostRosterV1 {
        source_identity: std::sync::Arc::new(()),
        buffers: [None; fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1],
        count: 0,
        readback_bytes: 0,
        fixup_count: 0,
        dispatch_contract_sha256: [0; 32],
    }
}

#[test]
fn generated_reservation_checked_stage_error_disposes_destination_without_install() {
    let drops = std::rc::Rc::new(Cell::new(0));
    let mut carrier = StagedCarrier {
        installed: None,
        install_calls: 0,
        panic_after_install: false,
    };
    let result = install_checked_readback(&mut carrier, |_| {
        let _staged = StagedReadback(drops.clone());
        // Models the existing native scope rejecting its closing check, after
        // the callback returned staged storage. No device token is fabricated.
        Err(crate::RuntimeGfx942GeneratedReservationErrorV1::AuthorityNotCurrent)
    });
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(carrier.install_calls, 0);
    assert!(carrier.installed.is_none());
    install_checked_readback(&mut carrier, |_| {
        Ok((staged_roster(), StagedReadback(drops.clone())))
    })
    .unwrap();
    assert_eq!(carrier.install_calls, 1);
    assert_eq!(drops.get(), 1);
    drop(carrier);
    assert_eq!(drops.get(), 2);
}

#[test]
fn generated_reservation_install_panic_retains_transferred_storage_in_original_carrier() {
    let drops = std::rc::Rc::new(Cell::new(0));
    let mut carrier = StagedCarrier {
        installed: None,
        install_calls: 0,
        panic_after_install: true,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        install_checked_readback(&mut carrier, |_| {
            Ok((staged_roster(), StagedReadback(drops.clone())))
        })
    }));
    assert!(result.is_err());
    assert!(carrier.installed.is_some());
    assert_eq!(drops.get(), 0);
    drop(carrier);
    assert_eq!(drops.get(), 1);
}

#[test]
fn generated_reservation_context_rejections_do_not_call_source_or_install() {
    let mut foreign = context();
    let mut context = context();
    let mut prepared = bound(
        &context,
        StagedCarrier {
            installed: None,
            install_calls: 0,
            panic_after_install: false,
        },
    );
    assert!(foreign.reserve_gfx942_prepared_v1(&mut prepared).is_err());
    assert!(context.reserve_gfx942_prepared_v1(&mut prepared).is_err());
    assert_eq!(prepared.value.install_calls, 0);
    assert!(prepared.value.installed.is_none());
}

// Model-only identity fixtures never construct a native checked device.
fn admission(domain_seed: u8, generation: u64) -> (DeviceIdentityStateV1, ModelDeviceAdmissionV1) {
    let digest = |seed| IdentityDigestV1::from_untrusted_bytes([seed; 32]);
    let domain = DeviceObservationDomainIdV1::from_untrusted_digest(digest(domain_seed));
    let profile = DeviceAdmissionProfileV1::gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        DeviceAdmissionProfileIdV1::from_untrusted_digest(digest(2)),
        digest(3),
        digest(4),
    );
    let epoch = ObservationEpochV1(1);
    let pci = PciAddressV1 {
        domain: 0,
        bus: 4,
        device: 1,
        function: 0,
    };
    let inventory = UntrustedDeviceInventoryV1::from_untrusted_observations(
        UntrustedKfdObservationV1 {
            domain_id: domain,
            epoch,
            node: DeviceNodeV1 {
                major: 511,
                minor: KFD_DEVICE_MINOR_V1,
            },
            uapi_major: KFD_UAPI_MAJOR_V1,
            uapi_minor: KFD_UAPI_MINOR_V1,
            schema_identity: digest(3),
            xnack: XnackObservationV1::Disabled,
        },
        vec![UntrustedTopologyObservationV1 {
            domain_id: domain,
            epoch,
            topology_node_id: 1,
            kfd_gpu_id: 2,
            gpu_unique_id: 7,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            target: GpuTargetObservationV1::Gfx942,
            compute_partition: ComputePartitionObservationV1::Spx,
            memory_partition: MemoryPartitionObservationV1::Nps1,
        }],
        vec![UntrustedRenderObservationV1 {
            domain_id: domain,
            epoch,
            node: DeviceNodeV1 {
                major: DRM_DEVICE_MAJOR_V1,
                minor: DRM_RENDER_MIN_MINOR_V1,
            },
            gpu_unique_id: 7,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: 1,
            drm_schema_identity: digest(4),
            driver_name: DrmDriverNameObservationV1::Amdgpu,
            drm_major: DRM_DRIVER_MAJOR_V1,
            drm_minor: DRM_DRIVER_MINOR_V1,
            drm_patch: DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: DrmFamilyObservationV1::AmdgpuFamilyAi,
        }],
    )
    .unwrap();
    DeviceIdentityStateV1::new(domain)
        .register_device_model_only(
            inventory.correlate_model_only(&profile).unwrap(),
            DeviceGenerationV1(generation),
        )
        .unwrap()
}

fn context() -> RuntimeContextV1<KfdRuntimeBackendV1> {
    RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap()
}

fn bound<T>(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    value: T,
) -> RuntimeGfx942PreparedV1<T> {
    RuntimeGfx942PreparedV1 {
        value,
        binding: PreparationBindingV1 {
            context_generation: context.context_generation,
            device: context.devices()[0].id(),
            backend_device: 7,
            native_device: admission(1, 1).1,
        },
        owner_local: PhantomData,
    }
}

#[test]
fn preparation_binding_rejects_each_identity_coordinate() {
    let context = context();
    let prepared = bound(&context, ());
    let binding = prepared.binding;
    let id = binding.device;
    assert!(binding.matches_context(context.context_generation, id, 7));
    assert!(!binding.matches_context(context.context_generation + 1, id, 7));
    assert!(!binding.matches_context(
        context.context_generation,
        RuntimeDeviceIdV1::new(id.context_generation, id.local + 1),
        7
    ));
    assert!(!binding.matches_context(
        context.context_generation,
        RuntimeDeviceIdV1::new(id.context_generation + 1, id.local),
        7
    ));
    assert!(!binding.matches_context(context.context_generation, id, 8));
    assert!(binding.matches_native(admission(1, 1).1));
    assert!(!binding.matches_native(admission(1, 2).1));
    assert!(!binding.matches_native(admission(2, 1).1));
}

#[test]
fn preparation_binding_survives_model_vm_creation_without_readmitting_device() {
    let context = context();
    let prepared = bound(&context, Rc::new(7));
    let (identity, device) = admission(1, 1);
    let correlation = device.correlation();
    let (identity, vm) = identity
        .register_vm_model_only(
            device,
            UntrustedVmObservationV1 {
                domain_id: device.domain_id(),
                device: device.model_key(),
                vm_id: VmIdV1(1),
                kfd_gpu_id: correlation.kfd_gpu_id(),
                render_node: correlation.render_node(),
                pci: correlation.identity().pci,
            },
        )
        .unwrap();
    assert_eq!(identity.devices().len(), 1);
    assert_eq!(vm.model_key().device, device.model_key());
    for _ in 0..3 {
        assert!(prepared.binding.matches_native(device));
        assert_eq!(**prepared.value(), 7);
    }
}

#[test]
fn preparation_rejects_foreign_unknown_terminal_and_synthetic_before_callback() {
    let mut context = context();
    let device = context.devices()[0].id();
    let called = Cell::new(false);
    for id in [
        device,
        RuntimeDeviceIdV1::new(device.context_generation + 1, device.local),
        RuntimeDeviceIdV1::new(device.context_generation, device.local + 1),
    ] {
        assert!(
            context
                .with_gfx942_preparation_device_v1(id, |_| {
                    called.set(true);
                    Ok::<_, ()>(())
                })
                .is_err()
        );
    }
    assert!(!context.is_terminal());
    context.quarantine_after_async_command_panic_v1();
    assert!(matches!(
        context.with_gfx942_preparation_device_v1(device, |_| {
            called.set(true);
            Ok::<_, ()>(())
        }),
        Err(RuntimeGfx942PreparationErrorV1::Context(
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextTerminal)
        ))
    ));
    assert!(!called.get());
}

#[test]
fn preparation_validation_rejects_foreign_context_and_binding_before_native_access() {
    let first = context();
    let mut second = context();
    let mut prepared = bound(&first, ());
    assert!(matches!(
        second.validate_gfx942_prepared_v1(&prepared),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownDevice
        ))
    ));
    prepared.binding.device = second.devices()[0].id();
    assert!(matches!(
        second.validate_gfx942_prepared_v1(&prepared),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    prepared.binding.context_generation = second.context_generation;
    prepared.binding.backend_device = 8;
    assert!(matches!(
        second.validate_gfx942_prepared_v1(&prepared),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
}

#[test]
fn preparation_outliving_shutdown_is_inert_and_rejected() {
    let mut context = context();
    let observer = Rc::new(());
    let prepared = bound(&context, Rc::clone(&observer));
    assert!(context.cleanup().is_complete());
    context.shutdown_owned_backend_v1().unwrap();
    assert!(!context.is_terminal());
    assert!(context.validate_gfx942_prepared_v1(&prepared).is_err());
    assert_eq!(Rc::strong_count(&observer), 2);
    drop(context);
    assert_eq!(Rc::strong_count(&observer), 2);
    drop(prepared);
    assert_eq!(Rc::strong_count(&observer), 1);
}
