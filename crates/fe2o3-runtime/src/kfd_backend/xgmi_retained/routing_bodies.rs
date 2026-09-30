// Boundary expressions retain their exact production callback call sites.
// The shared matches route normal returns; they establish no native authority.
macro_rules! retained_pair_publish_once_body {
    ($syntax:ident, $phase:expr, $requests:ident, $submit:expr) => {
        $syntax!({
            match $phase {
                BatchPhase::Prepared($requests) => match $submit {
                    Ok(tickets) => BatchPhase::Pending(tickets),
                    Err(failure) => BatchPhase::SubmitFailure(failure),
                },
                other => other,
            }
        })
    };
}

macro_rules! retained_pair_advance_body {
    ($syntax:ident, $published:expr, $work:ident, $deadline:ident, $tickets:ident $(,)?) => {
        $syntax!({
            let phase = $published;
            match phase {
                BatchPhase::Pending($tickets) => Advanced::Waited($work.wait($tickets, $deadline)),
                other => Advanced::Unchanged(other),
            }
        })
    };
}
