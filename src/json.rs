//! JSON emission uses an explicit work stack. A long binary or member-access
//! chain cannot overflow the native stack while being serialized.
use crate::{Ast, NodeId, Value};
use std::io::{self, Write};

enum Action<'ast, 'src> {
    Node(NodeId, usize),
    Field(&'static str, &'ast Value<'src>, usize),
    Element(NodeId, usize, bool),
    CloseObject(usize),
    CloseArray(usize, bool),
}

impl Ast<'_> {
    pub fn write_json(&self, mut writer: impl Write, pretty: bool) -> io::Result<()> {
        let mut work = vec![Action::Node(self.root, 0)];
        while let Some(action) = work.pop() {
            match action {
                Action::Node(id, depth) => {
                    let node = self.node(id);
                    writer.write_all(b"{")?;
                    for (index, (key, value)) in [
                        ("kind", node.syntax_kind.number() as usize),
                        ("pos", self.utf16_offset(node.span.start)),
                        ("end", self.utf16_offset(node.span.end)),
                        ("flags", node.flags as usize),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        prefix(&mut writer, key, depth + 1, pretty, index > 0)?;
                        write!(writer, "{value}")?;
                    }
                    work.push(Action::CloseObject(depth));
                    for (key, value) in node.fields.iter().rev() {
                        // The public tree is the written syntax.
                        if !matches!(value, Value::Node(id) if self.is_reparsed(*id)) {
                            work.push(Action::Field(key, value, depth + 1));
                        }
                    }
                }
                Action::Field(key, value, depth) => {
                    prefix(&mut writer, key, depth, pretty, true)?;
                    match value {
                        Value::Node(id) => work.push(Action::Node(*id, depth)),
                        Value::Nodes(ids) => {
                            writer.write_all(b"[")?;
                            work.push(Action::CloseArray(depth, !ids.is_empty()));
                            for (index, id) in ids.iter().enumerate().rev() {
                                work.push(Action::Element(*id, depth + 1, index > 0));
                            }
                        }
                        Value::Text(text) => string(&mut writer, text)?,
                        Value::Utf16(units) => {
                            writer.write_all(b"\"")?;
                            for unit in units {
                                write!(writer, "\\u{unit:04x}")?;
                            }
                            writer.write_all(b"\"")?;
                        }
                        Value::Bool(value) => {
                            writer.write_all(if *value { b"true" } else { b"false" })?
                        }
                        Value::Number(value) => write!(writer, "{value}")?,
                    }
                }
                Action::Element(id, depth, comma) => {
                    if comma {
                        writer.write_all(b",")?;
                    }
                    if pretty {
                        newline(&mut writer, depth)?;
                    }
                    work.push(Action::Node(id, depth));
                }
                Action::CloseObject(depth) => {
                    if pretty {
                        newline(&mut writer, depth)?;
                    }
                    writer.write_all(b"}")?;
                }
                Action::CloseArray(depth, nonempty) => {
                    if pretty && nonempty {
                        newline(&mut writer, depth)?;
                    }
                    writer.write_all(b"]")?;
                }
            }
        }
        Ok(())
    }
}

fn string(writer: &mut impl Write, value: &str) -> io::Result<()> {
    serde_json::to_writer(writer, value).map_err(io::Error::other)
}
fn newline(writer: &mut impl Write, depth: usize) -> io::Result<()> {
    writer.write_all(b"\n")?;
    let spaces = [b' '; 64];
    let mut count = depth * 2;
    while count > 0 {
        let length = count.min(spaces.len());
        writer.write_all(&spaces[..length])?;
        count -= length;
    }
    Ok(())
}
fn prefix(
    writer: &mut impl Write,
    key: &str,
    depth: usize,
    pretty: bool,
    comma: bool,
) -> io::Result<()> {
    if comma {
        writer.write_all(b",")?;
    }
    if pretty {
        newline(writer, depth)?;
    }
    string(writer, key)?;
    writer.write_all(if pretty { b": " } else { b":" })
}
