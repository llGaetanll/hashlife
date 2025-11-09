use std::str::FromStr;
use std::str::Utf8Error;

use thiserror::Error;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected end of file, expected '{exp}'")]
    UnexpectedEof { exp: char },

    #[error("Expected '{exp}', but got '{got}'")]
    UnexpectedToken { exp: char, got: char },

    #[error("Expected \"{exp}\", but got \"{got}\"")]
    UnexpectedSlice { exp: String, got: String },

    #[error("Unequal inputs. Got \"{left}\" and \"{right}\"")]
    UnequalInputs { left: String, right: String },
}

/// Consumes the slice until a non-ascii whitespace character is reached.
pub fn take_ws(bytes: &[u8]) -> &[u8] {
    let mut i = bytes.len();
    for (j, b) in bytes.iter().enumerate() {
        if b.is_ascii_whitespace() {
            continue;
        }

        i = j;
        break;
    }

    &bytes[i..]
}

/// Like `take_ws` but stops at a linebreak or non-ascii whitespace character. Returns whether any
/// bytes were consumed.
///
/// A linebreak is any of
/// * `\n`
/// * `\r`
/// * `\r\n`
///
/// This function completely consumes the linebreak.
pub fn take_ws_line(bytes: &[u8]) -> (bool, &[u8]) {
    let mut i = bytes.len();

    for (j, b) in bytes.iter().enumerate() {
        match b {
            b'\n' => {
                i = j + 1;

                break;
            }
            b'\r' => {
                if let Some(b'\n') = peek_1(&bytes[j..]) {
                    i = j + 2;
                } else {
                    i = j + 1;
                }

                break;
            }
            w if w.is_ascii_whitespace() => {}
            _ => {
                i = j;

                break;
            }
        }
    }

    let bytes = &bytes[i..];
    let consumed = i != 0;

    (consumed, bytes)
}

/// Like `take_ws_line` but consumes as many blank lines as possible
pub fn take_ws_lines(mut bytes: &[u8]) -> &[u8] {
    while let (true, rest) = take_ws_line(bytes) {
        bytes = rest;
    }

    bytes
}

/// Takes the next character from the slice. If none is found, the slice is left as-is.
pub const fn take_1(bytes: &[u8]) -> (Option<u8>, &[u8]) {
    let [b, bytes @ ..] = bytes else {
        return (None, bytes);
    };

    (Some(*b), bytes)
}

/// Like `take_1`, but doesn't consume the token
pub fn peek_1(bytes: &[u8]) -> Option<u8> {
    let [b, _bytes @ ..] = bytes else { return None };

    Some(*b)
}

/// Split `bytes` as (&bytes[..n], &bytes[n..]). If `n > bytes.len()`, leaves `bytes` as-is.
pub const fn split_n(bytes: &[u8], n: usize) -> (Option<&[u8]>, &[u8]) {
    let Some((res, bytes)) = bytes.split_at_checked(n) else {
        return (None, bytes);
    };

    (Some(res), bytes)
}

/// Like `split_n`, but doesn't consume the slice
pub const fn peek_n(bytes: &[u8], n: usize) -> Option<&[u8]> {
    let Some((res, _)) = bytes.split_at_checked(n) else {
        return None;
    };

    Some(res)
}

/// Expects the next character in `bytes` to be `b`. Otherwise leaves `bytes` unchanged.
pub fn expect(b: u8, bytes: &[u8]) -> ParseResult<&[u8]> {
    let (Some(a), bytes) = take_1(bytes) else {
        return Err(ParseError::UnexpectedEof { exp: b as char });
    };

    if a != b {
        return Err(ParseError::UnexpectedToken {
            exp: b as char,
            got: a as char,
        });
    }

    Ok(bytes)
}

/// Expects the next character in `bytes` to be `b`. Otherwise leaves `bytes` unchanged.
pub fn expect_slice<'a>(bs: &[u8], bytes: &'a [u8]) -> ParseResult<&'a [u8]> {
    if bytes.starts_with(bs) {
        // SAFETY: bytes starts with bs, so 0 <= bs.len() <= bytes.len();
        let (_, bytes) = unsafe { bytes.split_at_unchecked(bs.len()) };

        Ok(bytes)
    } else {
        let n = bs.len().min(bytes.len());

        Err(ParseError::UnexpectedSlice {
            exp: String::from_utf8_lossy(bs).to_string(),
            got: String::from_utf8_lossy(&bytes[..n]).to_string(),
        })
    }
}

/// Just like `expect`, except `bytes.len() == 1`.
///
/// In other words, checks that `b == bytes`.
pub fn is(b: u8, bytes: &[u8]) -> ParseResult<()> {
    expect(b, bytes).and_then(|_| {
        if bytes.len() == 1 {
            Ok(())
        } else {
            Err(ParseError::UnequalInputs {
                left: (b as char).to_string(),
                right: String::from_utf8_lossy(bytes).to_string(),
            })
        }
    })
}

/// Just like `expect_slice`, except `bs.len() == bytes.len()`.
///
/// In other words, checks that `bs == bytes`.
pub fn is_slice(bs: &[u8], bytes: &[u8]) -> ParseResult<()> {
    expect_slice(bs, bytes).and_then(|_| {
        if bs.len() == bytes.len() {
            Ok(())
        } else {
            Err(ParseError::UnequalInputs {
                left: String::from_utf8_lossy(bs).to_string(),
                right: String::from_utf8_lossy(bytes).to_string(),
            })
        }
    })
}

/// Advance the slice until `P` is satisfied, without consuming it.
#[inline]
pub fn take_until_fn<P>(p: P, bytes: &[u8]) -> (Option<&[u8]>, &[u8])
where
    P: Fn(u8) -> bool,
{
    let mut i = 0;
    for (j, &a) in bytes.iter().enumerate() {
        if !p(a) {
            continue;
        }

        i = j;
        break;
    }

    if i == 0 {
        (None, bytes)
    } else {
        // SAFETY: 0 <= i < bytes.len()
        let (res, bytes) = unsafe { bytes.split_at_unchecked(i) };

        (Some(res), bytes)
    }
}

/// Advance the slice until byte `b` is found, without consuming it.
///
/// If `b` is never found, `bytes` is left as-is.
pub fn take_until(b: u8, bytes: &[u8]) -> (Option<&[u8]>, &[u8]) {
    take_until_fn(|a| a == b, bytes)
}

/// Like `take_util`, but stops at the first ascii whitespace character found, without consuming it.
pub fn take_until_ws(bytes: &[u8]) -> (Option<&[u8]>, &[u8]) {
    take_until_fn(|a| a.is_ascii_whitespace(), bytes)
}

/// Like `take_until_fn`, but also consumes the first byte satisfying `P`.
#[inline]
pub fn take_with_fn<P>(p: P, bytes: &[u8]) -> (Option<&[u8]>, &[u8])
where
    P: Fn(u8) -> bool,
{
    let (Some(res), bytes) = take_until_fn(p, bytes) else {
        return (None, bytes);
    };

    let (_, bytes) = take_1(bytes);

    (Some(res), bytes)
}

/// Like `take_until`, but also consumes `b` without adding it to the output.
pub fn take_with(b: u8, bytes: &[u8]) -> (Option<&[u8]>, &[u8]) {
    let (Some(res), bytes) = take_until(b, bytes) else {
        return (None, bytes);
    };

    let (_, bytes) = take_1(bytes);

    (Some(res), bytes)
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("Error parsing bytes from UTF-8: {0}")]
    InvalidUTF8(Utf8Error),

    #[error("Failed to convert \"{str}\"")]
    ParseError { str: String },
}

/// Converts `&[u8]` to `T` if `T: FromStr`.
pub fn convert<T: FromStr>(bytes: &[u8]) -> Result<T, ConvertError> {
    let Ok(str) = str::from_utf8(bytes) else {
        unreachable!("RLE file is expected to be valid UTF-8")
    };

    let Ok(res) = str.parse::<T>() else {
        return Err(ConvertError::ParseError {
            str: str.to_string(),
        });
    };

    Ok(res)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_take_ws_full_ws() {
        let bytes = b"  ";

        let res = super::take_ws(bytes);

        assert_eq!(res, b"")
    }
}
