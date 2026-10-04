use fe2o3_device::{StaticIndex, StaticViewMut};

fn alias(view: &mut StaticViewMut<'_, u32, 4>) {
    let first = view.at_const_mut(StaticIndex::<4, 0>::CHECKED);
    let second = view.at_const_mut(StaticIndex::<4, 1>::CHECKED);
    *first += *second;
}

fn main() {}
