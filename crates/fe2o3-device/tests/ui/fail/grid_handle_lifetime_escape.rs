use fe2o3_device::{Grid, Invocation3D};

fn escape(invocation: &Invocation3D) -> Grid<'static> {
    Grid::from_invocation_snapshot(invocation).unwrap()
}

fn main() {}
