// Adversarial intermediate emission for downstream source-history tests only.
// Production builds contain neither this module nor its injection call.
use super::*;

type Site = (u32, u32, Option<u32>, u32);
thread_local! {
    static PENDING: std::cell::RefCell<Option<Vec<(Site, usize)>>> = const { std::cell::RefCell::new(None) };
}

struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        PENDING.with_borrow_mut(|pending| *pending = None);
    }
}

pub(super) fn with_exact_sites<T>(sites: &[Site], build: impl FnOnce() -> T) -> T {
    with_exact_replays(sites, 1, build)
}

pub(super) fn with_exact_replays<T>(sites: &[Site], visits: usize, build: impl FnOnce() -> T) -> T {
    assert!(visits > 0);
    assert!(
        PENDING.with_borrow(Option::is_none),
        "nested load fault injection"
    );
    if sites.is_empty() {
        return build();
    }
    let mut ordered = sites.to_vec();
    ordered.sort_unstable();
    assert!(!ordered.windows(2).any(|pair| pair[0] == pair[1]));
    PENDING.with_borrow_mut(|pending| {
        *pending = Some(ordered.into_iter().map(|site| (site, visits)).collect())
    });
    let reset = Reset;
    let output = build();
    assert!(
        PENDING.with_borrow(|pending| pending.as_ref().unwrap().is_empty()),
        "every injected site must fail the normal initialization check exactly the requested number of times"
    );
    drop(reset);
    output
}

pub(super) fn inject_failed_direct_load(
    result: Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Err(ProductionSemanticKirErrorV1::MissingLocalDefinition {
        function,
        block,
        statement,
        local,
    }) = &result
    else {
        return result;
    };
    let site = (*function, *block, *statement, *local);
    let injected = PENDING.with_borrow_mut(|pending| {
        let Some(sites) = pending else { return false };
        let Ok(index) = sites.binary_search_by_key(&site, |row| row.0) else {
            return false;
        };
        sites[index].1 -= 1;
        if sites[index].1 == 0 {
            sites.remove(index);
        }
        true
    });
    if injected { Ok(()) } else { result }
}

#[test]
fn load_faults_are_exact_once_default_off_and_restored_after_panic() {
    let error = |site: Site| {
        Err(ProductionSemanticKirErrorV1::MissingLocalDefinition {
            function: site.0,
            block: site.1,
            statement: site.2,
            local: site.3,
        })
    };
    let site = (7, 11, Some(13), 17);
    assert!(inject_failed_direct_load(error(site)).is_err());
    with_exact_sites(&[site], || {
        assert!(inject_failed_direct_load(error((7, 11, Some(14), 17))).is_err());
        assert!(inject_failed_direct_load(error(site)).is_ok());
        assert!(inject_failed_direct_load(error(site)).is_err());
    });
    with_exact_replays(&[site], 2, || {
        assert!(inject_failed_direct_load(error(site)).is_ok());
        assert!(inject_failed_direct_load(error(site)).is_ok());
        assert!(inject_failed_direct_load(error(site)).is_err());
    });
    assert!(inject_failed_direct_load(error(site)).is_err());
    assert!(
        std::panic::catch_unwind(|| with_exact_sites(&[site], || {
            panic!("injected construction failure")
        }))
        .is_err()
    );
    assert!(inject_failed_direct_load(error(site)).is_err());
    assert!(std::panic::catch_unwind(|| with_exact_sites(&[site], || ())).is_err());
    assert!(inject_failed_direct_load(error(site)).is_err());
    assert!(
        std::panic::catch_unwind(|| with_exact_sites(&[site], || { with_exact_sites(&[], || ()) }))
            .is_err()
    );
    assert!(inject_failed_direct_load(error(site)).is_err());
}
