#[path = "support/async_owner_copy.rs"]
mod async_owner_copy;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    async_owner_copy::main_for_profile(true)
}
