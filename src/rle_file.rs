use log::warn;
use thiserror::Error;

use crate::WorldOffset;
use crate::rle_data::RleBufWrite;
use crate::rule_set;
use crate::rule_set::RuleError;
use crate::rule_set::RuleSet;
use crate::util_parse::ParseError;

use crate::util_parse;

#[derive(Default)]
pub struct RleHeader<'a> {
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
pub fn read_rle<'a>(
    bytes: &'a [u8],
    buf: &'a mut dyn RleBufWrite,
) -> Result<RleHeader<'a>, RleError> {
    let mut header = RleHeader::default();
    let mut ruleset = None;

    let mut bytes = util_parse::take_ws_lines(bytes);

    // Parse as many comment lines as possible
    loop {
        let (Some(line), rest) = read_line_comment(bytes)? else {
            break;
        };

        let rest = util_parse::take_ws_lines(rest);

        match line {
            RleCommentLine::Comment => {}
            RleCommentLine::Name { name } => {
                if header.name.is_some() {
                    warn!("RLE file name already defined. Using latest");
                }

                header.name = Some(name);
            }
            RleCommentLine::Author { author } => {
                if header.author.is_some() {
                    warn!("RLE author already defined. Using latest");
                }

                header.author = Some(author);
            }
            RleCommentLine::Offset { x, y } => {
                if header.offset.is_some() {
                    warn!("RLE offset already defined. Using latest");
                }

                header.offset = Some((x, y))
            }
            RleCommentLine::RuleSet { set } => {
                ruleset = Some(set);
            }
        }

        bytes = rest;
    }

    // Parse header line, if it's present
    if let (Some(header_line), rest) = read_line_header(bytes)? {
        let RleHeaderLine { x, y, set } = header_line;

        if header.offset.is_some() {
            warn!("RLE offset already defined. Using latest");
        }

        header.offset = Some((x, y));
        bytes = rest;

        match (&mut ruleset, &set) {
            (_, None) => {}
            (None, Some(..)) => {
                ruleset = set;
            }
            (Some(..), Some(..)) => {
                warn!("RLE ruleset already defined. Using header line ruleset");

                ruleset = set;
            }
        }
    }

    header.set = ruleset.unwrap_or_default();

    let bytes = util_parse::take_ws_lines(bytes);

    // Parse encoding
    read_encoding(bytes, buf)?;

    Ok(header)
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
    let Ok(bytes) = util_parse::expect(b'#', bytes) else {
        return Ok((None, bytes));
    };

    let (Some(b), bytes) = util_parse::take_1(bytes) else {
        return Err(RleCommentLineError::NoType);
    };

    match b {
        // Comment line
        b'C' | b'c' => {
            let (_, bytes) = util_parse::take_with(b'\n', bytes);

            Ok((Some(RleCommentLine::Comment), bytes))
        }

        // Pattern name
        b'N' => {
            let bytes = util_parse::take_ws(bytes);
            let (Some(name), bytes) = util_parse::take_with(b'\n', bytes) else {
                return Err(RleCommentLineError::EmptyName);
            };

            let line = RleCommentLine::Name { name };

            Ok((Some(line), bytes))
        }

        // Pattern author
        b'O' => {
            let bytes = util_parse::take_ws(bytes);
            let (Some(author), bytes) = util_parse::take_with(b'\n', bytes) else {
                return Err(RleCommentLineError::EmptyAuthor);
            };

            let line = RleCommentLine::Author { author };

            Ok((Some(line), bytes))
        }

        // Pattern offset
        b'R' | b'P' => {
            let bytes = util_parse::take_ws(bytes);
            let ((x, y), bytes) = read_coordinates(bytes)?;

            let line = RleCommentLine::Offset { x, y };

            Ok((Some(line), bytes))
        }

        // Pattern rules
        b'r' => {
            let bytes = util_parse::take_ws(bytes);
            let (rule, bytes) = rule_set::parse_nameless_rule(bytes)?;
            let bytes = util_parse::take_ws(bytes);

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

    let (Some(b), bytes) = util_parse::take_1(bytes) else {
        unreachable!("read_coordinates internally takes until, so we haven't reached EOF")
    };

    match b {
        b',' => {
            let bytes = util_parse::take_ws(bytes);
            let bytes = util_parse::expect_slice("rule".as_bytes(), bytes)?;
            let bytes = util_parse::take_ws(bytes);
            let bytes = util_parse::expect(b'=', bytes)?;
            let bytes = util_parse::take_ws(bytes);

            let (rule, bytes) = match rule_set::parse_rule(bytes) {
                Ok((rule, bytes)) => (rule, bytes),
                Err(RuleError::NoBirths) => rule_set::parse_nameless_rule(bytes)?,
                Err(err) => return Err(err.into()),
            };

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
    RunLength(#[from] util_parse::ConvertError),

    #[error("Unrecognized byte: 0x{got:0X}")]
    UnrecognizedByte { got: u8 },
}

fn read_encoding(mut bytes: &[u8], buf: &mut dyn RleBufWrite) -> Result<(), RleEncodingError> {
    let mut rep: u64 = 1;

    loop {
        let Some(b) = util_parse::peek_1(bytes) else {
            return Err(RleEncodingError::UnexpectedEof);
        };

        match b {
            b'\r' | b'\n' | b' ' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;
            }

            // End of input
            b'!' => {
                buf.eof();

                break;
            }

            // Dead cell
            b'b' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                buf.dead_cell(rep);

                rep = 1;
            }

            // Live cell
            b'o' | b'x' | b'y' | b'z' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                buf.live_cell(rep);

                rep = 1;
            }

            // End of line
            b'$' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                buf.line_break(rep);

                rep = 1;
            }

            // NOTE: All numbers are > 1
            n if n.is_ascii_digit() => {
                let (Some(n), rest) = util_parse::take_until_fn(|b| !b.is_ascii_digit(), bytes)
                else {
                    unreachable!("We peeked and found a digit")
                };
                bytes = rest;

                if let Some(b'\n') = util_parse::peek_1(bytes) {
                    unreachable!("Repeat count cannot be cut off by a new line")
                };

                rep = util_parse::convert(n).map_err(RleEncodingError::RunLength)?;
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
    ParseX(#[source] util_parse::ConvertError),

    #[error("Expected y coordinate, found end of input")]
    NoY,

    #[error("Failed to parse y coordinate: {0}")]
    ParseY(#[source] util_parse::ConvertError),
}

fn read_coordinates(bytes: &[u8]) -> Result<((WorldOffset, WorldOffset), &[u8]), RleCoordError> {
    let bytes = util_parse::expect(b'x', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;
    let bytes = util_parse::take_ws(bytes);

    let (Some(x_bytes), bytes) = util_parse::take_with(b',', bytes) else {
        return Err(RleCoordError::NoX);
    };
    let x: WorldOffset = util_parse::convert(x_bytes).map_err(RleCoordError::ParseX)?;

    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'y', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;
    let bytes = util_parse::take_ws(bytes);

    // Coordinates can be terminated with either `,` or `\n`.
    let p = |b| b == b',' || b == b'\n';
    let (Some(y_bytes), bytes) = util_parse::take_until_fn(p, bytes) else {
        return Err(RleCoordError::NoY);
    };
    let y: WorldOffset = util_parse::convert(y_bytes).map_err(RleCoordError::ParseY)?;

    Ok(((x, y), bytes))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_read_coordinates() {
        let bytes = b"x = 1, y = 1\n";
        super::read_coordinates(bytes.as_slice()).unwrap();
    }
}
