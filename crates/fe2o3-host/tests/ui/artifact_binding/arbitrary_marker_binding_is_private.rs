use fe2o3_host::ValidatedArtifactSelectionV1;

struct ArbitraryKernel;

fn forge(validated: &ValidatedArtifactSelectionV1) {
    let _brand = validated.bind_marker::<ArbitraryKernel>();
}

fn main() {}
