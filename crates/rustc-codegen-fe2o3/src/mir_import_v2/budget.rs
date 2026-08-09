use super::normalized::CaptureLimitsV2;
use std::fmt::{self, Write};

pub(super) struct CaptureBudgetV2 {
    limits: CaptureLimitsV2,
    work_items: usize,
    text_bytes: usize,
}

impl CaptureBudgetV2 {
    pub(super) fn new(limits: CaptureLimitsV2) -> Self {
        Self {
            limits,
            work_items: 0,
            text_bytes: 0,
        }
    }

    pub(super) fn charge_work(&mut self, label: &str, count: usize) -> Result<(), BudgetErrorV2> {
        self.work_items = self
            .work_items
            .checked_add(count)
            .ok_or_else(|| BudgetErrorV2::new(label, "total work count overflowed"))?;
        if self.work_items > self.limits.max_total_work_items {
            return Err(BudgetErrorV2::new(
                label,
                format!(
                    "total capture work bound exceeded: {} > {}",
                    self.work_items, self.limits.max_total_work_items
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn bounded_debug(
        &mut self,
        label: &str,
        value: &impl fmt::Debug,
    ) -> Result<String, BudgetErrorV2> {
        self.bounded_format(label, format_args!("{value:?}"))
    }

    pub(super) fn bounded_display(
        &mut self,
        label: &str,
        value: &impl fmt::Display,
    ) -> Result<String, BudgetErrorV2> {
        self.bounded_format(label, format_args!("{value}"))
    }

    pub(super) fn bounded_str(
        &mut self,
        label: &str,
        value: &str,
    ) -> Result<String, BudgetErrorV2> {
        self.bounded_format(label, format_args!("{value}"))
    }

    fn bounded_format(
        &mut self,
        label: &str,
        arguments: fmt::Arguments<'_>,
    ) -> Result<String, BudgetErrorV2> {
        let remaining_total = self
            .limits
            .max_total_text_bytes
            .checked_sub(self.text_bytes)
            .ok_or_else(|| BudgetErrorV2::new(label, "total text budget was exhausted"))?;
        let per_value_limit = self.limits.max_text_bytes.min(remaining_total);
        let mut writer = BoundedWriter::new(per_value_limit);
        if writer.write_fmt(arguments).is_err() {
            return Err(BudgetErrorV2::new(
                label,
                format!(
                    "formatted text bound exceeded (per-value {}, remaining total {})",
                    self.limits.max_text_bytes, remaining_total
                ),
            ));
        }
        if writer.text.is_empty() || writer.text.contains('\0') {
            return Err(BudgetErrorV2::new(
                label,
                "formatted text is empty or contains NUL",
            ));
        }
        self.text_bytes = self
            .text_bytes
            .checked_add(writer.text.len())
            .ok_or_else(|| BudgetErrorV2::new(label, "total text count overflowed"))?;
        Ok(writer.text)
    }

    pub(super) fn work_items(&self) -> usize {
        self.work_items
    }

    pub(super) fn text_bytes(&self) -> usize {
        self.text_bytes
    }
}

struct BoundedWriter {
    text: String,
    limit: usize,
}

impl BoundedWriter {
    fn new(limit: usize) -> Self {
        Self {
            text: String::with_capacity(limit.min(256)),
            limit,
        }
    }
}

impl Write for BoundedWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        if end > self.limit {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BudgetErrorV2 {
    pub(super) label: String,
    pub(super) reason: String,
}

impl BudgetErrorV2 {
    fn new(label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for BudgetErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.label, self.reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ChunkedText(usize);

    impl fmt::Display for ChunkedText {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            for _ in 0..self.0 {
                formatter.write_str("12345678")?;
            }
            Ok(())
        }
    }

    #[test]
    fn formatter_stops_at_the_per_value_bound() {
        let mut budget = CaptureBudgetV2::new(CaptureLimitsV2 {
            max_text_bytes: 15,
            max_total_text_bytes: 1_024,
            ..CaptureLimitsV2::default()
        });
        let error = budget
            .bounded_display("chunked", &ChunkedText(1_000_000))
            .unwrap_err();
        assert!(error.to_string().contains("per-value 15"));
        assert_eq!(budget.text_bytes(), 0);
    }

    #[test]
    fn aggregate_work_and_text_are_checked_before_commit() {
        let mut budget = CaptureBudgetV2::new(CaptureLimitsV2 {
            max_total_work_items: 3,
            max_text_bytes: 8,
            max_total_text_bytes: 5,
            ..CaptureLimitsV2::default()
        });
        budget.charge_work("first", 3).unwrap();
        assert_eq!(budget.work_items(), 3);
        assert!(budget.charge_work("second", 1).is_err());
        assert_eq!(budget.bounded_str("a", "abc").unwrap(), "abc");
        assert!(budget.bounded_str("b", "xyz").is_err());
        assert_eq!(budget.text_bytes(), 3);
    }
}
