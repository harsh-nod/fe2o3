use fe2o3_host::InspectedPublishedDirectLinkPhysicalLayoutV1;

fn inspect(token: InspectedPublishedDirectLinkPhysicalLayoutV1) {
    let InspectedPublishedDirectLinkPhysicalLayoutV1 { inspected, .. } = token;
    let _ = inspected;
}

fn main() {}
