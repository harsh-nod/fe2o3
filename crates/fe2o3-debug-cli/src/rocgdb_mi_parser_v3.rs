//! Strict bounded parser for GDB/MI output records.

use std::collections::BTreeMap;

pub(crate) const MAX_MI_LINE_BYTES_V3: usize = 256 * 1024;
pub(crate) const MAX_MI_STRING_BYTES_V3: usize = 128 * 1024;
pub(crate) const MAX_MI_FIELDS_V3: usize = 4_096;
pub(crate) const MAX_MI_DEPTH_V3: usize = 24;
pub(crate) const MAX_MI_NAME_BYTES_V3: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MiParserLimitsV3 {
    pub(crate) max_line_bytes: usize,
    pub(crate) max_string_bytes: usize,
    pub(crate) max_fields: usize,
    pub(crate) max_depth: usize,
}

impl Default for MiParserLimitsV3 {
    fn default() -> Self {
        Self {
            max_line_bytes: MAX_MI_LINE_BYTES_V3,
            max_string_bytes: MAX_MI_STRING_BYTES_V3,
            max_fields: MAX_MI_FIELDS_V3,
            max_depth: MAX_MI_DEPTH_V3,
        }
    }
}

impl MiParserLimitsV3 {
    pub(crate) fn validate(self) -> Result<(), MiParseErrorV3> {
        if self.max_line_bytes == 0 || self.max_line_bytes > MAX_MI_LINE_BYTES_V3 {
            return Err(MiParseErrorV3::LimitOutOfRange("line"));
        }
        if self.max_string_bytes == 0 || self.max_string_bytes > MAX_MI_STRING_BYTES_V3 {
            return Err(MiParseErrorV3::LimitOutOfRange("string"));
        }
        if self.max_fields == 0 || self.max_fields > MAX_MI_FIELDS_V3 {
            return Err(MiParseErrorV3::LimitOutOfRange("fields"));
        }
        if self.max_depth == 0 || self.max_depth > MAX_MI_DEPTH_V3 {
            return Err(MiParseErrorV3::LimitOutOfRange("depth"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MiAsyncKindV3 {
    Exec,
    Status,
    Notify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MiStreamKindV3 {
    Console,
    Target,
    Log,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MiListV3 {
    Values(Vec<MiValueV3>),
    Results(Vec<(String, MiValueV3)>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MiValueV3 {
    Const(Vec<u8>),
    Tuple(BTreeMap<String, MiValueV3>),
    List(MiListV3),
}

impl MiValueV3 {
    pub(crate) fn as_const(&self) -> Option<&[u8]> {
        match self {
            Self::Const(value) => Some(value),
            Self::Tuple(_) | Self::List(_) => None,
        }
    }

    pub(crate) fn as_tuple(&self) -> Option<&BTreeMap<String, MiValueV3>> {
        match self {
            Self::Tuple(value) => Some(value),
            Self::Const(_) | Self::List(_) => None,
        }
    }

    pub(crate) fn as_values(&self) -> Option<&[MiValueV3]> {
        match self {
            Self::List(MiListV3::Values(values)) => Some(values),
            Self::Const(_) | Self::Tuple(_) | Self::List(MiListV3::Results(_)) => None,
        }
    }
}

pub(crate) type MiResultsV3 = BTreeMap<String, MiValueV3>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MiRecordV3 {
    Result {
        token: Option<u64>,
        class: String,
        results: MiResultsV3,
    },
    Async {
        token: Option<u64>,
        kind: MiAsyncKindV3,
        class: String,
        results: MiResultsV3,
    },
    Stream {
        kind: MiStreamKindV3,
        bytes: Vec<u8>,
    },
    Prompt,
}

pub(crate) fn parse_mi_record_v3(
    line: &[u8],
    limits: MiParserLimitsV3,
) -> Result<MiRecordV3, MiParseErrorV3> {
    limits.validate()?;
    if line.is_empty() {
        return Err(MiParseErrorV3::Empty);
    }
    if line.len() > limits.max_line_bytes {
        return Err(MiParseErrorV3::LineTooLarge);
    }
    let payload = if let Some(payload) = line.strip_suffix(b"\r\n") {
        payload
    } else if let Some(payload) = line.strip_suffix(b"\n") {
        payload
    } else {
        return Err(MiParseErrorV3::MissingTerminator);
    };
    if payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(MiParseErrorV3::EmbeddedLineBreak);
    }
    if payload == b"(gdb) " || payload == b"(gdb)" {
        return Ok(MiRecordV3::Prompt);
    }
    let mut parser = ParserV3 {
        bytes: payload,
        cursor: 0,
        limits,
        fields: 0,
    };
    let token = parser.parse_token()?;
    let marker = parser.take().ok_or(MiParseErrorV3::MissingRecordMarker)?;
    let record = match marker {
        b'^' => MiRecordV3::Result {
            token,
            class: parser.parse_name()?,
            results: parser.parse_optional_results(0)?,
        },
        b'*' | b'+' | b'=' => MiRecordV3::Async {
            token,
            kind: match marker {
                b'*' => MiAsyncKindV3::Exec,
                b'+' => MiAsyncKindV3::Status,
                b'=' => MiAsyncKindV3::Notify,
                _ => unreachable!(),
            },
            class: parser.parse_name()?,
            results: parser.parse_optional_results(0)?,
        },
        b'~' | b'@' | b'&' if token.is_none() => MiRecordV3::Stream {
            kind: match marker {
                b'~' => MiStreamKindV3::Console,
                b'@' => MiStreamKindV3::Target,
                b'&' => MiStreamKindV3::Log,
                _ => unreachable!(),
            },
            bytes: parser.parse_c_string()?,
        },
        b'~' | b'@' | b'&' => return Err(MiParseErrorV3::TokenOnStream),
        _ => return Err(MiParseErrorV3::MissingRecordMarker),
    };
    if parser.cursor != payload.len() {
        return Err(MiParseErrorV3::TrailingInput);
    }
    Ok(record)
}

struct ParserV3<'a> {
    bytes: &'a [u8],
    cursor: usize,
    limits: MiParserLimitsV3,
    fields: usize,
}

impl ParserV3<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn take(&mut self) -> Option<u8> {
        let value = self.peek()?;
        self.cursor += 1;
        Some(value)
    }

    fn parse_token(&mut self) -> Result<Option<u64>, MiParseErrorV3> {
        let start = self.cursor;
        while self.peek().is_some_and(|value| value.is_ascii_digit()) {
            self.cursor += 1;
        }
        if self.cursor == start {
            return Ok(None);
        }
        let token = std::str::from_utf8(&self.bytes[start..self.cursor])
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(MiParseErrorV3::InvalidToken)?;
        Ok(Some(token))
    }

    fn parse_name(&mut self) -> Result<String, MiParseErrorV3> {
        let start = self.cursor;
        while self.peek().is_some_and(is_name_byte) {
            self.cursor += 1;
        }
        let bytes = &self.bytes[start..self.cursor];
        if bytes.is_empty() || bytes.len() > MAX_MI_NAME_BYTES_V3 {
            return Err(MiParseErrorV3::InvalidName);
        }
        String::from_utf8(bytes.to_vec()).map_err(|_| MiParseErrorV3::InvalidName)
    }

    fn count_field(&mut self) -> Result<(), MiParseErrorV3> {
        self.fields = self
            .fields
            .checked_add(1)
            .ok_or(MiParseErrorV3::FieldLimit)?;
        if self.fields > self.limits.max_fields {
            return Err(MiParseErrorV3::FieldLimit);
        }
        Ok(())
    }

    fn require_depth(&self, depth: usize) -> Result<(), MiParseErrorV3> {
        if depth > self.limits.max_depth {
            Err(MiParseErrorV3::DepthLimit)
        } else {
            Ok(())
        }
    }

    fn parse_optional_results(&mut self, depth: usize) -> Result<MiResultsV3, MiParseErrorV3> {
        self.require_depth(depth)?;
        let mut results = BTreeMap::new();
        while self.peek() == Some(b',') {
            self.cursor += 1;
            let (name, value) = self.parse_result(depth + 1)?;
            if results.insert(name, value).is_some() {
                return Err(MiParseErrorV3::DuplicateVariable);
            }
        }
        Ok(results)
    }

    fn parse_result(&mut self, depth: usize) -> Result<(String, MiValueV3), MiParseErrorV3> {
        self.require_depth(depth)?;
        self.count_field()?;
        let name = self.parse_name()?;
        if self.take() != Some(b'=') {
            return Err(MiParseErrorV3::MissingEquals);
        }
        let value = self.parse_value(depth + 1)?;
        Ok((name, value))
    }

    fn parse_value(&mut self, depth: usize) -> Result<MiValueV3, MiParseErrorV3> {
        self.require_depth(depth)?;
        match self.peek() {
            Some(b'"') => self.parse_c_string().map(MiValueV3::Const),
            Some(b'{') => self.parse_tuple(depth + 1).map(MiValueV3::Tuple),
            Some(b'[') => self.parse_list(depth + 1).map(MiValueV3::List),
            _ => Err(MiParseErrorV3::InvalidValue),
        }
    }

    fn parse_tuple(&mut self, depth: usize) -> Result<MiResultsV3, MiParseErrorV3> {
        self.require_depth(depth)?;
        self.cursor += 1;
        let mut results = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.cursor += 1;
            return Ok(results);
        }
        loop {
            let (name, value) = self.parse_result(depth + 1)?;
            if results.insert(name, value).is_some() {
                return Err(MiParseErrorV3::DuplicateVariable);
            }
            match self.take() {
                Some(b',') => {}
                Some(b'}') => return Ok(results),
                _ => return Err(MiParseErrorV3::UnterminatedTuple),
            }
        }
    }

    fn parse_list(&mut self, depth: usize) -> Result<MiListV3, MiParseErrorV3> {
        self.require_depth(depth)?;
        self.cursor += 1;
        if self.peek() == Some(b']') {
            self.cursor += 1;
            return Ok(MiListV3::Values(Vec::new()));
        }
        if self.peek().is_some_and(is_name_byte) {
            let mut results = Vec::new();
            loop {
                let result = self.parse_result(depth + 1)?;
                results.push(result);
                match self.take() {
                    Some(b',') => {}
                    Some(b']') => return Ok(MiListV3::Results(results)),
                    _ => return Err(MiParseErrorV3::UnterminatedList),
                }
            }
        }
        let mut values = Vec::new();
        loop {
            self.count_field()?;
            values.push(self.parse_value(depth + 1)?);
            match self.take() {
                Some(b',') => {}
                Some(b']') => return Ok(MiListV3::Values(values)),
                _ => return Err(MiParseErrorV3::UnterminatedList),
            }
        }
    }

    fn parse_c_string(&mut self) -> Result<Vec<u8>, MiParseErrorV3> {
        if self.take() != Some(b'"') {
            return Err(MiParseErrorV3::InvalidString);
        }
        let mut output = Vec::new();
        loop {
            let byte = self.take().ok_or(MiParseErrorV3::UnterminatedString)?;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    let escaped = self.take().ok_or(MiParseErrorV3::UnterminatedString)?;
                    match escaped {
                        b'"' | b'\\' => output.push(escaped),
                        b'a' => output.push(0x07),
                        b'b' => output.push(0x08),
                        b'f' => output.push(0x0c),
                        b'n' => output.push(b'\n'),
                        b'r' => output.push(b'\r'),
                        b't' => output.push(b'\t'),
                        b'v' => output.push(0x0b),
                        b'0'..=b'7' => {
                            let mut value = escaped - b'0';
                            for _ in 0..2 {
                                let Some(next @ b'0'..=b'7') = self.peek() else {
                                    break;
                                };
                                self.cursor += 1;
                                value = value
                                    .checked_mul(8)
                                    .and_then(|current| current.checked_add(next - b'0'))
                                    .ok_or(MiParseErrorV3::InvalidEscape)?;
                            }
                            output.push(value);
                        }
                        b'x' => {
                            let high = self.take().and_then(hex_nibble);
                            let low = self.take().and_then(hex_nibble);
                            output.push(
                                high.zip(low)
                                    .map(|(high, low)| high << 4 | low)
                                    .ok_or(MiParseErrorV3::InvalidEscape)?,
                            );
                        }
                        _ => return Err(MiParseErrorV3::InvalidEscape),
                    }
                }
                value if value.is_ascii_control() => return Err(MiParseErrorV3::InvalidString),
                value => output.push(value),
            }
            if output.len() > self.limits.max_string_bytes {
                return Err(MiParseErrorV3::StringTooLarge);
            }
        }
    }
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MiParseErrorV3 {
    LimitOutOfRange(&'static str),
    Empty,
    LineTooLarge,
    StringTooLarge,
    MissingTerminator,
    EmbeddedLineBreak,
    InvalidToken,
    MissingRecordMarker,
    TokenOnStream,
    InvalidName,
    InvalidValue,
    InvalidString,
    InvalidEscape,
    UnterminatedString,
    UnterminatedTuple,
    UnterminatedList,
    MissingEquals,
    DuplicateVariable,
    FieldLimit,
    DepthLimit,
    TrailingInput,
}

impl std::fmt::Display for MiParseErrorV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid bounded GDB/MI record: {self:?}")
    }
}

impl std::error::Error for MiParseErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &[u8]) -> Result<MiRecordV3, MiParseErrorV3> {
        parse_mi_record_v3(line, MiParserLimitsV3::default())
    }

    #[test]
    fn classifies_result_async_stream_and_prompt_records() {
        assert!(matches!(
            parse(b"7^done,value=\"0x2a\"\n"),
            Ok(MiRecordV3::Result { token: Some(7), .. })
        ));
        assert!(matches!(
            parse(b"*stopped,reason=\"breakpoint-hit\"\n"),
            Ok(MiRecordV3::Async {
                kind: MiAsyncKindV3::Exec,
                ..
            })
        ));
        assert!(matches!(
            parse(b"=thread-created,id=\"1\"\n"),
            Ok(MiRecordV3::Async {
                kind: MiAsyncKindV3::Notify,
                ..
            })
        ));
        assert!(matches!(
            parse(b"~\"console\\ntext\"\n"),
            Ok(MiRecordV3::Stream {
                kind: MiStreamKindV3::Console,
                ..
            })
        ));
        assert_eq!(parse(b"(gdb)\n"), Ok(MiRecordV3::Prompt));
    }

    #[test]
    fn rejects_prose_duplicates_bad_escapes_and_tokens_on_streams() {
        for line in [
            b"Thread 1 stopped\n".as_slice(),
            b"1^done,value=\"a\",value=\"b\"\n",
            b"1^done,frame={addr=\"1\",addr=\"2\"}\n",
            b"~\"bad\\q\"\n",
            b"9~\"console\"\n",
            b"1^done trailing\n",
        ] {
            assert!(parse(line).is_err(), "accepted hostile record: {line:?}");
        }
    }

    #[test]
    fn enforces_line_field_string_and_depth_budgets() {
        let tiny = MiParserLimitsV3 {
            max_line_bytes: 32,
            max_string_bytes: 3,
            max_fields: 2,
            max_depth: 3,
        };
        assert_eq!(
            parse_mi_record_v3(b"1^done,value=\"abcd\"\n", tiny),
            Err(MiParseErrorV3::StringTooLarge)
        );
        assert_eq!(
            parse_mi_record_v3(b"1^done,a=\"1\",b=\"2\",c=\"3\"\n", tiny),
            Err(MiParseErrorV3::FieldLimit)
        );
        assert!(parse_mi_record_v3(b"1^done,a=[[[[\"x\"]]]]\n", tiny).is_err());
        assert_eq!(
            parse_mi_record_v3(b"1^done,value=\"01234567890123456789\"\n", tiny),
            Err(MiParseErrorV3::LineTooLarge)
        );
    }
}
