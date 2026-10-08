#![cfg(test)]

use super::*;
use std::{cell::RefCell, rc::Rc};

struct Original {
    id: usize,
    drops: Rc<RefCell<Vec<usize>>>,
}

impl Drop for Original {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}

#[derive(Clone, Copy)]
enum Failure {
    None,
    Allocate,
    Write,
    WritePanic,
    ReturnInput,
    RetainInput,
    PromotionPanic,
}

struct Ops {
    failure: Failure,
    events: Vec<(&'static str, usize)>,
    drops: Rc<RefCell<Vec<usize>>>,
    retained: Option<Original>,
}

impl Operations for Ops {
    type Buffer = Original;
    type Data = Original;
    type Bridge = Original;
    type Error = &'static str;

    fn allocate(&mut self, bytes: usize) -> Result<Original, Self::Error> {
        self.events.push(("allocate", bytes));
        if matches!(self.failure, Failure::Allocate) {
            return Err("allocate");
        }
        Ok(Original {
            id: 17,
            drops: self.drops.clone(),
        })
    }

    fn write(&mut self, input: &mut Original, bytes: &[u8]) -> Result<(), Self::Error> {
        assert_eq!(input.id, 17);
        self.events.push(("write", bytes.len()));
        match self.failure {
            Failure::Write => Err("write"),
            Failure::WritePanic => panic!("original write panic"),
            _ => Ok(()),
        }
    }

    fn promote(
        &mut self,
        input: Original,
    ) -> Result<(Original, Original), (Self::Error, Option<Original>)> {
        self.events.push(("promote", input.id));
        match self.failure {
            Failure::ReturnInput => Err(("settled refusal", Some(input))),
            Failure::RetainInput | Failure::PromotionPanic => {
                self.retained = Some(input);
                if matches!(self.failure, Failure::PromotionPanic) {
                    panic!("original promotion panic");
                }
                Err(("terminal lower", None))
            }
            _ => Ok((
                input,
                Original {
                    id: 29,
                    drops: self.drops.clone(),
                },
            )),
        }
    }
}

fn fixture(failure: Failure) -> (Custody<Original, Original, Original>, Ops) {
    (
        Custody::empty(),
        Ops {
            failure,
            events: Vec::new(),
            drops: Rc::new(RefCell::new(Vec::new())),
            retained: None,
        },
    )
}

#[test]
fn exact_extent_and_original_bridge_survive_binding_until_actual_data_release() {
    let (mut root, mut ops) = fixture(Failure::None);
    root.prepare(&mut ops, &[5; 260]).unwrap();
    assert_eq!(
        ops.events,
        [("allocate", 260), ("write", 260), ("promote", 17)]
    );
    assert!(matches!(&root, Custody::Output(data, bridge) if data.id == 17 && bridge.id == 29));
    assert!(ops.drops.borrow().is_empty());
    let data = root.take_data_for_binding();
    assert!(matches!(&root, Custody::Bound(bridge) if bridge.id == 29));
    assert!(!root.is_disposed_or_unentered());
    assert!(ops.drops.borrow().is_empty());
    drop(data);
    assert_eq!(*ops.drops.borrow(), [17]);
    root.dispose_after_data_release();
    assert!(root.is_disposed_or_unentered());
    assert_eq!(*ops.drops.borrow(), [17, 29]);
}

#[test]
fn allocation_refusal_never_reaches_write_or_promotion() {
    let (mut root, mut ops) = fixture(Failure::Allocate);
    assert_eq!(root.prepare(&mut ops, &[1; 4]), Err("allocate"));
    assert!(matches!(root, Custody::Empty));
    assert_eq!(ops.events, [("allocate", 4)]);
}

#[test]
fn write_refusal_keeps_original_input_rooted() {
    let (mut root, mut ops) = fixture(Failure::Write);
    assert_eq!(root.prepare(&mut ops, &[1; 4]), Err("write"));
    assert!(matches!(&root, Custody::Input(input) if input.id == 17));
    assert_eq!(ops.events, [("allocate", 4), ("write", 4)]);
    assert!(ops.drops.borrow().is_empty());
}

#[test]
fn write_unwind_does_not_drop_the_rooted_input() {
    let (mut root, mut ops) = fixture(Failure::WritePanic);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = root.prepare(&mut ops, &[1; 4]);
        }))
        .is_err()
    );
    assert!(matches!(&root, Custody::Input(input) if input.id == 17));
    assert!(ops.drops.borrow().is_empty());
}

#[test]
fn settled_promotion_refusal_restores_exact_original_without_retry() {
    let (mut root, mut ops) = fixture(Failure::ReturnInput);
    assert_eq!(root.prepare(&mut ops, &[1; 4]), Err("settled refusal"));
    assert!(matches!(&root, Custody::Input(input) if input.id == 17));
    assert!(ops.drops.borrow().is_empty());
}

#[test]
fn terminal_promotion_error_keeps_original_in_lower_custody() {
    let (mut root, mut ops) = fixture(Failure::RetainInput);
    assert_eq!(root.prepare(&mut ops, &[1; 4]), Err("terminal lower"));
    assert!(matches!(root, Custody::Promoting));
    assert_eq!(ops.retained.as_ref().unwrap().id, 17);
    assert!(!root.is_disposed_or_unentered());
    assert!(ops.drops.borrow().is_empty());
}

#[test]
fn promotion_unwind_preserves_lower_input_and_nonretired_sidecar() {
    let (mut root, mut ops) = fixture(Failure::PromotionPanic);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = root.prepare(&mut ops, &[1; 4]);
        }))
        .is_err()
    );
    assert!(matches!(root, Custody::Promoting));
    assert_eq!(ops.retained.as_ref().unwrap().id, 17);
    assert!(!root.is_disposed_or_unentered());
    assert!(ops.drops.borrow().is_empty());
}

#[test]
fn synthetic_backend_refuses_opt_in_without_affecting_default_readiness() {
    let backend = KfdRuntimeBackendV1::mock();
    assert!(backend.generated_lane_ready_v1().unwrap());
    assert!(matches!(
        backend.generated_sdma_lane_ready_v1(),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
}

#[test]
fn opt_in_waits_for_primary_even_when_auxiliary_lane_is_free() {
    let mut backend = KfdRuntimeBackendV1::mock();
    // Readiness-only control. No original queue or storage is fabricated.
    backend.native_available = true;
    assert!(backend.generated_sdma_lane_ready_v1().unwrap());
    backend.stream_compute_lanes.insert(91, 0);
    assert!(backend.generated_lane_ready_v1().unwrap());
    assert!(!backend.generated_sdma_lane_ready_v1().unwrap());
    backend.stream_compute_lanes.clear();
}

#[test]
fn initialized_content_descriptor_is_exact_and_not_an_output_receipt() {
    let bytes = [7; 260];
    let first = content([1; 32], &bytes).unwrap();
    assert_eq!(first.byte_len(), 260);
    assert_ne!(first, content([2; 32], &bytes).unwrap());
    assert_ne!(first, content([1; 32], &[6; 260]).unwrap());
    assert!(content([0; 32], &bytes).is_err());
    assert!(content([1; 32], &[]).is_err());
}
