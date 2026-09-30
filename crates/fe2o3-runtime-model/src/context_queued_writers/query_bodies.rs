// Shared read-only outer helpers; no admission or settlement authority.
macro_rules! queued_writer_same_body_v1 {
    ($left:ident, $right:ident) => {
        $left.slot == $right.slot && queued_writer_key_same_v1($left.key, $right.key)
    };
}

macro_rules! queued_ensure_usable_body_v1 {
    ($owner:ident) => {{
        if $owner.disposal_terminal {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }};
}

macro_rules! queued_root_body_v1 {
    ($owner:ident, $writer:ident) => {{
        $owner.ensure_usable()?;
        $owner.inner.lookup_writer($writer)?;
        if $writer.slot >= $owner.roots.len() {
            return Err(Error::InvalidReference);
        }
        let root = match $owner.roots[$writer.slot] {
            Some(root) => root,
            None => return Err(Error::InvalidReference),
        };
        if !queued_writer_same_v1(root.writer, $writer) {
            return Err(Error::InvalidReference);
        }
        Ok(root)
    }};
}

macro_rules! queued_destination_body_v1 {
    ($owner:ident, $write:ident) => {{
        $owner.ensure_usable()?;
        let state = $owner.inner.lookup_allocation($write.allocation)?;
        if state.device.context_generation != $write.device.context_generation
            || state.device.local != $write.device.local
        {
            return Err(Error::AllocationDeviceMismatch);
        }
        if state.byte_extent != $write.byte_extent {
            return Err(Error::AllocationExtentMismatch);
        }
        Ok(state)
    }};
}

macro_rules! queued_active_lookup_body_v1 {
    ($owner:ident, $reference:ident) => {
        $owner.inner.lookup_producer_read($reference)
    };
}

macro_rules! queued_active_status_body_v1 {
    ($owner:ident, $reference:ident) => {{
        $owner.ensure_usable()?;
        $owner.inner.producer_read_status($reference)
    }};
}
