macro_rules! composed_retained_credit_dispatch_body_v1 {
    ($this:ident, $credit:ident, $bytes:ident) => {{
        Arc::ptr_eq(&$this.0, &$credit.account.0)
            && $this
                .0
                .account
                .matches_retained_charge_v1(&$credit.inner, request_charge($bytes))
    }};
}
