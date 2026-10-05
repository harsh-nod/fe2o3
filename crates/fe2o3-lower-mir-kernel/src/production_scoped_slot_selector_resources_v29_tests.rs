#[test]
fn symbolic_slot_actual_module_preserves_exact_and_one_short_resource_boundaries() {
    for case in [SlotSelectorCase::Mask, SlotSelectorCase::Guard] {
        let (result, work, peak) =
            run_slot_selector_module(case, SlotSelectorFault::None, MODULE_LIMIT, MODULE_LIMIT);
        slot_selector_assertions_completed(true);
        result.unwrap();
        assert!(work > 0 && peak > MODULE_FLOOR);
        let exact = run_slot_selector_module(case, SlotSelectorFault::None, work, peak);
        slot_selector_assertions_completed(true);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, peak));
        let short_work = run_slot_selector_module(case, SlotSelectorFault::None, work - 1, peak).0;
        slot_selector_assertions_completed(false);
        descriptor_resource_error(short_work.unwrap_err(), true);
        let short_storage =
            run_slot_selector_module(case, SlotSelectorFault::None, work, peak - 1).0;
        slot_selector_assertions_completed(false);
        descriptor_resource_error(short_storage.unwrap_err(), false);
    }
}
