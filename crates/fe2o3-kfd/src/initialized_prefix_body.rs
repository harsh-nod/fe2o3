// These exact scalar expressions are also checked by the Verus proof root.
macro_rules! initialized_prefix_covers_body {
    ($syntax:ident, $prefix:ident, $extent:ident, $offset:ident, $len:ident) => {
        $syntax!({
            $prefix <= $extent && $len != 0 && $offset <= $prefix
                && $len <= $prefix - $offset
        })
    };
}

macro_rules! initialized_prefix_after_write_body {
    ($syntax:ident, $prefix:ident, $extent:ident, $offset:ident, $len:ident, $known:ident) => {
        $syntax!({
            if $prefix > $extent || $len == 0 || $offset > $extent
                || $len > $extent - $offset
            {
                return None;
            }
            let end = $offset + $len;
            if $known {
                if $offset <= $prefix && end > $prefix { Some(end) } else { Some($prefix) }
            } else {
                if $offset < $prefix { Some($offset) } else { Some($prefix) }
            }
        })
    };
}
