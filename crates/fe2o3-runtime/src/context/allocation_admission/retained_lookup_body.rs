macro_rules! context_retained_credit_lookup_body_v1 {
    ($this:ident, $id:ident, $device:ident, $byte_len:ident) => {{
        match ($this.accounts.get(&$device), $this.retained.get(&$id)) {
            (None, None) => true,
            (Some(account), Some(credits)) => {
                account.matches_retained_charge_v1($device, credits, request_charge($byte_len))
            }
            _ => false,
        }
    }};
}
