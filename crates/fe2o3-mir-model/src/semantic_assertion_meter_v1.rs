//! Shared caller-owned metering contract for live semantic analysis.
/// Calls precede work and requested payload allocation. A scoped caller keeps
/// all reservations until results, the analysis and partial backing are dropped,
/// including failures and unwinding. This interface never refunds caller credit.
pub trait SemanticAssertionMeterV1 {
    type Error;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error>;

    /// Historical logical visits are part of the same total-work ledger. The
    /// separate hook permits a legacy diagnostic counter, not a second grant.
    fn charge_legacy_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_work(amount)
    }
    fn charge_legacy_scan_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_legacy_work(amount)
    }
}
