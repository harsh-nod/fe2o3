// Exact R67 coordinate order is checked against R67ResourceKindV1.
macro_rules! resource_request_charge_body_v1 {
    ($bytes:ident) => {
        R67ResourceVectorV1 {
            counts: [0, $bytes, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        }
    };
}
