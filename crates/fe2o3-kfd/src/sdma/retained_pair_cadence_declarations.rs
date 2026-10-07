// Closed, copyable timing configuration; never ownership or completion authority.
macro_rules! retained_pair_cadence_declarations_v1 {
    ($items:ident) => {
        $items! {
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum Gfx942XgmiRetainedWaitCadenceV1 {
                Ordinary1ms,
                #[cfg_attr(
                    not(any(feature = "hardware-diagnostic", test)),
                    allow(dead_code)
                )]
                Ceiling25us,
            }
        }
    };
}
