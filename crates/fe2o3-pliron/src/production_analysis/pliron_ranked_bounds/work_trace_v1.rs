const RANKED_BOUNDS_WORK_TRACE_SITES_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RankedBoundsWorkSiteV1 {
    file: &'static str,
    line: u32,
    column: u32,
}

impl RankedBoundsWorkSiteV1 {
    fn caller(location: &'static core::panic::Location<'static>) -> Self {
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RankedBoundsWorkCountsV1 {
    calls: usize,
    requested: usize,
    admitted: usize,
}

impl RankedBoundsWorkCountsV1 {
    fn merge(&mut self, other: Self) -> bool {
        let mut saturated = false;
        for (target, amount) in [
            (&mut self.calls, other.calls),
            (&mut self.requested, other.requested),
            (&mut self.admitted, other.admitted),
        ] {
            if let Some(sum) = target.checked_add(amount) {
                *target = sum;
            } else {
                *target = usize::MAX;
                saturated = true;
            }
        }
        saturated
    }
}

#[derive(Clone, Copy, Debug)]
struct RankedBoundsWorkRowV1 {
    site: RankedBoundsWorkSiteV1,
    counts: RankedBoundsWorkCountsV1,
}

// Diagnostic storage and CPU are outside the authoritative analysis meter.
// Default builds contain neither this fixed ledger nor the caller metadata.
struct RankedBoundsWorkTraceV1 {
    rows: [Option<RankedBoundsWorkRowV1>; RANKED_BOUNDS_WORK_TRACE_SITES_V1],
    len: usize,
    overflow: RankedBoundsWorkCountsV1,
    total: RankedBoundsWorkCountsV1,
    baseline: Option<usize>,
    expected_current: usize,
    counter_changed: bool,
    saturated: bool,
    reported: bool,
}

impl Default for RankedBoundsWorkTraceV1 {
    fn default() -> Self {
        Self {
            rows: [None; RANKED_BOUNDS_WORK_TRACE_SITES_V1],
            len: 0,
            overflow: RankedBoundsWorkCountsV1::default(),
            total: RankedBoundsWorkCountsV1::default(),
            baseline: None,
            expected_current: 0,
            counter_changed: false,
            saturated: false,
            reported: false,
        }
    }
}

impl RankedBoundsWorkTraceV1 {
    fn record(
        &mut self,
        site: RankedBoundsWorkSiteV1,
        current: usize,
        amount: usize,
        admitted: bool,
    ) {
        if self.baseline.is_none() {
            self.baseline = Some(current);
        } else if current != self.expected_current {
            self.counter_changed = true;
        }
        self.expected_current = if admitted {
            current.saturating_add(amount)
        } else {
            current
        };
        let counts = RankedBoundsWorkCountsV1 {
            calls: 1,
            requested: amount,
            admitted: if admitted { amount } else { 0 },
        };
        self.saturated |= self.total.merge(counts);
        for row in self.rows[..self.len].iter_mut().flatten() {
            if row.site == site {
                self.saturated |= row.counts.merge(counts);
                return;
            }
        }
        if self.len < self.rows.len() {
            self.rows[self.len] = Some(RankedBoundsWorkRowV1 { site, counts });
            self.len += 1;
        } else {
            self.saturated |= self.overflow.merge(counts);
        }
    }

    fn reconciles(&self, current: usize) -> bool {
        let mut sum = self.overflow;
        let mut saturated = self.saturated;
        for row in self.rows[..self.len].iter().flatten() {
            saturated |= sum.merge(row.counts);
        }
        !saturated
            && !self.counter_changed
            && sum == self.total
            && self
                .baseline
                .and_then(|baseline| baseline.checked_add(self.total.admitted))
                == Some(current)
    }

    fn report_denial(&mut self, current: usize, amount: usize, actual: usize, limit: usize) {
        self.report_denial_to(
            &mut std::io::stderr().lock(),
            current,
            amount,
            actual,
            limit,
        );
    }

    fn report_denial_to(
        &mut self,
        output: &mut impl std::io::Write,
        current: usize,
        amount: usize,
        actual: usize,
        limit: usize,
    ) {
        if self.reported {
            return;
        }
        self.reported = true;
        // Observer output errors must never replace the analysis result.
        let _ = self.write_report(output, current, amount, actual, limit);
    }

    fn write_report(
        &self,
        output: &mut impl std::io::Write,
        current: usize,
        amount: usize,
        actual: usize,
        limit: usize,
    ) -> std::io::Result<()> {
        writeln!(
            output,
            "FE2O3_BOUNDS_WORK_TRACE v1 baseline={} baseline_known={} current={} request={} actual={} limit={} calls={} requested={} admitted={} rows={} truncated={} saturated={} counter_changed={} reconciled={}",
            self.baseline.unwrap_or(0),
            self.baseline.is_some(),
            current,
            amount,
            actual,
            limit,
            self.total.calls,
            self.total.requested,
            self.total.admitted,
            self.len,
            self.overflow.calls != 0,
            self.saturated,
            self.counter_changed,
            self.reconciles(current)
        )?;
        for row in self.rows[..self.len].iter().flatten() {
            writeln!(
                output,
                "FE2O3_BOUNDS_WORK_SITE file={} line={} column={} calls={} requested={} admitted={}",
                row.site.file,
                row.site.line,
                row.site.column,
                row.counts.calls,
                row.counts.requested,
                row.counts.admitted
            )?;
        }
        writeln!(
            output,
            "FE2O3_BOUNDS_WORK_OVERFLOW calls={} requested={} admitted={}",
            self.overflow.calls, self.overflow.requested, self.overflow.admitted
        )?;
        writeln!(output, "FE2O3_BOUNDS_WORK_TRACE_END rows={}", self.len)
    }
}

#[cfg(test)]
include!("work_trace_v1_tests.rs");
