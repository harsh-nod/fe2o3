#[derive(Clone, Copy, Debug)]
enum Limit {
    Roomy,
    ExactWork,
    ShortWork,
    ExactPeak,
    ShortPeak,
    ExactFloor,
    ShortFloor,
    ShortEntry,
}
const LIMITS: [Limit; 8] = [
    Limit::Roomy,
    Limit::ExactWork,
    Limit::ShortWork,
    Limit::ExactPeak,
    Limit::ShortPeak,
    Limit::ExactFloor,
    Limit::ShortFloor,
    Limit::ShortEntry,
];
impl Limit {
    fn inputs(self, floor: usize, work: usize, peak: usize) -> (usize, usize, usize) {
        let work = match self {
            Self::ExactWork => work,
            Self::ShortWork => work - 1,
            Self::ShortEntry => ENTRY - 1,
            _ => WORK_LIMIT,
        };
        let storage = match self {
            Self::ExactPeak => peak,
            Self::ShortPeak => peak - 1,
            _ => STORAGE_LIMIT,
        };
        let prepaid = match self {
            Self::ExactFloor => floor,
            Self::ShortFloor => floor - 1,
            _ => floor + EXTRA,
        };
        (work, storage, prepaid)
    }
    fn succeeds(self) -> bool {
        matches!(
            self,
            Self::Roomy | Self::ExactWork | Self::ExactPeak | Self::ExactFloor
        )
    }
    fn check_error(self, error: &Error, budget: &Budget<'_>, peak: usize) {
        match self {
            Self::ShortWork | Self::ShortEntry => {
                assert!(matches!(resource(error), Resource::Work(_)))
            }
            Self::ShortPeak => {
                assert!(matches!(resource(error), Resource::Storage(_)));
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            Self::ShortFloor => {
                assert!(matches!(error, Error::Resource(Resource::Accounting)));
                assert_eq!(budget.work(), ENTRY);
                assert_eq!(budget.failed_storage(), None);
            }
            _ => panic!("unexpected refusal for {self:?}: {error:?}"),
        }
    }
    fn failed_work(self, exact: usize) -> Option<usize> {
        match self {
            Self::ShortWork => Some(exact),
            Self::ShortEntry => Some(ENTRY),
            _ => None,
        }
    }
}

fn revalidation_boundaries(owner: &Prepared, supervisor: &Supervisor, exact_work: usize) {
    let floor = owner.retained_storage() + supervisor.retained_storage();
    let mut peak = 0;
    for case in LIMITS {
        let (work_limit, storage_limit, prepaid) = case.inputs(floor, exact_work, peak);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        let result = owner.revalidate(supervisor, &mut budget);
        assert_eq!(budget.storage(), prepaid, "{case:?}");
        if case.succeeds() {
            result.unwrap();
            assert_eq!(budget.work(), exact_work);
            if matches!(case, Limit::Roomy) {
                peak = budget.peak_storage();
            }
            assert_eq!(
                budget.peak_storage(),
                peak - if matches!(case, Limit::ExactFloor) {
                    EXTRA
                } else {
                    0
                }
            );
            assert_eq!(budget.failed_storage(), None);
        } else {
            case.check_error(&result.unwrap_err(), &budget, peak);
        }
        assert_eq!(work.failed_work(), case.failed_work(exact_work), "{case:?}");
    }
}
