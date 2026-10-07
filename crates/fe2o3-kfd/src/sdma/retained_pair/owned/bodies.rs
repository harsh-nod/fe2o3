// These are the production storage transitions, after run_operation has returned.
macro_rules! retained_pair_owned_parts_body {
    ($parts:ident) => {{ ($parts.queue, $parts.source, $parts.destination) }};
}

macro_rules! retained_pair_owned_new_body {
    ($context:ident) => {{
        Self {
            context: Some($context),
        }
    }};
}

macro_rules! retained_pair_owned_context_body {
    ($owner:ident) => {{
        match $owner.context.as_ref() {
            Some(context) => context,
            None => empty_owned_context(),
        }
    }};
}

macro_rules! retained_pair_owned_context_mut_body {
    ($owner:ident) => {{
        match $owner.context.as_mut() {
            Some(context) => context,
            None => empty_owned_context(),
        }
    }};
}

macro_rules! retained_pair_owned_take_body {
    ($owner:ident) => {{
        match $owner.context.take() {
            Some(context) => context,
            None => empty_owned_context(),
        }
    }};
}

macro_rules! retained_pair_owned_admit_post_body {
    ($owner:ident, $result:ident) => {{
        match $result {
            Ok(()) => Ok($owner),
            Err(error) => Err(Failure {
                owner: $owner,
                error,
                entry_refusal: true,
            }),
        }
    }};
}

macro_rules! retained_pair_owned_finish_post_body {
    ($owner:ident, $result:ident) => {{
        match $result {
            Ok(()) => Ok($owner.take()),
            Err(error) => {
                $owner.context_mut().quarantine();
                Err(Failure {
                    owner: $owner,
                    error,
                    entry_refusal: false,
                })
            }
        }
    }};
}

macro_rules! retained_pair_owned_recover_body {
    ($failure:ident) => {{
        if !$failure.entry_refusal || $failure.owner.context().terminal() {
            return Err($failure);
        }
        let context = $failure.owner.take();
        Ok(($failure.error, context))
    }};
}
