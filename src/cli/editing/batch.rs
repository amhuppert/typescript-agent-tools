//! Decoding and validating an edit batch file, the one owner of what a
//! batch may hold: `{"edits": [...]}`, each edit an object with an
//! `operation`, exactly one of `name` and `at`, an optional `file`, and
//! exactly one of `fragment` and `from`, where a member that is `null` is
//! absent and any other member is an error.
//!
//! A batch is decoded in two stages so that one run reports every problem.
//! First its text is read as JSON, the only stage that fails with a single
//! issue: serde's, at the line and column it stopped. Then every member of
//! the document and of each edit is checked, and each problem is an issue
//! named by its JSON path with zero-based indexes (`edits[1].extra`). Only a
//! batch with no issue becomes edits. Every failure names the help that
//! describes the format.

use super::{Fragment, Pending};
use crate::cli::failure::Code;
use crate::cli::help;
use crate::cli::registry::OperationId;
use crate::cli::request::{self, Unlocated};
use crate::cli::{Failure, Issue};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use typescript_parser::editing::Operation;
use typescript_parser::navigation::ShowQuery;

/// The edits the batch file at `path` names, in order, or the failure that
/// says why it names none: `input.read` when it cannot be read, and
/// `input.batch`, listing every issue, when it is not a valid batch.
pub(super) fn read(path: &Path) -> Result<Vec<Pending>, Failure> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| Failure::new(Code::Read, format!("{}: {error}", path.display())))?;
    decode(path, &text)
}

/// The edits `text`, the batch file at `path`, names, or the `input.batch`
/// failure that lists every issue.
fn decode(path: &Path, text: &str) -> Result<Vec<Pending>, Failure> {
    let document: Node = serde_json::from_str(text).map_err(|error| {
        invalid(
            path,
            "not JSON",
            vec![Issue {
                path: "$".into(),
                message: error.to_string(),
            }],
        )
    })?;
    let mut found = Found::default();
    let edits = batch(&document, &mut found);
    match found.summary() {
        None => Ok(edits),
        Some(summary) => Err(invalid(path, &summary, found.issues)),
    }
}

/// The `input.batch` failure of the batch at `path`.
fn invalid(path: &Path, summary: &str, issues: Vec<Issue>) -> Failure {
    Failure::new(Code::Batch, format!("{}: {summary}", path.display()))
        .with_issues(issues)
        .with_next(help::help_argv(OperationId::EditBatch.operation()))
}

/// A JSON value as written. An object keeps every member in order, one
/// given twice included, which `serde_json::Value` would keep only once, so
/// the validator can refuse it as decoding into a struct did.
enum Node {
    Null,
    /// A boolean or a number, which no member of a batch takes.
    Scalar,
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Scalar)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Node, E> {
        Ok(Node::String(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<Node, E> {
        Ok(Node::String(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Node, A::Error> {
        let mut nodes = Vec::new();
        while let Some(node) = items.next_element()? {
            nodes.push(node);
        }
        Ok(Node::Array(nodes))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Node, A::Error> {
        let mut members = Vec::new();
        while let Some(member) = entries.next_entry()? {
            members.push(member);
        }
        Ok(Node::Object(members))
    }
}

/// The issues found so far, and where: in which edits, and whether any is
/// outside every edit.
#[derive(Default)]
struct Found {
    issues: Vec<Issue>,
    /// The index of each edit with an issue, once each.
    edits: Vec<usize>,
    /// An issue concerns the document rather than one edit.
    document: bool,
}

impl Found {
    fn document(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.document = true;
        self.push(path.into(), message.into());
    }

    fn edit(&mut self, index: usize, path: impl Into<String>, message: impl Into<String>) {
        if self.edits.last() != Some(&index) {
            self.edits.push(index);
        }
        self.push(path.into(), message.into());
    }

    fn push(&mut self, path: String, message: String) {
        self.issues.push(Issue { path, message });
    }

    /// What the issues say about the batch, after its file name, or `None`
    /// when there are none: how many edits are invalid, or, when the
    /// document itself is, that it is not an edit batch.
    fn summary(&self) -> Option<String> {
        match (self.issues.is_empty(), self.document, self.edits.len()) {
            (true, _, _) => None,
            (false, true, _) => Some("not an edit batch".into()),
            (false, false, 1) => Some("1 edit is invalid".into()),
            (false, false, count) => Some(format!("{count} edits are invalid")),
        }
    }
}

/// `key` as a member of the JSON path `parent`: `parent.key` when it is an
/// identifier, and otherwise `parent["key"]`, quoted as JSON quotes it.
fn member(parent: &str, key: &str) -> String {
    let identifier = key
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    match (identifier, parent.is_empty()) {
        (true, true) => key.to_owned(),
        (true, false) => format!("{parent}.{key}"),
        (false, _) => format!("{parent}[{}]", Value::String(key.to_owned())),
    }
}

/// The edits of `document`, each one that has no issue, with every issue
/// of the document and its edits in `found`, in the order they are written.
fn batch(document: &Node, found: &mut Found) -> Vec<Pending> {
    let members = match document {
        Node::Object(members) => members,
        Node::Null | Node::Scalar | Node::String(_) | Node::Array(_) => {
            found.document("$", r#"expected {"edits": [...]}"#);
            return Vec::new();
        }
    };
    let mut named = None;
    for (key, value) in members {
        if key != "edits" {
            found.document(member("", key), "unknown field");
        } else if named.is_some() {
            found.document("edits", "given more than once");
        } else {
            named = Some(edits("edits", value, found));
        }
    }
    match named {
        Some(edits) => edits,
        None => {
            found.document("edits", "missing field");
            Vec::new()
        }
    }
}

/// The edits of `value`, the document's edits at `path`.
fn edits(path: &str, value: &Node, found: &mut Found) -> Vec<Pending> {
    match value {
        Node::Array(items) if items.is_empty() => {
            found.document(path, "the batch has no edits");
            Vec::new()
        }
        Node::Array(items) => items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| edit(index, &format!("{path}[{index}]"), item, found))
            .collect(),
        Node::Null | Node::Scalar | Node::String(_) | Node::Object(_) => {
            found.document(path, "expected an array of edits");
            Vec::new()
        }
    }
}

/// An edit's fields: its operation, then the strings it may give.
const FIELDS: [&str; 6] = ["operation", "name", "at", "file", "fragment", "from"];

/// A string member of an edit as given.
#[derive(Clone, Copy)]
enum Given<'a> {
    /// Not written, or `null`.
    Absent,
    Text(&'a str),
    /// Written as another type, an issue already found.
    Invalid,
}

impl<'a> Given<'a> {
    /// A field of the edit at `index`, at `path`, as `value` gives it,
    /// adding an issue to `found` when it is not a string.
    fn of(found: &mut Found, index: usize, path: String, value: &'a Node) -> Self {
        match value {
            Node::Null => Given::Absent,
            Node::String(text) => Given::Text(text),
            Node::Scalar | Node::Array(_) | Node::Object(_) => {
                found.edit(index, path, "expected a string");
                Given::Invalid
            }
        }
    }

    fn is_given(self) -> bool {
        !matches!(self, Given::Absent)
    }
}

/// The edit at `index`, written at `path`, or `None` when it has an
/// issue. Each issue is added to `found`: those of each field in the order
/// they are written, then those of the edit as a whole.
fn edit(index: usize, path: &str, node: &Node, found: &mut Found) -> Option<Pending> {
    let issues = found.issues.len();
    let mut operation = None;
    let mut strings: [Option<Given>; 5] = [None; 5];
    let field = |name: &str| member(path, name);
    let members = match node {
        Node::Object(members) => members,
        Node::Null | Node::Scalar | Node::String(_) | Node::Array(_) => {
            found.edit(index, path, "expected an object");
            return None;
        }
    };
    for (key, value) in members {
        match FIELDS.iter().position(|field| field == key) {
            None => found.edit(index, field(key), "unknown field"),
            Some(0) if operation.is_some() => found.edit(index, field(key), "given more than once"),
            Some(0) => operation = Some(self::operation(found, index, field(key), value)),
            Some(position) if strings[position - 1].is_some() => {
                found.edit(index, field(key), "given more than once")
            }
            Some(position) => {
                strings[position - 1] = Some(Given::of(found, index, field(key), value))
            }
        }
    }
    let operation = match operation {
        Some(operation) => operation,
        None => {
            found.edit(index, field(FIELDS[0]), "missing field");
            None
        }
    };
    let [name, at, file, fragment, from] = strings.map(|given| match given {
        Some(given) => given,
        None => Given::Absent,
    });
    if name.is_given() == at.is_given() {
        found.edit(index, path, "give exactly one of name and at");
    }
    // An at is checked whatever else is wrong, beside the file given for
    // it: one of another type is given too, though it names no path.
    let located = match at {
        Given::Text(at) => {
            let file = match file {
                Given::Absent => None,
                Given::Text(file) => Some(PathBuf::from(file)),
                Given::Invalid => Some(PathBuf::new()),
            };
            match request::locate(at, file) {
                Ok(location) => Some(location),
                Err(Unlocated::Malformed) => {
                    found.edit(
                        index,
                        field("at"),
                        format!("expects LINE:COLUMN or FILE:LINE:COLUMN, got {at}"),
                    );
                    None
                }
                Err(Unlocated::FileTwice) => {
                    found.edit(
                        index,
                        field("file"),
                        "at FILE:LINE:COLUMN names the file; omit file",
                    );
                    None
                }
                Err(Unlocated::NoFile) => {
                    found.edit(
                        index,
                        field("at"),
                        "LINE:COLUMN needs file, or give FILE:LINE:COLUMN",
                    );
                    None
                }
            }
        }
        Given::Absent | Given::Invalid => None,
    };
    if fragment.is_given() == from.is_given() {
        found.edit(index, path, "give exactly one of fragment and from");
    }
    if found.issues.len() > issues {
        return None;
    }
    // With no issue, the operation is valid, exactly one of name and at is
    // a string, an at names a location, and exactly one of fragment and
    // from is a string.
    let target = match (name, located, file) {
        (Given::Text(name), _, Given::Absent) => ShowQuery::Name {
            name: name.to_owned(),
            file: None,
        },
        (Given::Text(name), _, Given::Text(file)) => ShowQuery::Name {
            name: name.to_owned(),
            file: Some(PathBuf::from(file)),
        },
        (_, Some((file, position)), _) => ShowQuery::At { file, position },
        (Given::Absent | Given::Invalid, None, _) | (Given::Text(_), _, Given::Invalid) => {
            unreachable!("an edit with no issue names its target")
        }
    };
    let fragment = match (fragment, from) {
        (Given::Text(fragment), _) => Fragment::Inline(fragment.to_owned()),
        (_, Given::Text(from)) => Fragment::From(PathBuf::from(from)),
        (Given::Absent | Given::Invalid, Given::Absent | Given::Invalid) => {
            unreachable!("an edit with no issue names its fragment")
        }
    };
    Some(Pending {
        operation: operation.expect("an edit with no issue has an operation"),
        target,
        fragment,
    })
}

/// The operation `value`, at `path`, names, or `None` when it names none,
/// which is an issue added to `found`. It is a string, whose names the
/// library's deserializer for its operations decides.
fn operation(found: &mut Found, index: usize, path: String, value: &Node) -> Option<Operation> {
    let name = match value {
        Node::String(name) => name,
        Node::Null | Node::Scalar | Node::Array(_) | Node::Object(_) => {
            found.edit(index, path, "expected a string");
            return None;
        }
    };
    match serde_json::from_value(Value::String(name.clone())) {
        Ok(operation) => Some(operation),
        Err(error) => {
            found.edit(index, path, error.to_string());
            None
        }
    }
}
