mod support;

use std::io::{self, Write as _};

fn main() -> io::Result<()> {
    io::stdout().write_all(&support::tutorial_fill_v1::canonical_kir_v7())
}
