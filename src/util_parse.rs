use std::str::FromStr;

use anyhow::bail;

pub type ParseResult<T> = anyhow::Result<T>;

/// Consumes the slice until a non-ascii whitespace character is reached.
pub fn take_ws(bytes: &[u8]) -> &[u8] {
    let mut i = bytes.len() - 1;
    for (j, b) in bytes.iter().enumerate() {
        if b.is_ascii_whitespace() {
            continue;
        }

        i = j;
        break;
    }

    &bytes[i..]
}

/// Takes the next character from the slice. If none is found, the slice is left as-is.
pub fn take_1(bytes: &[u8]) -> (Option<u8>, &[u8]) {
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
pub fn split_n(bytes: &[u8], n: usize) -> (Option<&[u8]>, &[u8]) {
    let Some((res, bytes)) = bytes.split_at_checked(n) else {
        return (None, bytes);
    };

    (Some(res), bytes)
}

/// Like `split_n`, but doesn't consume the slice
pub fn peek_n(bytes: &[u8], n: usize) -> Option<&[u8]> {
    let (res, _) = bytes.split_at_checked(n)?;

    Some(res)
}

/// Expects the next character in `bytes` to be `b`. Otherwise leaves `bytes` unchanged.
pub fn expect(b: u8, bytes: &[u8]) -> ParseResult<&[u8]> {
    let (Some(a), bytes) = take_1(bytes) else {
        bail!("Expected '{}', found end of input", b as char)
    };

    if a != b {
        bail!("Expected '{}', found '{}'", b as char, a as char)
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

        bail!(
            "Expected \"{}\", found \"{}\"",
            String::from_utf8_lossy(bs),
            String::from_utf8_lossy(&bytes[..n]),
        )
    }
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

/// Advance the slice until byte `b` is found. If `b` is never found, `bytes` is left as-is.
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

/// Converts `&[u8]` to `T` if `T: FromStr`.
pub fn convert<T: FromStr>(bytes: &[u8]) -> ParseResult<T> {
    let str = str::from_utf8(bytes)?;

    let Ok(res) = str.parse::<T>() else {
        bail!("Failed to convert bytes: '{str}'")
    };

    Ok(res)
}
