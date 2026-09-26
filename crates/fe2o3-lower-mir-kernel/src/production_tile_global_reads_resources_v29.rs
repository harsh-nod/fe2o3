struct Gate {
    slot: usize,
    ledger: Ledger,
    floor: usize,
    failure: RefCell<Option<Failure>>,
}
impl Gate {
    fn new(budget: &Budget<'_>) -> Self {
        Self {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            failure: RefCell::new(None),
        }
    }
    fn valid(&self, budget: &Budget<'_>) -> bool {
        self.slot == std::ptr::from_ref(budget) as usize
            && self.ledger == budget.work_ledger_identity_v1()
            && budget.storage() >= self.floor
    }
    fn save<T>(&self, result: R<T>) -> R<T> {
        if let Err(error) = &result {
            if self.failure.borrow().is_none() {
                *self.failure.borrow_mut() = Some(error.clone());
            }
        }
        result
    }
    fn query(&self, budget: &mut Budget<'_>) -> R<()> {
        if !self.valid(budget) {
            return self.save(Err(Resource::Accounting.into()));
        }
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        self.save(budget.charge_work(1).map_err(Into::into))
    }
    fn postflight(&self, budget: &mut Budget<'_>) -> R<()> {
        if !self.valid(budget) || budget.storage() != self.floor {
            return self.save(Err(Resource::Accounting.into()));
        }
        self.query(budget)
    }
}
fn binding(obligation: Option<usize>, reason: &'static str) -> Failure {
    Failure::Binding { obligation, reason }
}
fn drain<T>(value: T) {
    let mut result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)));
    }
}
fn source_scope<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    let entry = Gate::new(budget);
    let returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let headers = size_of::<Gate>()
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<Joined>()))
            .and_then(|n| n.checked_add(size_of::<ProductionCheckedTileGlobalReadsV29<'_>>()))
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<R<T>>>()))
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<R<T>>>()))
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<()>>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(headers)?;
        run(budget)
    }));
    if !entry.valid(budget) {
        drain(returned);
        return Err(Resource::Accounting.into());
    }
    let result = match returned {
        Ok(result) => result,
        Err(payload) => {
            drain(payload);
            Err(Failure::Panicked)
        }
    };
    if let Err(error) = budget.release_storage(budget.storage() - entry.floor) {
        drain(result);
        return Err(error.into());
    }
    result
}
fn reserve<T>(rows: &mut Vec<T>, count: usize, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(2)?;
    if rows.capacity() >= count {
        return Ok(());
    }
    budget.charge_work(rows.len())?;
    let old = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    rows.try_reserve_exact(count.checked_sub(rows.len()).ok_or(Resource::Accounting)?)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.release_storage(old)?;
    Ok(())
}
fn push<T>(rows: &mut Vec<T>, value: T, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        reserve(
            rows,
            rows.capacity()
                .max(1)
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
            budget,
        )?;
    }
    rows.push(value);
    Ok(())
}
fn sort<T>(
    rows: &mut [T],
    width: usize,
    budget: &mut Budget<'_>,
    mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
) -> R<()> {
    // Bounded in-place heapsort. This is only a resource/index helper, never a
    // second source, arithmetic, provenance or predicate interpreter.
    let height = if rows.len() < 2 {
        0
    } else {
        usize::BITS as usize - (rows.len() - 1).leading_zeros() as usize
    };
    let work = rows
        .len()
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_mul(width.max(1)))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    fn sift<T>(
        rows: &mut [T],
        mut root: usize,
        end: usize,
        cmp: &mut impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) {
        while let Some(child) = root.checked_mul(2).and_then(|n| n.checked_add(1)) {
            if child >= end {
                return;
            }
            let right = child + 1;
            let selected = if right < end && cmp(&rows[child], &rows[right]).is_lt() {
                right
            } else {
                child
            };
            if !cmp(&rows[root], &rows[selected]).is_lt() {
                return;
            }
            rows.swap(root, selected);
            root = selected;
        }
    }
    let length = rows.len();
    for start in (0..length / 2).rev() {
        sift(rows, start, length, &mut compare);
    }
    for end in (1..length).rev() {
        rows.swap(0, end);
        sift(rows, 0, end, &mut compare);
    }
    Ok(())
}
fn lower_bound<T>(
    rows: &[T],
    width: usize,
    budget: &mut Budget<'_>,
    mut compare: impl FnMut(&T) -> std::cmp::Ordering,
) -> R<usize> {
    budget.charge_work(1)?;
    let (mut left, mut right) = (0, rows.len());
    while left < right {
        budget.charge_work(width.max(1))?;
        let mid = left + (right - left) / 2;
        if compare(&rows[mid]).is_lt() {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    Ok(left)
}
