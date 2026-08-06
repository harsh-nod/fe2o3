use fe2o3_host::{
    InspectedPublishedDirectLinkPhysicalLayoutV1, ValidatedPublishedDirectLinkSelectionV1,
};

fn inspect_caller_bytes(admission: ValidatedPublishedDirectLinkSelectionV1, bytes: &[u8]) {
    let _ = InspectedPublishedDirectLinkPhysicalLayoutV1::inspect(admission, bytes);
}

fn main() {}
