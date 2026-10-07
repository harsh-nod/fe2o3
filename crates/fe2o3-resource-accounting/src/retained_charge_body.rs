// Borrowed record observation only; account identity and locking are caller duties.
macro_rules! retained_credit_record_matches_body {
    ($record:ident, $owner:ident, $expected:ident) => {
        $owner != 0
            && $record.owner == $owner
            && $record.phase == Phase::Retained
            && $record.charge == $expected
    };
}
