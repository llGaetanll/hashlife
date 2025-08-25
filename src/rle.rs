use std::str::FromStr;

use anyhow::bail;
use anyhow::Context;

use crate::rule_set::RuleSet;
use crate::util_parse::ParseResult;
use crate::WorldOffset;

use crate::util_parse;

#[derive(Default)]
pub struct RleFile<'a> {
    pub name: Option<&'a [u8]>,
    pub author: Option<&'a [u8]>,
    pub offset: Option<(WorldOffset, WorldOffset)>,
    pub set: RuleSet,
}

/// Parse the RLE file format. Assumes the bytes are valid Ascii.
///
/// See: https://conwaylife.com/wiki/Run_Length_Encoded
pub fn read_rle<F>(mut bytes: &[u8], f: F) -> ParseResult<RleFile>
where
    F: FnMut(WorldOffset, WorldOffset),
{
    let mut file = RleFile::default();

    // Parse as many comment lines as possible
    loop {
        let res = read_line_comment(bytes).context("Failed to read comment line")?;
        let (Some(line), rest) = res else { break };

        match line {
            RleCommentLine::Comment => {}
            RleCommentLine::Name { name } => {
                if file.name.is_some() {
                    bail!("Rle file name already defined")
                }

                file.name = Some(name);
            }
            RleCommentLine::Author { author } => {
                if file.author.is_some() {
                    bail!("Rle file author already defined")
                }

                file.author = Some(author);
            }
            RleCommentLine::Offset { x, y } => {
                if file.offset.is_some() {
                    bail!("Rle file offset already defined")
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
    let res = read_line_header(bytes).context("Failed to read header line")?;
    if let (Some(header), rest) = res {
        let RleHeaderLine { x, y, .. } = header;
        if file.offset.is_some() {
            bail!("Rle file offset already defined")
        }

        file.offset = Some((x, y));
        bytes = rest;
    }

    let (dx, dy) = file.offset.unwrap_or_default();

    // Parse encoding
    read_encoding(bytes, dx, dy, f).context("Failed to read encoding")?;

    Ok(file)
}

enum RleCommentLine<'a> {
    Comment,
    Name { name: &'a [u8] },
    Author { author: &'a [u8] },
    Offset { x: WorldOffset, y: WorldOffset },
    RuleSet { set: RuleSet },
}

/// Attempt to parse a comment line, otherwise leaves `bytes` as-is.
fn read_line_comment(bytes: &[u8]) -> util_parse::ParseResult<(Option<RleCommentLine>, &[u8])> {
    let Ok(bytes) = util_parse::expect(b'#', bytes) else {
        return Ok((None, bytes));
    };

    let (Some(b), bytes) = util_parse::take_1(bytes) else {
        bail!("No comment type");
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
                bail!("Empty name line")
            };

            let line = RleCommentLine::Name { name };

            Ok((Some(line), bytes))
        }

        // Pattern author
        b'O' => {
            let bytes = util_parse::take_ws(bytes);
            let (Some(author), bytes) = util_parse::take_with(b'\n', bytes) else {
                bail!("Empty author line")
            };

            let line = RleCommentLine::Author { author };

            Ok((Some(line), bytes))
        }

        // Pattern offset
        b'R' | b'P' => {
            let bytes = util_parse::take_ws(bytes);
            let Ok(((x, y), bytes)) = read_coordinates(bytes) else {
                bail!("Invalid coordinates")
            };

            let line = RleCommentLine::Offset { x, y };

            Ok((Some(line), bytes))
        }

        // Pattern rules
        b'r' => {
            bail!("Comment pattern rules not yet supported")
        }

        b => {
            bail!("Unrecognized comment type '{}'", b as char)
        }
    }
}

struct RleHeaderLine {
    x: WorldOffset,
    y: WorldOffset,
    set: Option<RuleSet>,
}

/// Attempt to parse a header line, otherwise leaves `bytes` as-is.
fn read_line_header(bytes: &[u8]) -> util_parse::ParseResult<(Option<RleHeaderLine>, &[u8])> {
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

            let (Some(rule), bytes) = util_parse::take_until_ws(bytes) else {
                bail!("Expected rule, found end of input")
            };

            let Ok(rule) = std::str::from_utf8(rule) else {
                bail!("Failed to convert rule to utf-8")
            };

            let Ok(rule) = RuleSet::from_str(rule) else {
                bail!("Invalid rule: \"{}\"", rule)
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
        b => bail!("Invalid token: expected ',' or '\n', found '{}'", b as char),
    }
}

fn read_encoding<F>(
    mut bytes: &[u8],
    dx: WorldOffset,
    dy: WorldOffset,
    mut f: F,
) -> util_parse::ParseResult<()>
where
    F: FnMut(WorldOffset, WorldOffset),
{
    let mut rep: u64 = 1;

    let (mut x, mut y) = (0, 0);

    loop {
        let Some(b) = util_parse::peek_1(bytes) else {
            bail!("Unexpected end of input")
        };

        match b {
            b'\n' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;
            }

            // End of input
            b'!' => break,

            // Dead cell
            b'b' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                x += rep as WorldOffset;

                rep = 1;
            }

            // Live cell
            b'o' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                for i in 0..rep {
                    f(dx + x + i as WorldOffset, dy + y)
                }

                x += rep as WorldOffset;

                rep = 1;
            }

            // End of line
            b'$' => {
                let (_, rest) = util_parse::take_1(bytes);
                bytes = rest;

                y -= rep as WorldOffset;
                x = 0;

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
                    bail!("Repeat count cannot be cut off by a new line")
                };

                rep = util_parse::convert(n).context("Failed to convert run length")?;
            }

            b => bail!("Unrecognized character '{}'", b as char),
        }
    }

    Ok(())
}

fn read_coordinates(bytes: &[u8]) -> util_parse::ParseResult<((WorldOffset, WorldOffset), &[u8])> {
    let bytes = util_parse::expect(b'x', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;
    let bytes = util_parse::take_ws(bytes);

    let (Some(x_bytes), bytes) = util_parse::take_with(b',', bytes) else {
        bail!("Expected x coordinate, found end of input")
    };
    let x: WorldOffset = util_parse::convert(x_bytes).context("Failed to parse x offset")?;

    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'y', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;
    let bytes = util_parse::take_ws(bytes);

    // Coordinates can be terminated with either `,` or `\n`.
    let p = |b| b == b',' || b == b'\n';
    let (Some(y_bytes), bytes) = util_parse::take_until_fn(p, bytes) else {
        bail!("Expected y coordinate, found end of input")
    };
    let y: WorldOffset = util_parse::convert(y_bytes).context("Failed to parse y offset")?;

    Ok(((x, y), bytes))
}

#[cfg(test)]
mod test {
    #[test]
    fn read_coordinates() {
        let bytes = b"x = 1, y = 1\n";
        super::read_coordinates(bytes.as_slice()).unwrap();
    }
}
