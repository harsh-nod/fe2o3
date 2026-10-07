//! Captured code and descriptive authority fixtures; no device/native operation.

use super::*;
use crate::*;
use std::cell::Cell;
use std::rc::Rc;

mod arena1024;
mod registry16;

const IMAGE: &[u8] = include_bytes!("../../../tests/fixtures/native-fill-worker/kernel.hsaco");

struct Authority {
    object: [u8; 32],
    length: u64,
    contract: [u8; 32],
    binding: Gfx942RuntimeInvocationBindingV1,
    stale: Rc<Cell<bool>>,
}

// SAFETY: This fixture only borrows/validates inert source bytes; it never calls
// Context/native construction or publishes a packet.
#[allow(unsafe_code)]
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for Authority {
    type CurrentnessError = ();
    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.object
    }
    fn finalized_hsaco_length(&self) -> u64 {
        self.length
    }
    fn kernel_name(&self) -> &str {
        "fill_write_only"
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.contract
    }
    fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        self.binding
    }
    fn device_unique_id(&self) -> u64 {
        42
    }
    fn revalidate_currentness(&self) -> Result<(), ()> {
        if self.stale.get() { Err(()) } else { Ok(()) }
    }
}

struct Carrier {
    storage: GeneratedGfx942PersistentStorageV1,
    authority: Authority,
}

impl RuntimeGfx942GeneratedCarrierV1 for Carrier {
    type CurrentnessError = ();
    type Readback = ();
    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, ()> {
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            &self.storage,
            IMAGE,
            &self.authority,
        )
    }
    fn source_mut(&mut self) -> Option<RuntimeGfx942GeneratedSourceMutV1<'_, ()>> {
        Some(RuntimeGfx942GeneratedSourceMutV1::new(
            &mut self.storage,
            IMAGE,
            &self.authority,
        ))
    }
    fn prepare_readback(&self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        Ok(())
    }
    fn install_readback(&mut self, _: ()) {}
}

struct Borrowed<'a>(&'a Carrier);
impl RuntimeGfx942GeneratedCarrierV1 for Borrowed<'_> {
    type CurrentnessError = ();
    type Readback = ();
    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, ()> {
        self.0.source()
    }
    fn prepare_readback(&self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        Ok(())
    }
    fn install_readback(&mut self, _: ()) {}
}

fn carrier(index: usize) -> Carrier {
    let count = 1 + index as u64 * 64;
    let grid = (index as u32 + 1) * 64;
    let mut explicit = vec![0; 16];
    explicit[8..].copy_from_slice(&count.to_le_bytes());
    let prepared = prepare_gfx942_runtime_dispatch_v1(
        IMAGE,
        "fill_write_only",
        Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            vec![
                Gfx942RuntimeDispatchBufferV1::new(
                    vec![0; count as usize * 4],
                    Gfx942RuntimeBufferAccessV1::WriteOnly,
                )
                .unwrap(),
            ],
            vec![fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            fe2o3_aql::AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]).unwrap(),
            0,
            100,
        ),
    )
    .unwrap();
    let projection = prepared
        .into_native_conditional_fill64_projection_v1(
            IMAGE,
            fe2o3_kfd::NativeConditionalFill64PremisesV1::new(
                [0x70 + index as u8; 32],
                count,
                grid,
            )
            .unwrap(),
        )
        .unwrap();
    let authority = Authority {
        object: projection.identity().object_sha256(),
        length: projection.finalized_hsaco_length(),
        contract: projection.dispatch_contract_sha256(),
        binding: projection.invocation_binding(),
        stale: Rc::new(Cell::new(false)),
    };
    Carrier {
        storage: projection.into_generated_storage_v1(),
        authority,
    }
}

fn cohort() -> RuntimeGfx942GeneratedCohort3V1<Carrier> {
    RuntimeGfx942GeneratedCohort3V1::new(std::array::from_fn(carrier))
}

#[test]
fn registry4_original_roster_rejects_scalar_cohort_duplicate_and_reordered_identities() {
    let registry = RuntimeGfx942GeneratedRegistry4V1::new(std::array::from_fn(carrier));
    let roster = registry.validate_sources(42).unwrap();
    assert_eq!(
        roster.source_identity.profile(),
        GeneratedProfileV1::NativeFillRegistry4
    );
    assert_eq!(
        (roster.count, roster.readback_bytes, roster.fixup_count),
        (4, 1552, 4)
    );
    assert!(!roster.matches(&cohort().validate_sources(42).unwrap()));
    for member in &registry.members {
        assert!(!roster.matches(&member.source().validate(42).unwrap()));
    }
    let [a, b, c, d] = registry.members.each_ref();
    let reordered = RuntimeGfx942GeneratedRegistry4V1::new([
        Borrowed(d),
        Borrowed(b),
        Borrowed(c),
        Borrowed(a),
    ]);
    assert!(!reordered.validate_sources(42).unwrap().matches(&roster));
    let duplicated = RuntimeGfx942GeneratedRegistry4V1::new([
        Borrowed(a),
        Borrowed(b),
        Borrowed(c),
        Borrowed(a),
    ]);
    assert!(duplicated.validate_sources(42).is_err());
}

#[test]
fn registry_repeat2_profile_never_matches_the_same_single_use_originals() {
    let mut original = RuntimeGfx942GeneratedRegistry4V1::new(std::array::from_fn(carrier));
    let once = original.validate_sources(42).unwrap();
    // This changes an inert private fixture tag, not execution or completion authority.
    original.repeat2 = true;
    let repeated = original.validate_sources(42).unwrap();
    assert_eq!(
        repeated.source_identity.profile(),
        GeneratedProfileV1::NativeFillRegistry4Repeat2
    );
    assert!(!repeated.matches(&once));
    assert!(!once.matches(&repeated));
    assert_ne!(
        once.dispatch_contract_sha256,
        repeated.dispatch_contract_sha256
    );
    assert!(original.source_mut_v1().unwrap().matches_roster(&repeated));
    assert!(!original.source_mut_v1().unwrap().matches_roster(&once));
    assert_eq!(
        (once.count, once.readback_bytes),
        (repeated.count, repeated.readback_bytes)
    );
}

#[test]
fn registry4_nested_native_inputs_preserve_all_four_original_borrows_and_closing_checks() {
    let registry = RuntimeGfx942GeneratedRegistry4V1::new(std::array::from_fn(carrier));
    let roster = registry.validate_sources(42).unwrap();
    let pointers = registry
        .members
        .each_ref()
        .map(|member| member.storage.buffers()[0].bytes().as_ptr());
    let called = Cell::new(0);
    registry
        .with_native_inputs_v1(42, &roster, |programs, buffers| {
            assert_eq!(programs.len(), 4);
            for (index, buffer) in buffers.iter().enumerate() {
                assert_eq!(buffer[0].bytes().as_ptr(), pointers[index]);
            }
            called.set(called.get() + 1);
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(called.get(), 1);
    let stale = registry.members[3].authority.stale.clone();
    assert!(
        registry
            .with_native_inputs_v1(42, &roster, |_, _| {
                stale.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(
        registry
            .with_native_inputs_v1(42, &roster, |_, _| panic!("stale callback"))
            .is_err()
    );
}

#[test]
fn registry4_control_transfer_requires_the_exact_ordered_roster() {
    let mut registry = RuntimeGfx942GeneratedRegistry4V1::new(std::array::from_fn(carrier));
    let expected = registry.validate_sources(42).unwrap();
    let foreign = RuntimeGfx942GeneratedRegistry4V1::new(std::array::from_fn(carrier))
        .validate_sources(42)
        .unwrap();
    let mut source = registry.source_mut_v1().unwrap();
    assert!(!source.matches_roster(&foreign));
    assert!(source.matches_roster(&expected));
    let mut controls = [None, None, None, None];
    assert!(source.transfer_controls_into(&mut controls));
    assert!(controls.iter().all(Option::is_some));
    assert!(!source.matches_roster(&expected));
    assert!(!source.transfer_controls_into(&mut controls));
}

#[test]
fn cohort3_retains_exact_three_source_contracts_and_order_without_scalar_alias() {
    let cohort = cohort();
    let roster = cohort.validate_sources(42).unwrap();
    assert_eq!(
        roster.source_identity.profile(),
        GeneratedProfileV1::NativeFillCohort3
    );
    assert_eq!(
        (roster.count, roster.readback_bytes, roster.fixup_count),
        (3, 780, 3)
    );
    let GeneratedContractsV1::Cohort3(contracts) = roster.dispatch_contract_sha256 else {
        panic!()
    };
    for (index, member) in cohort.members.iter().enumerate() {
        let original = member.source().validate(42).unwrap();
        assert!(!roster.source_identity.matches(&original.source_identity));
        assert_eq!(contracts[index], member.authority.contract);
        assert_eq!(roster.buffers[index].unwrap().ordinal, index);
        assert_eq!(
            roster.buffers[index].unwrap().bytes,
            (1 + index as u64 * 64) * 4
        );
        assert!(member.storage.control_available());
    }
    let [a, b, c] = cohort.members.each_ref();
    let reordered = RuntimeGfx942GeneratedCohort3V1::new([Borrowed(b), Borrowed(a), Borrowed(c)]);
    assert!(!reordered.validate_sources(42).unwrap().matches(&roster));
    let duplicated = RuntimeGfx942GeneratedCohort3V1::new([Borrowed(a), Borrowed(a), Borrowed(c)]);
    assert!(matches!(
        duplicated.validate_sources(42),
        Err(RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster)
    ));
}

#[test]
fn cohort3_nested_inputs_borrow_all_originals_and_return_callback_result_unchanged() {
    let cohort = cohort();
    let roster = cohort.validate_sources(42).unwrap();
    let mut called = 0;
    let result = cohort
        .with_native_inputs_v1(42, &roster, |programs, buffers| {
            called += 1;
            for index in 0..3 {
                assert_eq!(programs[index].envelope().bytes().as_ptr(), IMAGE.as_ptr());
                assert_eq!(buffers[index].len(), 1);
                assert_eq!(
                    buffers[index][0].bytes().as_ptr(),
                    cohort.members[index].storage.buffers()[0].bytes().as_ptr()
                );
            }
            Ok(())
        })
        .unwrap();
    assert!(result.is_ok());
    assert_eq!(called, 1);
    assert!(cohort.validate_sources(42).unwrap().matches(&roster));
    assert!(cohort.members.iter().all(|m| m.storage.control_available()));
}

#[test]
fn cohort3_every_original_currentness_is_checked_before_and_after_callback() {
    for index in 0..3 {
        let cohort = cohort();
        let roster = cohort.validate_sources(42).unwrap();
        cohort.members[index].authority.stale.set(true);
        assert!(
            cohort
                .with_native_inputs_v1(42, &roster, |_, _| panic!("stale entry"))
                .is_err()
        );
        cohort.members[index].authority.stale.set(false);
        let mut called = false;
        assert!(
            cohort
                .with_native_inputs_v1(42, &roster, |_, _| {
                    called = true;
                    cohort.members[index].authority.stale.set(true);
                    Ok(())
                })
                .is_err()
        );
        assert!(called);
        assert!(cohort.members.iter().all(|m| m.storage.control_available()));
    }
}

#[test]
fn cohort3_wrong_original_roster_never_enters_native_input_callback() {
    for axis in 0..5 {
        let cohort = cohort();
        let mut roster = cohort.validate_sources(42).unwrap();
        match axis {
            0 => roster.source_identity = Arc::new(()).into(),
            1 => roster.count = 2,
            2 => roster.buffers[1].as_mut().unwrap().bytes += 4,
            3 => roster.dispatch_contract_sha256 = GeneratedContractsV1::Singleton([1; 32]),
            _ => roster.fixup_count = 1,
        }
        assert!(
            cohort
                .with_native_inputs_v1(42, &roster, |_, _| panic!("wrong roster"))
                .is_err()
        );
        assert!(cohort.members.iter().all(|m| m.storage.control_available()));
    }
}

#[test]
fn cohort3_currentness_wrapper_preserves_callback_error_and_all_source_owners() {
    let cohort = cohort();
    let roster = cohort.validate_sources(42).unwrap();
    assert_eq!(
        cohort
            .with_current_sources_v1(42, &roster, || Err::<(), _>(17))
            .unwrap(),
        Err(17)
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = cohort.with_native_inputs_v1(42, &roster, |_, _| panic!("injected callback"));
    }));
    assert!(panic.is_err());
    assert!(cohort.validate_sources(42).unwrap().matches(&roster));
    assert!(cohort.members.iter().all(|m| m.storage.control_available()));
}

#[test]
fn cohort3_mutable_control_transfer_is_exact_once_and_roots_all_three_originals() {
    let mut cohort = cohort();
    let roster = cohort.validate_sources(42).unwrap();
    let mut destination = [None, None, None];
    {
        let mut source = cohort.source_mut_v1().unwrap();
        assert!(source.validate(42).unwrap().matches(&roster));
        assert!(source.matches_roster(&roster));
        let mut wrong = roster.clone();
        wrong.dispatch_contract_sha256 = GeneratedContractsV1::Singleton([7; 32]);
        assert!(!source.matches_roster(&wrong));
        assert!(source.transfer_controls_into(&mut destination));
        assert!(destination.iter().all(Option::is_some));
        assert!(!source.matches_roster(&roster));
        assert!(!source.transfer_controls_into(&mut [None, None, None]));
    }
    assert!(
        cohort
            .members
            .iter()
            .all(|m| !m.storage.control_available())
    );
    assert!(cohort.validate_sources(42).unwrap().matches(&roster));
    let mut other = self::cohort();
    {
        let mut source = other.source_mut_v1().unwrap();
        assert!(!source.transfer_controls_into(&mut destination));
    }
    assert!(other.members.iter().all(|m| m.storage.control_available()));
}
