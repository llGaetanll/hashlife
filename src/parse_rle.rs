use std::str::FromStr;
use std::str::Utf8Error;
use thiserror::Error;
use tracing::warn;

use crate::WorldOffset;
use crate::parse_util::ParseError;
use crate::rule_set;
use crate::rule_set::RuleError;
use crate::rule_set::RuleSet;

use crate::parse_util;

#[derive(Default)]
pub struct RleFile<'a> {
    pub name: Option<&'a [u8]>,
    pub author: Option<&'a [u8]>,
    pub offset: Option<(WorldOffset, WorldOffset)>,
    pub set: RuleSet,
}

#[derive(Debug, Error)]
pub enum RleError {
    #[error("Comment line error: {0}")]
    CommentLine(#[from] RleCommentLineError),

    #[error("Header line error: {0}")]
    HeaderLine(#[from] RleHeaderLineError),

    #[error("Encoding error: {0}")]
    Encoding(#[from] RleEncodingError),
}

/// Parse the RLE file format. Assumes the bytes are valid Ascii.
///
/// See: https://conwaylife.com/wiki/Run_Length_Encoded
pub fn read_rle<F>(mut bytes: &'_ [u8], f: F) -> Result<RleFile<'_>, RleError>
where
    F: FnMut(WorldOffset, WorldOffset),
{
    let mut file = RleFile::default();

    // Parse as many comment lines as possible
    loop {
        let res = read_line_comment(bytes)?;
        let (Some(line), rest) = res else { break };

        match line {
            RleCommentLine::Comment => {}
            RleCommentLine::Name { name } => {
                if file.name.is_some() {
                    warn!("RLE file name already defined. Using latest");
                }

                file.name = Some(name);
            }
            RleCommentLine::Author { author } => {
                if file.author.is_some() {
                    warn!("RLE author already defined. Using latest");
                }

                file.author = Some(author);
            }
            RleCommentLine::Offset { x, y } => {
                if file.offset.is_some() {
                    warn!("RLE offset already defined. Using latest");
                }

                file.offset = Some((x, y))
            }
            RleCommentLine::RuleSet { set } => {
                file.set = set;
            }
        }

        bytes = rest;
    }

    // Parse header line, if it's present
    let res = read_line_header(bytes)?;
    if let (Some(header), rest) = res {
        let RleHeaderLine { x, y, .. } = header;
        if file.offset.is_some() {
            warn!("RLE offset already defined. Using latest");
        }

        file.offset = Some((x, y));
        bytes = rest;
    }

    let (dx, dy) = file.offset.unwrap_or_default();

    // Parse encoding
    read_encoding(bytes, dx, dy, f)?;

    Ok(file)
}

enum RleCommentLine<'a> {
    Comment,
    Name { name: &'a [u8] },
    Author { author: &'a [u8] },
    Offset { x: WorldOffset, y: WorldOffset },
    RuleSet { set: RuleSet },
}

#[derive(Debug, Error)]
pub enum RleCommentLineError {
    #[error("No comment type")]
    NoType,

    #[error("Empty name line")]
    EmptyName,

    #[error("Empty author line")]
    EmptyAuthor,

    #[error("Invalid rule: {0}")]
    InvalidRule(#[from] RuleError),

    #[error("Invalid coordinates: {0}")]
    InvalidCoord(#[from] RleCoordError),

    #[error("Invalid comment type, found '{got}'")]
    InvalidType { got: char },
}

/// Attempt to parse a comment line, otherwise leaves `bytes` as-is.
fn read_line_comment(
    bytes: &'_ [u8],
) -> Result<(Option<RleCommentLine<'_>>, &'_ [u8]), RleCommentLineError> {
    let Ok(bytes) = parse_util::expect(b'#', bytes) else {
        return Ok((None, bytes));
    };

    let (Some(b), bytes) = parse_util::take_1(bytes) else {
        return Err(RleCommentLineError::NoType);
    };

    match b {
        // Comment line
        b'C' | b'c' => {
            let (_, bytes) = parse_util::take_with(b'\n', bytes);

            Ok((Some(RleCommentLine::Comment), bytes))
        }

        // Pattern name
        b'N' => {
            let bytes = parse_util::take_ws(bytes);
            let (Some(name), bytes) = parse_util::take_with(b'\n', bytes) else {
                return Err(RleCommentLineError::EmptyName);
            };

            let line = RleCommentLine::Name { name };

            Ok((Some(line), bytes))
        }

        // Pattern author
        b'O' => {
            let bytes = parse_util::take_ws(bytes);
            let (Some(author), bytes) = parse_util::take_with(b'\n', bytes) else {
                return Err(RleCommentLineError::EmptyAuthor);
            };

            let line = RleCommentLine::Author { author };

            Ok((Some(line), bytes))
        }

        // Pattern offset
        b'R' | b'P' => {
            let bytes = parse_util::take_ws(bytes);
            let ((x, y), bytes) = read_coordinates(bytes)?;

            let line = RleCommentLine::Offset { x, y };

            Ok((Some(line), bytes))
        }

        // Pattern rules
        b'r' => {
            let bytes = parse_util::take_ws(bytes);
            let (rule, bytes) = rule_set::parse_nameless_rule(bytes)?;
            let bytes = parse_util::take_ws(bytes);

            let line = RleCommentLine::RuleSet { set: rule };

            Ok((Some(line), bytes))
        }

        b => Err(RleCommentLineError::InvalidType { got: b as char }),
    }
}

struct RleHeaderLine {
    x: WorldOffset,
    y: WorldOffset,
    set: Option<RuleSet>,
}

#[derive(Debug, Error)]
pub enum RleHeaderLineError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),

    #[error("Invalid token: expected ',' or '\n', found '{got}'")]
    InvalidToken { got: char },

    #[error("Invalid rule: {0}")]
    InvalidRule(#[from] RuleError),
}

/// Attempt to parse a header line, otherwise leaves `bytes` as-is.
fn read_line_header(bytes: &[u8]) -> Result<(Option<RleHeaderLine>, &[u8]), RleHeaderLineError> {
    let Ok(((x, y), bytes)) = read_coordinates(bytes) else {
        return Ok((None, bytes));
    };

    let (Some(b), bytes) = parse_util::take_1(bytes) else {
        unreachable!("read_coordinates internally takes until, so we haven't reached EOF")
    };

    match b {
        b',' => {
            let bytes = parse_util::take_ws(bytes);
            let bytes = parse_util::expect_slice("rule".as_bytes(), bytes)?;
            let bytes = parse_util::take_ws(bytes);
            let bytes = parse_util::expect(b'=', bytes)?;
            let bytes = parse_util::take_ws(bytes);

            let (rule, bytes) = rule_set::parse_rule(bytes)?;

            let line = RleHeaderLine {
                x,
                y,
                set: Some(rule),
            };

            Ok((Some(line), bytes))
        }
        b'\n' => {
            let line = RleHeaderLine { x, y, set: None };

            Ok((Some(line), bytes))
        }
        b => Err(RleHeaderLineError::InvalidToken { got: b as char }),
    }
}

#[derive(Debug, Error)]
pub enum RleEncodingError {
    #[error("Unexpected EOF")]
    UnexpectedEof,

    #[error("Failed to convert run length: {0}")]
    RunLength(#[from] ConvertError),

    #[error("Unrecognized byte: 0x{got:0X}")]
    UnrecognizedByte { got: u8 },
}

fn read_encoding<F>(
    mut bytes: &[u8],
    dx: WorldOffset,
    dy: WorldOffset,
    mut f: F,
) -> Result<(), RleEncodingError>
where
    F: FnMut(WorldOffset, WorldOffset),
{
    let mut rep: u64 = 1;

    let (mut x, mut y) = (0, 0);

    loop {
        let Some(b) = parse_util::peek_1(bytes) else {
            return Err(RleEncodingError::UnexpectedEof);
        };

        match b {
            b'\r' | b'\n' => {
                let (_, rest) = parse_util::take_1(bytes);
                bytes = rest;
            }

            // End of input
            b'!' => break,

            // Dead cell
            b'b' => {
                let (_, rest) = parse_util::take_1(bytes);
                bytes = rest;

                x += rep as WorldOffset;

                rep = 1;
            }

            // Live cell
            b'o' => {
                let (_, rest) = parse_util::take_1(bytes);
                bytes = rest;

                for i in 0..rep {
                    f(dx + x + i as WorldOffset, dy + y)
                }

                x += rep as WorldOffset;

                rep = 1;
            }

            // End of line
            b'$' => {
                let (_, rest) = parse_util::take_1(bytes);
                bytes = rest;

                y -= rep as WorldOffset;
                x = 0;

                rep = 1;
            }

            // NOTE: All numbers are > 1
            n if n.is_ascii_digit() => {
                let (Some(n), rest) = parse_util::take_until_fn(|b| !b.is_ascii_digit(), bytes)
                else {
                    unreachable!("We peeked and found a digit")
                };
                bytes = rest;

                if let Some(b'\n') = parse_util::peek_1(bytes) {
                    unreachable!("Repeat count cannot be cut off by a new line")
                };

                rep = convert(n).map_err(RleEncodingError::RunLength)?;
            }

            b => return Err(RleEncodingError::UnrecognizedByte { got: b }),
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum RleCoordError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),

    #[error("Expected x coordinate, found end of input")]
    NoX,

    #[error("Failed to parse x coordinate: {0}")]
    ParseX(#[source] ConvertError),

    #[error("Expected y coordinate, found end of input")]
    NoY,

    #[error("Failed to parse y coordinate: {0}")]
    ParseY(#[source] ConvertError),
}

fn read_coordinates(bytes: &[u8]) -> Result<((WorldOffset, WorldOffset), &[u8]), RleCoordError> {
    let bytes = parse_util::expect(b'x', bytes)?;
    let bytes = parse_util::take_ws(bytes);
    let bytes = parse_util::expect(b'=', bytes)?;
    let bytes = parse_util::take_ws(bytes);

    let (Some(x_bytes), bytes) = parse_util::take_with(b',', bytes) else {
        return Err(RleCoordError::NoX);
    };
    let x: WorldOffset = convert(x_bytes).map_err(RleCoordError::ParseX)?;

    let bytes = parse_util::take_ws(bytes);
    let bytes = parse_util::expect(b'y', bytes)?;
    let bytes = parse_util::take_ws(bytes);
    let bytes = parse_util::expect(b'=', bytes)?;
    let bytes = parse_util::take_ws(bytes);

    // Coordinates can be terminated with either `,` or `\n`.
    let p = |b| b == b',' || b == b'\n';
    let (Some(y_bytes), bytes) = parse_util::take_until_fn(p, bytes) else {
        return Err(RleCoordError::NoY);
    };
    let y: WorldOffset = convert(y_bytes).map_err(RleCoordError::ParseY)?;

    Ok(((x, y), bytes))
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("Error parsing bytes from UTF-8: {0}")]
    InvalidUTF8(Utf8Error),

    #[error("Failed to convert \"{str}\"")]
    ParseError { str: String },
}

/// Converts `&[u8]` to `T` if `T: FromStr`.
fn convert<T: FromStr>(bytes: &[u8]) -> Result<T, ConvertError> {
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
mod test {
    #[test]
    fn read_coordinates() {
        let bytes = b"x = 1, y = 1\n";
        super::read_coordinates(bytes.as_slice()).unwrap();
    }
}
