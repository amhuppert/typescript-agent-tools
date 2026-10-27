//! The failure of a command that leaves no outcome to deliver: a usage
//! error, an input that cannot be read, a service that cannot be reached, a
//! result that cannot be written, or a fault of this program. Each carries
//! a category and a code from [`CODES`], the one table of failure codes,
//! which `--help --json` publishes and `docs/cli-reference.md` lists, so a
//! caller branches on the code, never the message. A failure is built only
//! from a code, so no other code can be given.
//!
//! This module owns a failure's two forms, its text and its document's
//! `error`; `super::delivery` decides its channel and exit code. Analysis
//! failures, rejected edits, and unselected targets are outcomes with
//! their own documents, not failures.

use super::command_line;
use serde::Serialize;
use serde_json::{Value, json};
use std::fmt::Write as _;

/// The kind of a failure, which decides how it is delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Category {
    /// The request, its environment, or a path it names for output.
    Usage,
    /// An input the request names cannot be read or is not well formed.
    Input,
    /// The project service could not be started or reached.
    Transport,
    /// A result could not be written.
    Write,
    /// A fault of this program.
    Internal,
}

/// A registered failure code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    NotUnicode,
    UnknownOption,
    MissingValue,
    InvalidValue,
    Operation,
    FlagScope,
    RepeatedFlag,
    MissingFlag,
    Selection,
    Operands,
    Output,
    Environment,
    Read,
    Batch,
    Directory,
    Socket,
    Lock,
    Startup,
    Timeout,
    Protocol,
    Identity,
    WriteOutput,
    WritePlan,
    Watchdog,
}

impl Code {
    const COUNT: usize = Self::Watchdog as usize + 1;

    /// This code's entry.
    pub(crate) fn entry(self) -> &'static CodeEntry {
        let entry = &CODES[self as usize];
        debug_assert_eq!(entry.code, self, "{}", entry.name);
        entry
    }

    /// The code `name` names, or `None` when none is registered.
    pub(crate) fn named(name: &str) -> Option<Self> {
        CODES
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.code)
    }
}

/// What a code means: its name, `category.snake_case`, and its category.
pub(crate) struct CodeEntry {
    pub(crate) code: Code,
    pub(crate) name: &'static str,
    pub(crate) category: Category,
    pub(crate) meaning: &'static str,
}

const fn code(
    code: Code,
    category: Category,
    name: &'static str,
    meaning: &'static str,
) -> CodeEntry {
    CodeEntry {
        code,
        name,
        category,
        meaning,
    }
}

/// Every failure code, in [`Code`] order.
pub(crate) static CODES: [CodeEntry; Code::COUNT] = [
    code(
        Code::NotUnicode,
        Category::Usage,
        "usage.not_unicode",
        "An argument is not Unicode; requests name files exactly. Always text.",
    ),
    code(
        Code::UnknownOption,
        Category::Usage,
        "usage.unknown_option",
        "An argument written as an option names no flag.",
    ),
    code(
        Code::MissingValue,
        Category::Usage,
        "usage.missing_value",
        "A flag that takes a value is the last argument.",
    ),
    code(
        Code::InvalidValue,
        Category::Usage,
        "usage.invalid_value",
        "A flag's value is not in the form it takes, such as --at or --depth.",
    ),
    code(
        Code::Operation,
        Category::Usage,
        "usage.operation",
        "The flags name operations that cannot run together.",
    ),
    code(
        Code::FlagScope,
        Category::Usage,
        "usage.flag_scope",
        "The operation does not take a flag, or takes it only beside another.",
    ),
    code(
        Code::RepeatedFlag,
        Category::Usage,
        "usage.repeated_flag",
        "A flag the operation takes once is given again.",
    ),
    code(
        Code::MissingFlag,
        Category::Usage,
        "usage.missing_flag",
        "A flag the operation requires is absent, such as --project or --from.",
    ),
    code(
        Code::Selection,
        Category::Usage,
        "usage.selection",
        "The request does not select exactly once: both or neither of --at and --name, or a selection where a plan or batch names its own.",
    ),
    code(
        Code::Operands,
        Category::Usage,
        "usage.operands",
        "The FILE operands do not fit the operation: more than one, stdin where it reads files by name, none where it needs one, or one beside --at FILE:LINE:COLUMN.",
    ),
    code(
        Code::Output,
        Category::Usage,
        "usage.output",
        "An editing command cannot write its result or plan at the path given: it exists, names the other output or a file the command reads or edits, or cannot be inspected.",
    ),
    code(
        Code::Environment,
        Category::Usage,
        "usage.environment",
        "A TSPARSE_* variable holds a value it does not accept.",
    ),
    code(
        Code::Read,
        Category::Input,
        "input.read",
        "An input the request names cannot be read: the file to parse, stdin, a fragment, or a batch file.",
    ),
    code(
        Code::Batch,
        Category::Input,
        "input.batch",
        "A batch file is not JSON, or not a valid edit batch: each issue is named by its JSON path, and next prints the batch format.",
    ),
    code(
        Code::Directory,
        Category::Transport,
        "transport.directory",
        "The service's runtime directory cannot be created or inspected, or is not a private directory owned by this user with mode 0700; next or remedy says how to fix it.",
    ),
    code(
        Code::Socket,
        Category::Transport,
        "transport.socket",
        "The service's socket cannot be bound, connected, or used: its path is too long for a socket address or names another kind of file, or the connection failed or closed before an answer.",
    ),
    code(
        Code::Lock,
        Category::Transport,
        "transport.lock",
        "A lock file in the service's runtime directory cannot be opened or locked; next or remedy says how to select a writable directory.",
    ),
    code(
        Code::Startup,
        Category::Transport,
        "transport.startup",
        "A service could not be started: it could not be spawned or set up, it exited without recording why, or the diagnostic it recorded cannot be read or removed.",
    ),
    code(
        Code::Timeout,
        Category::Transport,
        "transport.timeout",
        "The service did not become ready, take a request, answer, or stop in time.",
    ),
    code(
        Code::Protocol,
        Category::Transport,
        "transport.protocol",
        "A request or answer the other side cannot use: a request over 1 MiB, a request the service could not read, or an answer that cannot be decoded or does not answer the request.",
    ),
    code(
        Code::Identity,
        Category::Transport,
        "transport.identity",
        "The service's identity, its configuration's absolute path and the build, cannot be established, or the service at its socket serves another.",
    ),
    code(
        Code::WriteOutput,
        Category::Write,
        "write.output",
        "The result could not be written to stdout or the output file, so neither can its document be: always text.",
    ),
    code(
        Code::WritePlan,
        Category::Write,
        "write.plan",
        "The plan could not be saved.",
    ),
    code(
        Code::Watchdog,
        Category::Internal,
        "internal.watchdog",
        "The query's time and memory watchdog could not run.",
    ),
];

/// One field of a failure's input that is wrong, by its JSON path with
/// zero-based indexes (`edits[1].name`).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Issue {
    pub(crate) path: String,
    pub(crate) message: String,
}

/// A failure that leaves no outcome to deliver. Its parts are boxed, so a
/// `Result` that carries one stays small.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Failure(Box<Parts>);

#[derive(Debug, PartialEq, Eq)]
struct Parts {
    category: Category,
    code: Code,
    /// One line: what failed and, where one exists, the remedy.
    message: String,
    /// One sentence, only for a designed constraint.
    why: Option<&'static str>,
    issues: Vec<Issue>,
    /// The argument list of a follow-up command, program name first.
    next: Option<Vec<String>>,
    /// The remedy as a sentence, where it is not one command, such as
    /// setting a variable. A failure has at most one of it and `next`.
    remedy: Option<String>,
    /// The text of an outcome whose effect stands though its result could
    /// not be written, such as an applied plan's, so the effect is never
    /// lost. Only a write failure has one, and it is never a document.
    undelivered: Option<String>,
}

impl Failure {
    /// A failure with `code`'s category.
    pub(crate) fn new(code: Code, message: impl Into<String>) -> Self {
        let entry = code.entry();
        Self(Box::new(Parts {
            category: entry.category,
            code,
            message: message.into(),
            why: None,
            issues: Vec::new(),
            next: None,
            remedy: None,
            undelivered: None,
        }))
    }

    pub(crate) fn with_why(mut self, why: &'static str) -> Self {
        self.0.why = Some(why);
        self
    }

    pub(crate) fn with_issues(mut self, issues: Vec<Issue>) -> Self {
        self.0.issues = issues;
        self
    }

    pub(crate) fn with_next(mut self, argv: Vec<String>) -> Self {
        debug_assert!(self.0.remedy.is_none());
        self.0.next = Some(argv);
        self
    }

    pub(crate) fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        debug_assert!(self.0.next.is_none());
        self.0.remedy = Some(remedy.into());
        self
    }

    /// The text of an outcome a write failure leaves undelivered; only a
    /// write failure takes one.
    pub(crate) fn with_undelivered(mut self, text: String) -> Self {
        debug_assert_eq!(self.0.category, Category::Write);
        self.0.undelivered = Some(text);
        self
    }

    /// The same failure, its message rewritten by `reword`.
    pub(crate) fn reworded(mut self, reword: impl FnOnce(&str) -> String) -> Self {
        self.0.message = reword(&self.0.message);
        self
    }

    pub(crate) fn category(&self) -> Category {
        self.0.category
    }

    pub(crate) fn code(&self) -> Code {
        self.0.code
    }

    pub(crate) fn message(&self) -> &str {
        &self.0.message
    }

    /// Why the constraint it reports holds, where it is a designed one.
    pub(crate) fn why(&self) -> Option<&'static str> {
        self.0.why
    }

    /// The argument list of the command that remedies it.
    pub(crate) fn next(&self) -> Option<&[String]> {
        self.0.next.as_deref()
    }

    /// The remedy as a sentence, where it is not one command.
    pub(crate) fn remedy(&self) -> Option<&str> {
        self.0.remedy.as_deref()
    }

    /// The failure as stderr shows it, without a final newline:
    /// `CODE: message`, the outcome it left undelivered, then one
    /// `  path: message` line per issue, `  why: …`, and `  next: …`, the
    /// follow-up command or the remedy.
    pub(crate) fn text(&self) -> String {
        let parts = &self.0;
        let mut text = format!("{}: {}", parts.code.entry().name, parts.message);
        if let Some(undelivered) = &parts.undelivered {
            write!(text, "\n{undelivered}").unwrap();
        }
        for issue in &parts.issues {
            write!(text, "\n  {}: {}", issue.path, issue.message).unwrap();
        }
        if let Some(why) = parts.why {
            write!(text, "\n  why: {why}").unwrap();
        }
        if let Some(next) = &parts.next {
            write!(text, "\n  next: {}", command_line::render(next)).unwrap();
        }
        if let Some(remedy) = &parts.remedy {
            write!(text, "\n  next: {remedy}").unwrap();
        }
        text
    }

    /// The `error` member of the failure's document, where `next` is only
    /// ever a command, `{"argv": [...]}`, and a remedy that is not one is
    /// `remedy`.
    pub(crate) fn error(&self) -> Value {
        let parts = &self.0;
        let mut error = json!({
            "category": parts.category,
            "code": parts.code.entry().name,
            "message": parts.message,
        });
        if !parts.issues.is_empty() {
            error["issues"] = parts
                .issues
                .iter()
                .map(|issue| json!({ "path": issue.path, "message": issue.message }))
                .collect();
        }
        if let Some(why) = parts.why {
            error["why"] = json!(why);
        }
        if let Some(next) = &parts.next {
            error["next"] = json!({ "argv": next });
        }
        if let Some(remedy) = &parts.remedy {
            error["remedy"] = json!(remedy);
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::{CODES, Category, Code, Failure, Issue};
    use serde_json::json;

    #[test]
    fn each_code_stands_at_its_id_and_is_named_by_its_category() {
        let mut names = Vec::new();
        for (index, entry) in CODES.iter().enumerate() {
            assert_eq!(entry.code as usize, index, "{}", entry.name);
            let (category, rest) = entry.name.split_once('.').unwrap();
            assert_eq!(json!(entry.category), json!(category), "{}", entry.name);
            assert!(
                !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{}",
                entry.name
            );
            assert!(!entry.meaning.is_empty(), "{}", entry.name);
            assert!(!names.contains(&entry.name), "{}", entry.name);
            names.push(entry.name);
        }
    }

    #[test]
    fn text_and_document_give_every_part_in_order() {
        let failure = Failure::new(Code::Batch, "batch.json: 2 edits are invalid")
            .with_why("a batch is prepared whole or not at all")
            .with_next(vec![
                "tsparse".into(),
                "--apply".into(),
                "my plan.json".into(),
            ])
            .with_issues(vec![
                Issue {
                    path: "edits[0]".into(),
                    message: "give exactly one of name and at".into(),
                },
                Issue {
                    path: "edits[1].extra".into(),
                    message: "unknown field".into(),
                },
            ]);
        assert_eq!(failure.category(), Category::Input);
        assert_eq!(
            failure.text(),
            "input.batch: batch.json: 2 edits are invalid\n  edits[0]: give exactly one of name and at\n  edits[1].extra: unknown field\n  why: a batch is prepared whole or not at all\n  next: tsparse --apply 'my plan.json'"
        );
        assert_eq!(
            failure.error(),
            json!({
                "category": "input",
                "code": "input.batch",
                "message": "batch.json: 2 edits are invalid",
                "issues": [
                    {"path": "edits[0]", "message": "give exactly one of name and at"},
                    {"path": "edits[1].extra", "message": "unknown field"},
                ],
                "why": "a batch is prepared whole or not at all",
                "next": {"argv": ["tsparse", "--apply", "my plan.json"]},
            })
        );
        let written = Failure::new(Code::WriteOutput, "out.json: Is a directory")
            .with_undelivered("applied: a.ts changed\nwarning: busy".into());
        assert_eq!(
            written.text(),
            "write.output: out.json: Is a directory\napplied: a.ts changed\nwarning: busy"
        );
        assert_eq!(
            written.error(),
            json!({"category": "write", "code": "write.output", "message": "out.json: Is a directory"})
        );
        // A remedy that is not one command is a sentence on the next line,
        // and in the document never a `next`.
        let remedied = Failure::new(Code::Socket, "Project service socket /r/k.sock: too long")
            .with_remedy("set a shorter TSPARSE_SERVICE_DIR");
        assert_eq!(
            remedied.text(),
            "transport.socket: Project service socket /r/k.sock: too long\n  next: set a shorter TSPARSE_SERVICE_DIR"
        );
        assert_eq!(
            remedied.error(),
            json!({
                "category": "transport",
                "code": "transport.socket",
                "message": "Project service socket /r/k.sock: too long",
                "remedy": "set a shorter TSPARSE_SERVICE_DIR",
            })
        );
    }
}
