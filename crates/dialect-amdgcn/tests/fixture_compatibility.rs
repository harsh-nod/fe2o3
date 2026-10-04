use std::fs;
use std::path::Path;

#[test]
fn historical_fixture_paths_match_the_model_owned_fixtures() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let facade_fixtures = manifest.join("tests/fixtures");
    let model_fixtures = manifest.join("../fe2o3-amdgcn-model/tests/fixtures");

    let fixture_names = |directory: &Path| {
        let mut names = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    };

    let names = fixture_names(&model_fixtures);
    assert_eq!(fixture_names(&facade_fixtures), names);
    for name in names {
        assert_eq!(
            fs::read(facade_fixtures.join(&name)).unwrap(),
            fs::read(model_fixtures.join(&name)).unwrap(),
            "historical fixture {name:?} drifted from the model-owned fixture"
        );
    }
}
