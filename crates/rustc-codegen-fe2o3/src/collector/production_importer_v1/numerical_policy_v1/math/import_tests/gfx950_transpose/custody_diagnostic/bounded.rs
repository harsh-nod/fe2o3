use std::fmt::{self, Write};

pub(super) type Result<T = ()> = std::result::Result<T, &'static str>;
pub(super) const MAX_BYTES: usize = 1024 * 1024;
pub(super) const MAX_ITEMS: usize = 131_072;
const RESERVE: usize = 256;

pub(super) struct Output {
    pub(super) text: String,
    pub(super) remaining: usize,
}

impl Output {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
            remaining: MAX_ITEMS,
        }
    }

    pub(super) fn charge(&mut self, units: usize) -> Result {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or("diagnostic item bound reached")?;
        Ok(())
    }

    pub(super) fn line(&mut self, args: fmt::Arguments<'_>) -> Result {
        self.charge(1)?;
        self.write_fmt(args)
            .and_then(|_| self.write_char('\n'))
            .map_err(|_| "diagnostic byte bound reached")
    }

    pub(super) fn finish(mut self, result: Result) -> String {
        if let Err(reason) = result {
            // All reasons are fixed local strings shorter than the reserved tail.
            let _ = writeln!(
                self.text,
                "\nTRANSPOSE_CUSTODY_INCOMPLETE: {reason}; not a complete source/return observation"
            );
        }
        self.text
    }
}

impl Write for Output {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > (MAX_BYTES - RESERVE).saturating_sub(self.text.len()) {
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
