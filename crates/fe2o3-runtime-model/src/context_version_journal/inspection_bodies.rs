macro_rules! owner_inspection_scalar_body {
    ($owner:ident, $field:ident) => {
        $owner.$field
    };
}

macro_rules! owner_inspection_length_body {
    ($owner:ident, $field:ident) => {
        $owner.$field.len()
    };
}

macro_rules! owner_inspection_deref_body {
    ($owner:ident, $field:ident) => {
        &$owner.$field
    };
}
