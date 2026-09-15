use super::*;

#[test]
fn helper_working_scope_transfers_only_retained_cells_and_keeps_peak() {
    let mut parent = WorkingCells::new(100);
    parent.reserve(11).unwrap();
    let mut work = 0;
    let mut scope = WorkingScopeV1::new(&mut parent, &mut work).unwrap();
    scope.cells().reserve(17).unwrap();
    scope.cells().release(10);
    assert_eq!(scope.finish(19_u32, 7).unwrap(), 19);
    assert_eq!(parent.live, 18);
    assert_eq!(parent.peak, 11 + HEADER_CELLS + 17);
    assert_eq!(parent.limit, 100);
    assert_eq!(work, HEADER_CELLS);
}

#[test]
fn helper_working_scope_nested_failure_preserves_suspended_parent_owners() {
    let mut parent = WorkingCells::new(100);
    parent.reserve(11).unwrap();
    let mut work = 0;
    let mut outer = WorkingScopeV1::new(&mut parent, &mut work).unwrap();
    outer.cells().reserve(3).unwrap();
    let outer_live = outer.cells().live;
    {
        let mut inner = WorkingScopeV1::new(outer.cells(), &mut work).unwrap();
        inner.cells().reserve(5).unwrap();
        assert!(inner.cells().reserve(100).is_err());
    }
    assert_eq!(outer.cells().live, outer_live);
    assert_eq!(outer.cells().peak, 11 + 2 * HEADER_CELLS + 8);
    outer.finish((), 3).unwrap();
    assert_eq!(parent.live, 14);
    assert_eq!(work, 2 * HEADER_CELLS);
}

#[test]
fn helper_working_scope_nested_success_keeps_child_output_until_released() {
    let mut parent = WorkingCells::new(100);
    let mut work = 0;
    let mut outer = WorkingScopeV1::new(&mut parent, &mut work).unwrap();
    let mut inner = WorkingScopeV1::new(outer.cells(), &mut work).unwrap();
    inner.cells().reserve(5).unwrap();
    inner.finish((), 5).unwrap();
    assert_eq!(outer.cells().live, HEADER_CELLS + 5);
    outer.cells().release(5);
    outer.finish((), 0).unwrap();
    assert_eq!(parent.live, 0);
    assert_eq!(parent.peak, 2 * HEADER_CELLS + 5);
}

#[test]
fn helper_working_scope_inherited_limit_and_header_are_not_reset() {
    let mut parent = WorkingCells::new(17 + HEADER_CELLS);
    parent.reserve(17).unwrap();
    let mut work = 0;
    {
        let mut scope = WorkingScopeV1::new(&mut parent, &mut work).unwrap();
        let cells = scope.cells();
        assert_eq!(cells.live, cells.limit);
        assert!(cells.reserve(1).is_err());
    }
    assert_eq!(parent.live, 17);
    assert_eq!(parent.peak, parent.limit);
    let mut short = WorkingCells::new(17 + HEADER_CELLS - 1);
    short.reserve(17).unwrap();
    assert!(WorkingScopeV1::new(&mut short, &mut work).is_err());
    assert_eq!(short.live, 17);
    assert_eq!(short.peak, 17);
    assert_eq!(work, 2 * HEADER_CELLS);
}

#[test]
fn helper_working_scope_cleanup_never_refunds_shared_work() {
    let mut parent = WorkingCells::new(100);
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - HEADER_CELLS;
    drop(WorkingScopeV1::new(&mut parent, &mut work).unwrap());
    assert_eq!(parent.live, 0);
    assert_eq!(work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
    assert!(WorkingScopeV1::new(&mut parent, &mut work).is_err());
    assert_eq!(parent.live, 0);
    assert!(work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
}

#[test]
fn helper_working_scope_mismatched_return_drops_value_and_releases_temporaries() {
    struct Output(std::rc::Rc<std::cell::Cell<bool>>);
    impl Drop for Output {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for retained in [4, 6, usize::MAX] {
        let mut parent = WorkingCells::new(100);
        parent.reserve(3).unwrap();
        let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut scope = WorkingScopeV1::new(&mut parent, &mut 0).unwrap();
        scope.cells().reserve(5).unwrap();
        assert!(matches!(
            scope.finish(Output(dropped.clone()), retained),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "helper range scope still owns temporary storage"
            ))
        ));
        assert!(dropped.get());
        assert_eq!(parent.live, 3);
        assert_eq!(parent.peak, 3 + HEADER_CELLS + 5);
    }
}
