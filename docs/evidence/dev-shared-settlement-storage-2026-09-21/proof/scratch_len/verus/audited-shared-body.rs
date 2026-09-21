        let writer_returns = match $storage.writer_free_len.checked_add(1) {
            Some(value) => value,
            None => return Err($invalid),
        };
        let member_returns = match $storage.member_free_len.checked_add($count) {
            Some(value) => value,
            None => return Err($invalid),
        };
        if writer_returns > $storage.writer_limit
            || writer_returns > $storage.writer_storage
            || member_returns > $storage.member_limit
            || member_returns > $storage.member_storage
            || false
        {
            return Err($invalid);
        }
        Ok(())
