//! Help, rendered from the registry (`super::registry`) and the failure
//! codes (`super::failure`) alone: the index of operations, one operation's
//! entry, and each as a JSON document, which `tests/cli_registry.rs`
//! renders as `docs/cli-reference.md`. Help reads no file, variable, or
//! service, so a request for it prints the same bytes whatever else it
//! names.

use super::command_line;
use super::delivery::{Delivered, Format, write_document, write_output};
use super::failure::{CODES, Category, CodeEntry};
use super::registry::{
    self, EXIT_CODES, ExitMeaning, FLAGS, Flag, FlagId, GLOBAL_FLAGS, Kind, OPERATIONS,
    OUTPUT_FLAGS, Operation, OperationId, Pretty, Repeat, RepeatedValue, SERVICE_ADMINISTRATION,
};
use super::request::Help;
use serde::Serialize;
use std::fmt::Write as _;

/// Print the help `help` asks for on stdout.
pub(crate) fn run(help: &Help) -> Result<(), Delivered> {
    write_output(None, |writer| match (help.format, help.operation) {
        (Format::Text, None) => writer.write_all(index().as_bytes()),
        (Format::Text, Some(operation)) => writer.write_all(leaf(operation).as_bytes()),
        (Format::Json, None) => write_document(writer, &registry(), help.pretty),
        (Format::Json, Some(operation)) => {
            write_document(writer, &OperationDocument::of(operation), help.pretty)
        }
    })
}

/// The index, without the final newline `write_output` adds: one line per
/// operation, the flags operations share, what each exit code means, and
/// how to ask for one operation's help.
fn index() -> String {
    let mut text = String::from(
        "tsparse parses TypeScript 7 into a compiler-style JSON AST, and navigates, queries, and edits the declarations of files and projects.\n\nOperations:\n",
    );
    rows(
        &mut text,
        OPERATIONS
            .iter()
            .map(|operation| (selector(operation), operation.summary)),
    );
    writeln!(text, "\n{SERVICE_ADMINISTRATION}").unwrap();
    let shared = |flags: &[FlagId]| {
        flags
            .iter()
            .map(|&id| {
                let flag = id.flag();
                let mut spelled = selector_of(flag);
                for alias in flag.aliases {
                    write!(spelled, " ({alias})").unwrap();
                }
                spelled
            })
            .collect::<Vec<_>>()
    };
    let output = shared(OUTPUT_FLAGS);
    let (last, first) = output.split_last().expect("there are output flags");
    writeln!(
        text,
        "\nShared flags: {}, and {last} where an operation lists them; {} for every operation.",
        first.join(", "),
        shared(GLOBAL_FLAGS).join(" and ")
    )
    .unwrap();
    exits(&mut text, &EXIT_CODES);
    write!(
        text,
        "\ntsparse --OP --help prints one operation's usage, flags, examples, and exit codes, as {} or {} do. With {}, {} prints every operation and flag as one document, which docs/cli-reference.md renders.",
        help_command(OperationId::References.operation()),
        help_command(OperationId::Apply.operation()),
        FlagId::Json,
        help_command(OperationId::Parse.operation()),
    )
    .unwrap();
    text
}

/// One operation's entry, without the final newline `write_output` adds.
fn leaf(operation: &Operation) -> String {
    let mut text = match operation.flag {
        Some(flag) => format!("tsparse {flag}: {}\n\nUsage:\n", operation.summary),
        None => format!("tsparse: {}\n\nUsage:\n", operation.summary),
    };
    for usage in operation.usage {
        writeln!(text, "  {usage}").unwrap();
    }
    write!(text, "\n{}\n\nFlags:\n", operation.details).unwrap();
    rows(
        &mut text,
        operation
            .accepted()
            .map(|id| (label(id.flag()), id.flag().summary)),
    );
    writeln!(text, "{}\n\nExamples:", rules(operation)).unwrap();
    for example in operation.examples {
        let argv: Vec<&str> = std::iter::once("tsparse")
            .chain(example.args.iter().copied())
            .collect();
        writeln!(
            text,
            "  {}\n      {}",
            command_line::render(&argv),
            example.summary
        )
        .unwrap();
    }
    exits(&mut text, operation.exits);
    let related: Vec<String> = operation
        .related
        .iter()
        .map(|&id| help_command(id.operation()))
        .collect();
    write!(
        text,
        "\nRelated: {}\nDocument: {}",
        related.join(", "),
        operation.doc
    )
    .unwrap();
    text
}

/// The operation's repeated-value, `--pretty`, and display-limit rules,
/// as one sentence each, for its help and the reference alike.
fn rules(operation: &Operation) -> String {
    let values = match operation.repeat {
        RepeatedValue::LastValue => "A value flag given more than once takes its last value.",
        RepeatedValue::Reject => "A value flag may be given once.",
    };
    let mut rules = match operation.pretty {
        Pretty::Alone => format!(
            "{values} {} is valid without {}.",
            FlagId::Pretty,
            FlagId::Json
        ),
        Pretty::WithJson => format!(
            "{values} {} is valid only with {}.",
            FlagId::Pretty,
            FlagId::Json
        ),
    };
    let bounds: Vec<String> = operation
        .limits
        .iter()
        .map(|bounded| format!("{} {}", bounded.default, bounded.summary))
        .collect();
    if let Some((last, first)) = bounds.split_last() {
        let listed = match first {
            [] => last.clone(),
            first => format!("{} and {last}", first.join(", ")),
        };
        write!(
            rules,
            " Display limits: at most {listed}; {} N|all sets them, {} without {} removes them, and a cut's last line names the command that shows the rest.",
            FlagId::Limit,
            FlagId::Output.flag().aliases[0],
            FlagId::Limit
        )
        .unwrap();
    }
    rules
}

/// `items` as aligned `label  summary` lines.
fn rows(text: &mut String, items: impl Iterator<Item = (String, &'static str)>) {
    let items: Vec<_> = items.collect();
    let width = items
        .iter()
        .map(|(label, _)| label.len())
        .fold(0, usize::max);
    for (label, summary) in items {
        writeln!(text, "  {label:<width$}  {summary}").unwrap();
    }
}

fn exits(text: &mut String, exits: &[ExitMeaning]) {
    text.push_str("\nExit codes:\n");
    for exit in exits {
        writeln!(text, "  {}  {}", exit.code, exit.meaning).unwrap();
    }
}

/// A flag as a leaf lists it: its aliases and name, then its value.
fn label(flag: &Flag) -> String {
    let mut label = String::new();
    for alias in flag.aliases {
        write!(label, "{alias}, ").unwrap();
    }
    label.push_str(&selector_of(flag));
    label
}

/// A flag's name, then its value.
fn selector_of(flag: &Flag) -> String {
    match flag.kind {
        Kind::Boolean => flag.name.to_string(),
        Kind::Value(placeholder) => format!("{} {placeholder}", flag.name),
    }
}

/// How an operation is asked for: its flag with its value, and parse mode
/// by an input alone.
fn selector(operation: &Operation) -> String {
    match operation.flag {
        Some(flag) => selector_of(flag.flag()),
        None => "[FILE|-]".into(),
    }
}

/// The command that prints `operation`'s help, as an argument list: its
/// flag, with its value's placeholder as the value (a value is never a flag
/// position), then `--help`; parse mode's is the index's. A failure that
/// help explains names it as `next`.
pub(crate) fn help_argv(operation: &Operation) -> Vec<String> {
    let mut argv = vec!["tsparse"];
    if let Some(flag) = operation.flag {
        argv.push(flag.flag().name);
        if let Kind::Value(placeholder) = flag.flag().kind {
            argv.push(placeholder);
        }
    }
    argv.push(FlagId::Help.flag().name);
    argv.into_iter().map(String::from).collect()
}

/// `help_argv` as a command line.
fn help_command(operation: &Operation) -> String {
    command_line::render(&help_argv(operation))
}

/// The complete registry: every operation and flag, the shared flags, what
/// each exit code means, and every failure code.
#[derive(Serialize)]
struct RegistryDocument {
    operations: Vec<OperationDocument>,
    flags: Vec<FlagDocument>,
    global_flags: Vec<&'static str>,
    output_flags: Vec<&'static str>,
    exit_codes: Vec<ExitDocument>,
    failure_codes: Vec<FailureCodeDocument>,
}

fn registry() -> RegistryDocument {
    let names = |flags: &[FlagId]| flags.iter().map(|id| id.flag().name).collect();
    RegistryDocument {
        operations: OPERATIONS.iter().map(OperationDocument::of).collect(),
        flags: FLAGS.iter().map(FlagDocument::of).collect(),
        global_flags: names(GLOBAL_FLAGS),
        output_flags: names(OUTPUT_FLAGS),
        exit_codes: EXIT_CODES.iter().map(ExitDocument::of).collect(),
        failure_codes: CODES.iter().map(FailureCodeDocument::of).collect(),
    }
}

/// A flag, the same in every entry that lists it.
#[derive(Serialize)]
struct FlagDocument {
    name: &'static str,
    aliases: &'static [&'static str],
    /// The value's placeholder; `null` for a switch.
    value: Option<&'static str>,
    summary: &'static str,
    repeat: &'static str,
    /// `repeat` as the reference states it.
    repeat_rule: &'static str,
    /// The operation the flag chooses, by name.
    selects: Option<&'static str>,
}

impl FlagDocument {
    fn of(flag: &Flag) -> Self {
        let (repeat, repeat_rule) = match flag.repeat {
            Repeat::Redundant => ("redundant", "stays on"),
            Repeat::PerOperation => ("per_operation", "as the operation says"),
            Repeat::Reject => ("reject", "refused"),
        };
        Self {
            name: flag.name,
            aliases: flag.aliases,
            value: match flag.kind {
                Kind::Boolean => None,
                Kind::Value(placeholder) => Some(placeholder),
            },
            summary: flag.summary,
            repeat,
            repeat_rule,
            selects: registry::selected_by(flag.id).map(|operation| operation.name),
        }
    }
}

/// One operation's entry, listing every flag it accepts.
#[derive(Serialize)]
struct OperationDocument {
    name: &'static str,
    flag: Option<&'static str>,
    summary: &'static str,
    details: &'static str,
    usage: &'static [&'static str],
    flags: Vec<FlagDocument>,
    repeated_values: &'static str,
    pretty: &'static str,
    /// The lists it bounds, which `--limit` sets, with their defaults.
    limits: Vec<LimitDocument>,
    /// `repeated_values`, `pretty`, and `limits` as help states them.
    rules: String,
    examples: Vec<ExampleDocument>,
    exits: Vec<ExitDocument>,
    related: Vec<&'static str>,
    doc: &'static str,
}

impl OperationDocument {
    fn of(operation: &Operation) -> Self {
        Self {
            name: operation.name,
            flag: operation.flag.map(|flag| flag.flag().name),
            summary: operation.summary,
            details: operation.details,
            usage: operation.usage,
            flags: operation
                .accepted()
                .map(|id| FlagDocument::of(id.flag()))
                .collect(),
            repeated_values: match operation.repeat {
                RepeatedValue::LastValue => "last_value",
                RepeatedValue::Reject => "reject",
            },
            pretty: match operation.pretty {
                Pretty::Alone => "alone",
                Pretty::WithJson => "with_json",
            },
            limits: operation
                .limits
                .iter()
                .map(|bounded| LimitDocument {
                    list: bounded.list.name(),
                    default: bounded.default,
                    summary: bounded.summary,
                })
                .collect(),
            rules: rules(operation),
            examples: operation
                .examples
                .iter()
                .map(|example| ExampleDocument {
                    argv: std::iter::once("tsparse")
                        .chain(example.args.iter().copied())
                        .collect(),
                    summary: example.summary,
                })
                .collect(),
            exits: operation.exits.iter().map(ExitDocument::of).collect(),
            related: operation
                .related
                .iter()
                .map(|id| id.operation().name)
                .collect(),
            doc: operation.doc,
        }
    }
}

/// A list an operation bounds, and its default bound.
#[derive(Serialize)]
struct LimitDocument {
    /// Its key in a document's `display`.
    list: &'static str,
    default: usize,
    summary: &'static str,
}

#[derive(Serialize)]
struct ExampleDocument {
    /// The command, program name first, as `next` commands carry theirs.
    argv: Vec<&'static str>,
    summary: &'static str,
}

#[derive(Serialize)]
struct ExitDocument {
    code: u8,
    meaning: &'static str,
}

impl ExitDocument {
    fn of(exit: &ExitMeaning) -> Self {
        Self {
            code: exit.code,
            meaning: exit.meaning,
        }
    }
}

/// A failure code, which a failure's text begins with and its document's
/// `error.code` holds.
#[derive(Serialize)]
struct FailureCodeDocument {
    code: &'static str,
    category: Category,
    meaning: &'static str,
}

impl FailureCodeDocument {
    fn of(entry: &CodeEntry) -> Self {
        Self {
            code: entry.name,
            category: entry.category,
            meaning: entry.meaning,
        }
    }
}
