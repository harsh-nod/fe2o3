use super::*;
use std::{
    alloc::Layout,
    cell::Cell,
    mem::{align_of, size_of_val},
};

pub(super) fn independent_callback<F>(_: &F) -> usize {
    let mut layout = Layout::from_size_align(0, 1).unwrap();
    for field in [
        Layout::new::<&Function>(),
        Layout::new::<ControlFlowLimits>(),
        Layout::new::<&mut Budget<'_>>(),
        Layout::new::<&mut Option<MeteredIndexedControlFlowV1>>(),
        Layout::new::<&mut Option<Error>>(),
        Layout::new::<&mut Option<Vec<(ValueId, Option<ValueId>)>>>(),
        Layout::new::<F>(),
    ] {
        layout = layout.extend(field).unwrap().0;
    }
    let layout = layout.pad_to_align();
    assert_eq!(
        size_of::<FunctionScopeInvocationV1<'_, '_, '_, F>>(),
        layout.size()
    );
    assert_eq!(
        align_of::<FunctionScopeInvocationV1<'_, '_, '_, F>>(),
        layout.align()
    );
    assert_eq!(
        size_of::<AssertUnwindSafe<FunctionScopeInvocationV1<'_, '_, '_, F>>>(),
        layout.size()
    );
    size_of::<F>() + 2 * layout.size()
}

fn independent_base() -> usize {
    size_of::<Option<MeteredIndexedControlFlowV1>>()
        + size_of::<Option<Error>>()
        + size_of::<Option<Vec<(ValueId, Option<ValueId>)>>>()
        + size_of::<FunctionControlFlowViewV1<'_, '_, '_>>()
        + size_of::<Result<(), Error>>()
        + size_of::<Result<Result<(), Error>, Box<dyn std::any::Any + Send>>>()
        + size_of::<FunctionControlFlowParameterV1>()
        + size_of::<Result<FunctionControlFlowParameterV1, Error>>()
        + size_of::<FunctionControlFlowParameterInputV1>()
        + size_of::<Result<FunctionControlFlowParameterInputV1, Error>>()
        + size_of::<&Function>()
        + size_of::<&MeteredIndexedControlFlowV1>()
        + 3 * size_of::<usize>()
}

#[repr(C, align(64))]
struct Capture<'a> {
    bytes: [u8; 1025],
    drops: &'a Cell<usize>,
}
impl Drop for Capture<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

fn captured<'a, 'function, 'work>(
    drops: &'a Cell<usize>,
    entered: &'a Cell<usize>,
    panic: bool,
) -> impl for<'scope> FnOnce(
    &mut FunctionControlFlowViewV1<'scope, 'function, 'work>,
) -> Result<(), Error>
+ 'a {
    let capture = Capture {
        bytes: [17; 1025],
        drops,
    };
    move |view| {
        assert_eq!(capture.bytes[1024], 17);
        entered.set(entered.get() + 1);
        assert!(view.is_reachable(BlockId(90))?);
        if panic {
            std::panic::panic_any(0x1357_2468u32);
        }
        drop(capture);
        Ok(())
    }
}

fn run_capture(
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), Error>, usize, usize, usize, usize) {
    let function = graph(false, false);
    let drops = Cell::new(0);
    let entered = Cell::new(0);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let callback = captured(&drops, &entered, false);
    let result =
        with_function_control_flow_v1(&function, Default::default(), &mut budget, callback);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(drops.get(), 1);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        entered.get(),
        drops.get(),
    )
}

#[test]
fn captured_callback_has_an_independent_padded_frame_and_exact_entry_boundary() {
    let function = graph(false, false);
    for short in [false, true] {
        let drops = Cell::new(0);
        let entered = Cell::new(0);
        let callback = captured(&drops, &entered, false);
        assert!(size_of_val(&callback) >= 1025);
        let extra = independent_callback(&callback);
        assert_eq!(function_scope_callback_header_v1(&callback).unwrap(), extra);
        assert_eq!(function_scope_header_v1().unwrap(), independent_base());
        let headers = independent_base() + extra;
        let storage_limit = FLOOR + headers - usize::from(short);
        // Entry work is exactly four. At the exact storage boundary, only the
        // CFG builder's next work charge may refuse; no callback is invoked.
        let mut work = Work::new(4);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let error =
            with_function_control_flow_v1(&function, Default::default(), &mut budget, callback)
                .unwrap_err();
        if short {
            assert!(matches!(error, Error::Resource(Resource::Storage(error))
                if error.actual() == FLOOR + headers && error.limit() == storage_limit));
            assert_eq!(budget.peak_storage(), FLOOR);
            assert_eq!(budget.failed_storage(), Some(FLOOR + headers));
            assert_eq!(budget.failed_work(), None);
        } else {
            assert!(matches!(error, Error::Resource(Resource::Work(error))
                if error.actual() == 5 && error.limit() == 4));
            assert_eq!(budget.peak_storage(), FLOOR + headers);
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(entered.get(), 0);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn moved_callback_full_scope_has_exact_and_one_short_work_and_storage() {
    let baseline = run_capture(LIMIT, LIMIT);
    assert_eq!(baseline.0, Ok(()));
    assert_eq!((baseline.3, baseline.4), (1, 1));
    assert_eq!(run_capture(baseline.1, baseline.2), baseline);
    let work_short = run_capture(baseline.1 - 1, baseline.2);
    assert!(matches!(
        work_short.0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    let storage_short = run_capture(baseline.1, baseline.2 - 1);
    assert!(matches!(
        storage_short.0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn moved_callback_unwind_preserves_payload_drop_and_preexisting_storage() {
    let function = graph(false, false);
    let drops = Cell::new(0);
    let entered = Cell::new(0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let callback = captured(&drops, &entered, true);
    let expected_headers = independent_base() + independent_callback(&callback);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        with_function_control_flow_v1(&function, Default::default(), &mut budget, callback)
    }));
    assert_eq!(
        *outcome.unwrap_err().downcast::<u32>().unwrap(),
        0x1357_2468
    );
    assert_eq!(entered.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.peak_storage() >= FLOOR + expected_headers);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.failed_work(), None);
}
