//! Closed argument transport for the physical-entry whole-root source census.
//! Numeric local indexes are compiler-owned MIR positions, never owner IDs.
pub(super) const MAX_LOCALS: usize = 4096;
const ABSENT: u8 = u8::MAX;

#[cfg(test)]
#[path = "production_physical_entry_transport_v37_tests.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArgumentTransport {
    origins: [u8; MAX_LOCALS],
    locals: usize,
    marker_consumed: bool,
}
impl ArgumentTransport {
    pub(super) fn new(locals: usize) -> Result<Self, &'static str> {
        if !(6..=MAX_LOCALS).contains(&locals) {
            return Err("physical-entry source local bound exceeded");
        }
        let mut state = Self {
            origins: [ABSENT; MAX_LOCALS],
            locals,
            marker_consumed: false,
        };
        for index in 0..5 {
            state.origins[index + 1] = index as u8;
        }
        Ok(state)
    }
    fn temporary(&self, index: usize) -> Result<(), &'static str> {
        if index < 6 || index >= self.locals {
            return Err("physical-entry source mutates a root argument or invalid local");
        }
        Ok(())
    }
    pub(super) fn storage(&mut self, index: usize) -> Result<(), &'static str> {
        self.temporary(index)?;
        self.origins[index] = ABSENT;
        Ok(())
    }
    pub(super) fn assign(
        &mut self,
        destination: usize,
        source: usize,
        moved: bool,
    ) -> Result<(), &'static str> {
        self.temporary(destination)?;
        let origin = self
            .origins
            .get(source)
            .copied()
            .filter(|_| source < self.locals)
            .filter(|value| *value != ABSENT)
            .ok_or("physical-entry source transport reads an undefined or foreign value")?;
        if origin == 0 && !moved {
            return Err("physical-entry output owner cannot be copied");
        }
        // Every check precedes mutation; self-move transport preserves its value.
        if moved {
            self.origins[source] = ABSENT;
        }
        self.origins[destination] = origin;
        Ok(())
    }
    pub(super) fn consume_marker(
        &mut self,
        locals: [u32; 5],
        moved: u8,
    ) -> Result<(), &'static str> {
        if self.marker_consumed || moved & 1 == 0 || moved & !0b1_1111 != 0 {
            return Err("physical-entry marker repeated or output owner not moved");
        }
        for (index, local) in locals.into_iter().enumerate() {
            let actual = self
                .origins
                .get(local as usize)
                .copied()
                .filter(|_| (local as usize) < self.locals);
            if actual != Some(index as u8) {
                return Err("physical-entry marker operands differ from exact root argument order");
            }
        }
        for (index, local) in locals.into_iter().enumerate() {
            if moved & (1 << index) != 0 {
                self.origins[local as usize] = ABSENT;
            }
        }
        self.marker_consumed = true;
        Ok(())
    }
    pub(super) fn require_marker(&self) -> Result<(), &'static str> {
        if self.marker_consumed {
            Ok(())
        } else {
            Err("physical-entry source never consumed its marker")
        }
    }
}
