use fe2o3_device::{Global, PrivateMemoryView, ReadOnly};

fn require_global<Brand>(_input: &Global<'_, f32, ReadOnly, Brand>) {}

fn substitute_private<Brand>(input: &PrivateMemoryView<'_, f32, ReadOnly, Brand>) {
    require_global(input);
}
