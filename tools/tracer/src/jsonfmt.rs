//! Central JSON serialization — the wire and on-disk format.
//!
//! The format is `ensure_ascii`-style: every non-ASCII scalar is emitted
//! as a `\uXXXX` escape (astral chars as a UTF-16 surrogate pair), with
//! `", "` / `": "` separators and 2-space indent for pretty output.
//! serde_json emits raw UTF-8 by default; this module is the single place
//! that fixes the representation so every `--json` command and every
//! on-disk cache entry shares one stable byte format.
//!
//! Functional content is unchanged — this is representation only.

use serde_json::ser::{CharEscape, Formatter};
use serde_json::Value;
use std::io::{self, Write};

/// Escape policy: every non-ASCII scalar emitted as a `\uXXXX` escape.
struct AsciiEscaper<F> {
    inner: F,
}

impl<F: Formatter> Formatter for AsciiEscaper<F> {
    fn write_null<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.write_null(w)
    }
    fn write_bool<W: ?Sized + Write>(&mut self, w: &mut W, v: bool) -> io::Result<()> {
        self.inner.write_bool(w, v)
    }
    fn write_i64<W: ?Sized + Write>(&mut self, w: &mut W, v: i64) -> io::Result<()> {
        self.inner.write_i64(w, v)
    }
    fn write_u64<W: ?Sized + Write>(&mut self, w: &mut W, v: u64) -> io::Result<()> {
        self.inner.write_u64(w, v)
    }
    fn write_f64<W: ?Sized + Write>(&mut self, w: &mut W, v: f64) -> io::Result<()> {
        self.inner.write_f64(w, v)
    }
    fn write_number_str<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        v: &str,
    ) -> io::Result<()> {
        self.inner.write_number_str(w, v)
    }
    fn begin_string<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_string(w)
    }
    fn end_string<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_string(w)
    }

    /// The load-bearing override: serde hands us each run of non-escaped
    /// bytes as a `&str`. The format requires EVERY non-ASCII scalar to be
    /// escaped, so we re-emit ASCII verbatim and every codepoint >= 0x80 as
    /// `\uXXXX` (astral planes as a UTF-16 surrogate pair).
    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut start = 0;
        let bytes = fragment.as_bytes();
        for (i, ch) in fragment.char_indices() {
            if ch.is_ascii() {
                continue;
            }
            if start < i {
                w.write_all(&bytes[start..i])?;
            }
            let cp = ch as u32;
            if cp <= 0xFFFF {
                write!(w, "\\u{cp:04x}")?;
            } else {
                let v = cp - 0x1_0000;
                let hi = 0xD800 + (v >> 10);
                let lo = 0xDC00 + (v & 0x3FF);
                write!(w, "\\u{hi:04x}\\u{lo:04x}")?;
            }
            start = i + ch.len_utf8();
        }
        if start < bytes.len() {
            w.write_all(&bytes[start..])?;
        }
        Ok(())
    }

    fn write_char_escape<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        ce: CharEscape,
    ) -> io::Result<()> {
        self.inner.write_char_escape(w, ce)
    }

    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_array(w)
    }
    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array(w)
    }
    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_array_value(w, first)
    }
    fn end_array_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array_value(w)
    }
    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_object(w)
    }
    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object(w)
    }
    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_object_key(w, first)
    }
    fn end_object_key<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object_key(w)
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_object_value(w)
    }
    fn end_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object_value(w)
    }
}

/// Pretty form: 2-space indent, ASCII-escaped scalars. Under indent the
/// item separator is "," (a newline follows) and the key separator is ": ";
/// serde_json's PrettyFormatter with a 2-space indent produces exactly this.
pub fn to_pretty(value: &Value) -> String {
    let indent = b"  ";
    let pretty = serde_json::ser::PrettyFormatter::with_indent(indent);
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        AsciiEscaper { inner: pretty },
    );
    serde::Serialize::serialize(value, &mut ser).expect("json pretty");
    String::from_utf8(buf).expect("utf8")
}

/// Compact form, no indent (used for cache entries): the separators are
/// `", "` and `": "`. serde_json's built-in CompactFormatter uses "," and
/// ":", which does not match this format, so we wrap a separator-faithful
/// formatter.
pub fn to_compact(value: &Value) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        AsciiEscaper {
            inner: CompactSeparatorFormatter::default(),
        },
    );
    serde::Serialize::serialize(value, &mut ser).expect("json compact");
    String::from_utf8(buf).expect("utf8")
}

/// Compact separators required by the format (no indent): item `", "`,
/// key `": "`. serde_json's built-in CompactFormatter uses "," / ":" which
/// does NOT match; this formatter does.
#[derive(Default)]
struct CompactSeparatorFormatter;

impl Formatter for CompactSeparatorFormatter {
    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }
    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }
    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }
}
