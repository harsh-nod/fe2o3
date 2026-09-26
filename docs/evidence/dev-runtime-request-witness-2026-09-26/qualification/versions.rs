#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn qualification_corrupt_phase_storage_v1(&mut self) {
        self.phases.clear();
    }
}
