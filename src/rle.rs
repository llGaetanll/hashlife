use anyhow::bail;
use anyhow::Context;

use crate::rules::RuleSet;
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
    while let (Some(line), rest) = read_line_comment(bytes)? {
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
    if let (Some(header), rest) = read_line_header(bytes)? {
        let RleHeaderLine { x, y } = header;
        if file.offset.is_some() {
            bail!("Rle file offset already defined")
        }

        file.offset = Some((x, y));
        bytes = rest;
    }

    let (x, y) = file.offset.unwrap_or((0, 0));

    // Parse encoding
    read_encoding(bytes, x, y, f)?;

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
            bail!("Pattern rules not yet supported")
        }

        c => {
            bail!("Unrecognized comment type '{c}'")
        }
    }
}

struct RleHeaderLine {
    x: WorldOffset,
    y: WorldOffset,
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
            todo!("Parse rules")
        }
        b'\n' => {
            let line = RleHeaderLine { x, y };
            Ok((Some(line), bytes))
        }
        b => bail!("Invalid token: expected ',' or '\n', found '{b}'"),
    }
}

fn read_encoding<F>(
    bytes: &[u8],
    mut dx: WorldOffset,
    mut dy: WorldOffset,
    mut f: F,
) -> util_parse::ParseResult<()>
where
    F: FnMut(WorldOffset, WorldOffset),
{
    let mut rep: u64 = 1;

    loop {
        let (Some(b), bytes) = util_parse::take_1(bytes) else {
            bail!("Unexpected end of input")
        };

        match b {
            b'\n' => {}

            // End of input
            b'!' => break,

            // Dead cell
            b'b' => {
                dx += rep as WorldOffset;

                rep = 1;
            }

            // Live cell
            b'o' => {
                for i in 0..rep {
                    f(dx + i as WorldOffset, dy)
                }

                rep = 1;
            }

            // End of line
            b'$' => {
                dy += rep as WorldOffset;

                rep = 1;
            }

            // NOTE: All numbers are > 1
            n if n.is_ascii_digit() => {
                let (Some(n), bytes) = util_parse::take_until_fn(|b| b.is_ascii_digit(), bytes)
                else {
                    unreachable!("We peeked and found a digit")
                };

                if let Some(b'\n') = util_parse::peek_1(bytes) {
                    bail!("Repeat count cannot be cut off by a new line")
                };

                rep = util_parse::convert(n)?;
            }

            b => bail!("Unrecognized character '{b}'"),
        }
    }

    Ok(())
}

fn read_coordinates(bytes: &[u8]) -> util_parse::ParseResult<((WorldOffset, WorldOffset), &[u8])> {
    let bytes = util_parse::expect(b'y', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;
    let bytes = util_parse::take_ws(bytes);

    let (Some(x_bytes), bytes) = util_parse::take_with(b',', bytes) else {
        panic!("Expected x coordinate, found end of input")
    };
    let x: WorldOffset = util_parse::convert(x_bytes).context("Failed to parse x offset")?;

    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'y', bytes)?;
    let bytes = util_parse::take_ws(bytes);
    let bytes = util_parse::expect(b'=', bytes)?;

    // Coordinates can be terminated with either `,` or `\n`.
    let p = |b| b == b',' || b == b'\n';
    let (Some(y_bytes), bytes) = util_parse::take_until_fn(p, bytes) else {
        panic!("Expected x coordinate, found end of input")
    };
    let y: WorldOffset = util_parse::convert(y_bytes).context("Failed to parse y offset")?;

    Ok(((x, y), bytes))
}
