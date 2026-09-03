use serde::Serialize;

#[derive(Serialize)]
pub struct Probe {
    pub value: u32,
}
