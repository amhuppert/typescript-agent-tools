//! The one parsed-request boundary: command-line arguments become one
//! executable request or a usage error. Handlers never re-decide which
//! flag combinations make sense. The vocabulary (which flags exist, how
//! they are spelled, which take a value, and which operations accept them)
//! is `super::registry`'s; this module decides which combinations of
//! accepted flags form a valid request.

use super::Failure;
use super::delivery::{Display, Format, Limit, Route};
use super::editing;
use super::failure::Code;
use super::navigation::Depth;
use super::registry::{self, FlagId, Kind, Operation, OperationId, Pretty, Repeat, RepeatedValue};
use std::ffi::OsString;
use std::path::PathBuf;
use typescript_parser::editing::Operation as EditOperation;
use typescript_parser::navigation::{Selector, ShowQuery};
use typescript_parser::project::ProjectRequest;
use typescript_parser::references::{ReferenceFilters, ReferenceQuery, ReferenceRequest};
use typescript_parser::source::SourcePosition;
use typescript_parser::types::{TypeQuery, TypeRequest};

pub(crate) enum Request {
    Help(Help),
    Version,
    Parse(Parse),
    Service(Service),
    Doctor(Doctor),
    Query(Query),
    Edit(EditCommand),
}

/// A request that is not valid: its usage failure, and how that failure is
/// delivered, which is read from the arguments before anything is
/// validated (design, step 3: "Knowing that JSON was wanted").
pub(crate) struct Invalid {
    pub(crate) failure: Failure,
    pub(crate) route: Route<'static>,
}

/// A request for help, which reads nothing but the registry.
pub(crate) struct Help {
    /// The one operation the other flags name, whose entry is printed;
    /// `None`, for the index, when they name none or several.
    pub(crate) operation: Option<&'static Operation>,
    pub(crate) format: Format,
    /// Indent the document of `--json`.
    pub(crate) pretty: bool,
}

/// An editing command and how its outcomes are delivered.
pub(crate) struct EditCommand {
    pub(crate) action: EditAction,
    pub(crate) format: Format,
    /// Indent the document of `--json`.
    pub(crate) pretty: bool,
    pub(crate) output: Option<String>,
    /// What a preview and an unselected edit display of their lists.
    pub(crate) display: Display,
}

pub(crate) enum EditAction {
    /// Prepare and preview a batch, in named files or a project (its
    /// configuration as spelled, and whether to answer directly), and
    /// optionally save it as a plan.
    Prepare {
        source: EditSource,
        project: Option<(String, bool)>,
        plan: Option<PathBuf>,
    },
    /// Apply a saved plan.
    Apply { plan: PathBuf },
}

/// Where a prepare command's edits come from.
pub(crate) enum EditSource {
    /// One edit on the command line, its fragment read from a file or `-`.
    One {
        operation: EditOperation,
        target: ShowQuery,
        from: PathBuf,
    },
    /// A batch file.
    Batch(PathBuf),
}

/// Parse mode: one file, or stdin when `file` is `None` or `-`.
pub(crate) struct Parse {
    pub(crate) file: Option<String>,
    pub(crate) pretty: bool,
    pub(crate) jsx: bool,
    pub(crate) check: bool,
    pub(crate) output: Option<String>,
}

pub(crate) struct Service {
    pub(crate) action: ServiceAction,
    /// The operation's flag, for the message of platforms without a service.
    #[cfg_attr(unix, allow(dead_code))]
    pub(crate) flag: String,
    pub(crate) project: String,
    /// Status's form: text, or its document with `--json`.
    pub(crate) format: Format,
    pub(crate) pretty: bool,
    pub(crate) output: Option<String>,
}

/// A request for the doctor's report, of one project's configuration and
/// service when `project` names it.
pub(crate) struct Doctor {
    pub(crate) project: Option<String>,
    pub(crate) format: Format,
    /// Indent the document of `--json`.
    pub(crate) pretty: bool,
}

pub(crate) enum ServiceAction {
    Status,
    Stop,
    Serve,
}

/// An analysis query and how its outcomes are delivered.
pub(crate) struct Query {
    pub(crate) target: Target,
    pub(crate) format: Format,
    /// Indent documents: with `--json`, and a reference answer's, which
    /// `--pretty` alone selects in place of its text.
    pub(crate) pretty: bool,
    /// The outline levels to display; parsing and the catalog never depend
    /// on it, and other operations ignore it.
    pub(crate) depth: Depth,
    pub(crate) output: Option<String>,
    /// What the answer displays of the lists it bounds.
    pub(crate) display: Display,
}

/// What answers a query.
pub(crate) enum Target {
    /// Navigation of one named file alone, always in this process.
    File {
        file: PathBuf,
        operation: FileOperation,
    },
    /// A request on a configured project, answered by the project's
    /// service, or in this process when `direct` or where there is no
    /// service. The configuration keeps the request's spelling.
    Project {
        config: String,
        request: ProjectRequest,
        direct: bool,
    },
}

pub(crate) enum FileOperation {
    Outline,
    Show(Selector),
}

impl Target {
    /// The operation's name in documents.
    pub(crate) fn operation(&self) -> &'static str {
        match self {
            Self::File {
                operation: FileOperation::Outline,
                ..
            }
            | Self::Project {
                request: ProjectRequest::Outline { .. },
                ..
            } => "outline",
            Self::File {
                operation: FileOperation::Show(_),
                ..
            }
            | Self::Project {
                request: ProjectRequest::Show(_),
                ..
            } => "show",
            Self::Project {
                request: ProjectRequest::References(_),
                ..
            }
            | Self::Project {
                request: ProjectRequest::ReferenceSearch(_),
                ..
            } => "references",
            Self::Project {
                request: ProjectRequest::Edit(_),
                ..
            } => "edit",
            Self::Project {
                request: ProjectRequest::Type(_),
                ..
            } => "type",
        }
    }
}

/// Every registered flag given, before validation.
#[derive(Default)]
struct Flags {
    /// Each flag in argument order, with its value when it takes one; a
    /// value is absent only beside the usage error that refuses the request.
    given: Vec<(FlagId, Option<String>)>,
    files: Vec<String>,
    /// Every argument as given, in order, with what it was read as, for the
    /// command that repeats the request (`Flags::display`).
    args: Vec<(String, Read)>,
}

/// What the parser read an argument as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Read {
    Operand,
    Flag(FlagId),
    /// The value of the flag before it.
    Value(FlagId),
}

impl Flags {
    fn has(&self, flag: FlagId) -> bool {
        self.given.iter().any(|&(given, _)| given == flag)
    }

    /// The last value given for `flag`: the only one wherever the operation
    /// rejects a repeat.
    fn value(&self, flag: FlagId) -> Option<String> {
        self.given
            .iter()
            .rev()
            .find(|&&(given, _)| given == flag)
            .and_then(|(_, value)| value.clone())
    }

    fn format(&self) -> Format {
        if self.has(FlagId::Json) {
            Format::Json
        } else {
            Format::Text
        }
    }

    /// The operations the flags given name, each once, with the first flag
    /// that selects it, in argument order.
    fn named(&self) -> Vec<(FlagId, &'static Operation)> {
        let mut named: Vec<(FlagId, &Operation)> = Vec::new();
        for &(flag, _) in &self.given {
            if let Some(operation) = registry::selected_by(flag) {
                if named.iter().all(|(_, earlier)| earlier.id != operation.id) {
                    named.push((flag, operation));
                }
            }
        }
        named
    }

    /// The one operation the flags run: of the operations they name, the
    /// one that accepts every such flag, so an operation's flag that another
    /// named operation accepts (`--references` beside a service action)
    /// changes nothing. No operation flag runs parse mode.
    fn operation(&self) -> Result<&'static Operation, Failure> {
        let named = self.named();
        if named.is_empty() {
            return Ok(OperationId::Parse.operation());
        }
        let mut chosen = named
            .iter()
            .filter(|(_, operation)| named.iter().all(|&(flag, _)| operation.accepts(flag)));
        match (chosen.next(), chosen.next()) {
            (Some(&(_, operation)), None) => Ok(operation),
            _ => Err(Failure::new(
                Code::Operation,
                format!(
                    "Choose one operation: {}",
                    list(named.iter().map(|&(flag, _)| flag))
                ),
            )),
        }
    }

    /// The operation whose help a request for help prints: the one its
    /// flags name, however spelled or repeated. Help asks which operations
    /// the flags name, not which one a request would run, so a service
    /// action beside the `--references` it takes names two; with none or
    /// several the index answers (decisions.md, step 2).
    fn help(&self) -> Option<&'static Operation> {
        match self.named()[..] {
            [(_, operation)] => Some(operation),
            [] | [_, _, ..] => None,
        }
    }

    /// How a usage failure of the request these flags were meant to form
    /// is delivered, read before anything is validated: with `--json` in a
    /// flag position, as a document naming the one operation the flags
    /// name. A failure that is about `--json` itself is text whatever this
    /// says ([`Refusal::Text`]).
    fn refusal(&self) -> Route<'static> {
        if !self.has(FlagId::Json) {
            return Route::TEXT;
        }
        let (operation, fields) = match self.help() {
            None => (None, &[][..]),
            Some(operation) => match editing::documents(operation.id) {
                Some(documents) => (Some(documents.operation), documents.fields),
                None => (Some(operation.name), &[][..]),
            },
        };
        Route {
            format: Format::Json,
            pretty: self.has(FlagId::Pretty),
            output: None,
            new_file: false,
            operation,
            fields,
        }
    }

    /// A flag the operation does not accept is out of its scope, and the
    /// error lists every operation that accepts it. When that flag is
    /// `--json`, the operation has no document, so the error is text.
    fn check_scope(&self, operation: &Operation) -> Result<(), Refusal> {
        match self
            .given
            .iter()
            .find(|&&(flag, _)| !operation.accepts(flag))
        {
            Some(&(flag, _)) => {
                let failure = Failure::new(
                    Code::FlagScope,
                    format!("{flag} applies to: {}", list(registry::accepting(flag))),
                );
                Err(match flag {
                    FlagId::Json => Refusal::Text(failure),
                    _ => Refusal::Routed(failure),
                })
            }
            None => Ok(()),
        }
    }

    /// What the request displays of the lists it bounds: `--limit`, a
    /// positive number or `all`, and the command that shows every item,
    /// which repeats the request's arguments as given with `--limit all`
    /// in place of any `--limit` value, or added (the run owner's answer to
    /// question a61fe38c). A command that edits leaves out `--plan` and
    /// `-o`, both of which now exist; a query keeps `-o`, so following it
    /// writes every item where the request asked. The command names no
    /// transport: the service and a direct load give the same answer byte
    /// for byte, so it leaves out `--direct` and spells
    /// `--references-direct` as `--references`, and the answer that prints
    /// it is the same whichever entry gave it (charter, `parity`). The
    /// command keeps the request's form, so an outline's text stays text,
    /// whose import lines list eight names and count the rest, as every
    /// text outline does (the run owner's resolution of this step's
    /// review).
    fn display(&self, operation: &'static Operation, edits: bool) -> Result<Display, Failure> {
        let limit = match self.value(FlagId::Limit).as_deref() {
            None => Limit::Default,
            Some("all") => Limit::All,
            Some(text) => match text.parse::<usize>() {
                Ok(bound) if bound > 0 && text.bytes().all(|c| c.is_ascii_digit()) => {
                    Limit::At(bound)
                }
                // Zero, a sign, or text that is not a number in range.
                Ok(_) | Err(_) => {
                    return Err(Failure::new(
                        Code::InvalidValue,
                        format!(
                            "{} expects a positive number or all, got {text}",
                            FlagId::Limit
                        ),
                    ));
                }
            },
        };
        let dropped: &[FlagId] = if edits {
            &[FlagId::Direct, FlagId::Plan, FlagId::Output]
        } else {
            &[FlagId::Direct]
        };
        let mut reveal = vec!["tsparse".to_string()];
        let mut limited = false;
        for (arg, read) in &self.args {
            match *read {
                Read::Flag(flag) | Read::Value(flag) if dropped.contains(&flag) => {}
                Read::Flag(FlagId::ReferencesDirect) => reveal.push(FlagId::References.to_string()),
                Read::Value(FlagId::Limit) => reveal.push("all".into()),
                Read::Flag(flag) => {
                    limited |= flag == FlagId::Limit;
                    reveal.push(arg.clone());
                }
                Read::Operand | Read::Value(_) => reveal.push(arg.clone()),
            }
        }
        if !limited {
            reveal.extend([FlagId::Limit.to_string(), "all".into()]);
        }
        Ok(Display::new(
            operation,
            limit,
            self.has(FlagId::Output),
            reveal,
        ))
    }

    /// A flag given again: a switch stays on, a value follows the
    /// operation's policy, and a flag given once is refused. The message
    /// names the operation unless the flag is the operation's own.
    fn check_repeats(&self, operation: &Operation) -> Result<(), Failure> {
        for (index, &(flag, _)) in self.given.iter().enumerate() {
            let refused = match flag.flag().repeat {
                Repeat::Redundant => false,
                Repeat::PerOperation => operation.repeat == RepeatedValue::Reject,
                Repeat::Reject => true,
            };
            if refused
                && self.given[..index]
                    .iter()
                    .any(|&(earlier, _)| earlier == flag)
            {
                return Err(Failure::new(
                    Code::RepeatedFlag,
                    if operation.flag == Some(flag) {
                        format!("{flag} may be given at most once")
                    } else {
                        format!("{operation} takes {flag} at most once")
                    },
                ));
            }
        }
        Ok(())
    }
}

/// `items` as a message lists them.
fn list(items: impl IntoIterator<Item = impl std::fmt::Display>) -> String {
    items
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The request `args` (without the program name) describe, or a usage error.
///
/// `--help` in any flag position (an argument a value-taking flag consumes
/// is none) wins over every usage error, and the operation the other flags
/// name selects its help, so the whole list is read before any error is
/// reported. An argument that is not Unicode refuses every other request,
/// always as text. `--version` wins over every other error after it.
pub(crate) fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Request, Invalid> {
    let mut flags = Flags::default();
    // Requests name files exactly, so the first argument that is not
    // Unicode is a usage error wherever it stands; only the message shows
    // its lossy form.
    let mut unicode = None;
    // The first other usage error in reading the arguments.
    let mut error = None;
    let mut help = false;
    let mut version = false;
    while let Some(arg) = args.next() {
        // As an operand, an argument that is not Unicode takes no flag
        // position.
        let arg = match arg.into_string() {
            Ok(arg) => arg,
            Err(arg) => {
                unicode.get_or_insert_with(|| not_unicode(&arg));
                continue;
            }
        };
        // `-` alone is stdin; any other argument beginning with `-` where a
        // flag may stand (not a flag's value) must be a registered flag.
        if arg == "-" || !arg.starts_with('-') {
            flags.args.push((arg.clone(), Read::Operand));
            flags.files.push(arg);
            continue;
        }
        let Some(flag) = registry::flag(&arg) else {
            error.get_or_insert_with(|| unknown_option(&arg));
            continue;
        };
        match flag.id {
            FlagId::Help => help = true,
            FlagId::Version => version |= error.is_none(),
            _ => {
                flags.args.push((arg.clone(), Read::Flag(flag.id)));
                // A value that is missing or not Unicode is recorded as
                // absent; the error it raised refuses the request unless
                // it asks for help, which reads no value.
                let value = match flag.kind {
                    Kind::Boolean => None,
                    Kind::Value(_) => match args.next().map(OsString::into_string) {
                        Some(Ok(value)) => {
                            flags.args.push((value.clone(), Read::Value(flag.id)));
                            Some(value)
                        }
                        Some(Err(value)) => {
                            unicode.get_or_insert_with(|| not_unicode(&value));
                            None
                        }
                        None => {
                            error.get_or_insert_with(|| {
                                Failure::new(Code::MissingValue, format!("Missing value for {arg}"))
                            });
                            None
                        }
                    },
                };
                flags.given.push((flag.id, value));
            }
        }
    }
    if help {
        return Ok(Request::Help(Help {
            operation: flags.help(),
            format: flags.format(),
            pretty: flags.has(FlagId::Pretty),
        }));
    }
    if let Some(unicode) = unicode {
        return Err(Invalid {
            failure: unicode,
            route: Route::TEXT,
        });
    }
    if version {
        return Ok(Request::Version);
    }
    let route = flags.refusal();
    let invalid = |refusal| match refusal {
        Refusal::Routed(failure) => Invalid { failure, route },
        Refusal::Text(failure) => Invalid {
            failure,
            route: Route::TEXT,
        },
    };
    if let Some(error) = error {
        return Err(invalid(Refusal::Routed(error)));
    }
    validate(flags).map_err(invalid)
}

/// A usage failure found in validating the flags, by how it is delivered.
enum Refusal {
    /// As the arguments ask, which `Flags::refusal` reads.
    Routed(Failure),
    /// As text, being about `--json` itself: the operation that refuses it
    /// has no document (design, step 3).
    Text(Failure),
}

impl From<Failure> for Refusal {
    fn from(failure: Failure) -> Self {
        Self::Routed(failure)
    }
}

/// The request the flags read from the arguments form, or its usage
/// failure.
fn validate(flags: Flags) -> Result<Request, Refusal> {
    if flags.files.len() > 1 {
        return Err(Failure::new(Code::Operands, "Expected one input file").into());
    }
    let operation = flags.operation()?;
    refuse_selection(operation, &flags)?;
    flags.check_scope(operation)?;
    flags.check_repeats(operation)?;
    if operation.pretty == Pretty::WithJson && flags.has(FlagId::Pretty) && !flags.has(FlagId::Json)
    {
        return Err(Failure::new(
            Code::FlagScope,
            format!(
                "{} applies to {operation} only with {}",
                FlagId::Pretty,
                FlagId::Json
            ),
        )
        .into());
    }
    let request = match operation.id {
        OperationId::Parse => Ok(parse_mode(flags)),
        OperationId::Outline => navigation(operation, true, flags),
        OperationId::Show => navigation(operation, false, flags),
        OperationId::Type => type_query(operation, flags),
        OperationId::References => references(operation, flags),
        OperationId::ReplaceBody => edit(operation, EditOperation::ReplaceBody, flags),
        OperationId::InsertAfter => edit(operation, EditOperation::InsertAfter, flags),
        OperationId::EditBatch => batch(operation, flags),
        OperationId::Apply => apply(operation, flags),
        OperationId::ReferencesStatus => service(operation, ServiceAction::Status, flags),
        OperationId::ReferencesStop => service(operation, ServiceAction::Stop, flags),
        OperationId::ReferencesServe => service(operation, ServiceAction::Serve, flags),
        OperationId::Doctor => doctor(operation, flags),
    };
    request.map_err(Refusal::from)
}

/// An argument that is not Unicode, shown in its lossy form.
fn not_unicode(arg: &OsString) -> Failure {
    Failure::new(
        Code::NotUnicode,
        format!("Arguments must be Unicode: {}", arg.to_string_lossy()),
    )
}

/// An argument written as an option that names no registered flag, with
/// the nearest registered name when one is close, pointing back into help.
fn unknown_option(arg: &str) -> Failure {
    let message = match registry::nearest(arg) {
        Some(flag) => format!(
            "Unknown option: {arg}; did you mean {}? See tsparse {}",
            flag.id,
            FlagId::Help
        ),
        None => format!("Unknown option: {arg}. See tsparse {}", FlagId::Help),
    };
    Failure::new(Code::UnknownOption, message)
}

/// A plan and a batch file name their own targets, so `--apply` and
/// `--edit-batch` refuse a selection on the command line with that reason
/// before any flag is reported out of scope.
fn refuse_selection(operation: &Operation, flags: &Flags) -> Result<(), Failure> {
    let selection = flags.has(FlagId::At) || flags.has(FlagId::Name) || !flags.files.is_empty();
    if operation.id == OperationId::Apply && (selection || flags.has(FlagId::Project)) {
        return Err(Failure::new(
            Code::Selection,
            format!(
                "{operation} takes only the plan: it applies the recorded files and edits without selecting them again"
            ),
        ));
    }
    if operation.id == OperationId::EditBatch && (selection || flags.has(FlagId::From)) {
        return Err(Failure::new(
            Code::Selection,
            format!("{operation} names every edit's target and fragment in the batch file"),
        ));
    }
    Ok(())
}

fn parse_mode(flags: Flags) -> Request {
    Request::Parse(Parse {
        pretty: flags.has(FlagId::Pretty),
        jsx: flags.has(FlagId::Tsx),
        check: flags.has(FlagId::Check),
        output: flags.value(FlagId::Output),
        file: flags.files.into_iter().next(),
    })
}

/// Type selection and bounded display options belong to this request boundary.
/// The evaluator receives one validated project request, in either transport.
fn type_query(operation: &'static Operation, flags: Flags) -> Result<Request, Failure> {
    let expand = flags.has(FlagId::Expand);
    let depth = match flags.value(FlagId::Depth) {
        Some(_) if !expand => {
            return Err(Failure::new(
                Code::FlagScope,
                format!(
                    "{} applies to {operation} only with {}",
                    FlagId::Depth,
                    FlagId::Expand
                ),
            ));
        }
        None => 3,
        Some(text) => match text.parse::<usize>() {
            Ok(depth) if !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit()) => depth,
            _ => {
                return Err(Failure::new(
                    Code::InvalidValue,
                    format!("--depth expects an integer from 0 through 32, got {text}"),
                ));
            }
        },
    };
    let project = required_project(operation, &flags)?;
    let file = flags.files.first().cloned();
    if file.as_deref() == Some("-") {
        return Err(Failure::new(
            Code::Operands,
            format!("{operation} reads files from the project, not stdin"),
        ));
    }
    let query = match (flags.value(FlagId::At), flags.value(FlagId::Name)) {
        (Some(at), None) => {
            let (file, position) = location(&at, file, &format!("{operation} {}", FlagId::At))?;
            TypeQuery::At { file, position }
        }
        (None, Some(name)) => TypeQuery::Name {
            name,
            file: file.map(PathBuf::from),
        },
        _ => return Err(one_selection(operation)),
    };
    let request = TypeRequest {
        query,
        expand,
        depth,
    };
    request
        .validate()
        .map_err(|error| Failure::new(Code::InvalidValue, error.message))?;
    Ok(Request::Query(Query {
        target: Target::Project {
            config: project,
            request: ProjectRequest::Type(request),
            direct: flags.has(FlagId::Direct),
        },
        format: flags.format(),
        pretty: flags.has(FlagId::Pretty),
        depth: Depth::default(),
        output: flags.value(FlagId::Output),
        display: flags.display(operation, false)?,
    }))
}

/// The configuration an operation that requires `--project` names.
fn required_project(operation: &Operation, flags: &Flags) -> Result<String, Failure> {
    flags.value(FlagId::Project).ok_or_else(|| {
        Failure::new(
            Code::MissingFlag,
            format!("{operation} requires --project TSCONFIG"),
        )
    })
}

/// A selection by neither or both of `--at` and `--name`.
fn one_selection(operation: &Operation) -> Failure {
    Failure::new(
        Code::Selection,
        format!("{operation} requires exactly one of --at or --name"),
    )
}

fn service(operation: &Operation, action: ServiceAction, flags: Flags) -> Result<Request, Failure> {
    let project = required_project(operation, &flags)?;
    if !flags.files.is_empty() {
        return Err(Failure::new(
            Code::Operands,
            "Service actions do not take a reference query",
        ));
    }
    Ok(Request::Service(Service {
        action,
        flag: operation.to_string(),
        project,
        format: flags.format(),
        pretty: flags.has(FlagId::Pretty),
        output: flags.value(FlagId::Output),
    }))
}

fn doctor(operation: &Operation, flags: Flags) -> Result<Request, Failure> {
    if !flags.files.is_empty() {
        return Err(Failure::new(
            Code::Operands,
            format!("{operation} takes no FILE; name a project with --project TSCONFIG"),
        ));
    }
    Ok(Request::Doctor(Doctor {
        project: flags.value(FlagId::Project),
        format: flags.format(),
        pretty: flags.has(FlagId::Pretty),
    }))
}

fn references(operation: &'static Operation, flags: Flags) -> Result<Request, Failure> {
    let project = required_project(operation, &flags)?;
    let file = flags.files.first().cloned();
    // `--file` names a file as the FILE operand does, so neither is stdin.
    let filter = flags.value(FlagId::File);
    if file.as_deref() == Some("-") || filter.as_deref() == Some("-") {
        return Err(Failure::new(
            Code::Operands,
            format!("{operation} reads files from the project, not stdin"),
        ));
    }
    let query = match (flags.value(FlagId::At), flags.value(FlagId::Name)) {
        (Some(at), None) => {
            let (file, position) = location(&at, file, &format!("{operation} {}", FlagId::At))?;
            ReferenceQuery::At { file, position }
        }
        (None, Some(name)) => ReferenceQuery::Name {
            name,
            file: file.map(PathBuf::from),
        },
        _ => return Err(one_selection(operation)),
    };
    Ok(Request::Query(Query {
        target: Target::Project {
            config: project,
            request: ProjectRequest::ReferenceSearch(ReferenceRequest {
                query,
                filters: ReferenceFilters {
                    calls: flags.has(FlagId::Calls),
                    exclude_tests: flags.has(FlagId::NoTests),
                    file: filter.map(PathBuf::from),
                },
            }),
            direct: flags.has(FlagId::ReferencesDirect) || flags.has(FlagId::Direct),
        },
        // Answers are text unless `--json`, which also delivers analysis
        // failures as documents, or `--pretty`, which keeps the indented
        // document with failures as diagnostics (`super::references`).
        format: flags.format(),
        pretty: flags.has(FlagId::Pretty),
        depth: Depth::default(),
        output: flags.value(FlagId::Output),
        display: flags.display(operation, false)?,
    }))
}

/// Outline and show: file navigation, or a project request with
/// `--project`, whose files then narrow or locate the selection.
fn navigation(
    operation: &'static Operation,
    outline: bool,
    flags: Flags,
) -> Result<Request, Failure> {
    let project = flags.value(FlagId::Project);
    let direct = flags.has(FlagId::Direct);
    if direct && project.is_none() {
        return Err(Failure::new(
            Code::FlagScope,
            "--direct applies to project queries; file navigation always runs directly",
        ));
    }
    let file = match flags.files.first() {
        Some(file) if file == "-" => {
            return Err(Failure::new(
                Code::Operands,
                format!("{operation} reads a named FILE, not stdin"),
            ));
        }
        file => file.cloned(),
    };
    let query = |target, depth| {
        Ok(Request::Query(Query {
            target,
            format: flags.format(),
            pretty: flags.has(FlagId::Pretty),
            depth,
            output: flags.value(FlagId::Output),
            display: flags.display(operation, false)?,
        }))
    };
    if outline {
        let depth = match flags.value(FlagId::Depth).as_deref() {
            None => Depth::default(),
            Some("all") => Depth::All,
            Some(text) => match text.parse::<usize>() {
                Ok(levels) if levels > 0 => Depth::Levels(levels),
                _ => {
                    return Err(Failure::new(
                        Code::InvalidValue,
                        format!("--depth expects a positive number of levels or all, got {text}"),
                    ));
                }
            },
        };
        let file =
            PathBuf::from(file.ok_or_else(|| {
                Failure::new(Code::Operands, format!("{operation} requires a FILE"))
            })?);
        let target = match project {
            Some(config) => Target::Project {
                config,
                request: ProjectRequest::Outline { file },
                direct,
            },
            None => Target::File {
                file,
                operation: FileOperation::Outline,
            },
        };
        return query(target, depth);
    }
    let target = match (flags.value(FlagId::At), flags.value(FlagId::Name), project) {
        (Some(at), None, project) => {
            let (file, position) = location(&at, file, &format!("{operation} {}", FlagId::At))?;
            match project {
                Some(config) => Target::Project {
                    config,
                    request: ProjectRequest::Show(ShowQuery::At { file, position }),
                    direct,
                },
                None => Target::File {
                    file,
                    operation: FileOperation::Show(Selector::At(position)),
                },
            }
        }
        (None, Some(name), Some(config)) => Target::Project {
            config,
            request: ProjectRequest::Show(ShowQuery::Name {
                name,
                file: file.map(PathBuf::from),
            }),
            direct,
        },
        (None, Some(name), None) => Target::File {
            file: PathBuf::from(file.ok_or_else(|| {
                Failure::new(
                    Code::Operands,
                    format!("{operation} requires a FILE without --project"),
                )
            })?),
            operation: FileOperation::Show(Selector::Name(name)),
        },
        _ => return Err(one_selection(operation)),
    };
    query(target, Depth::default())
}

/// An editing command delivering `action` as the flags ask.
fn edit_command(
    operation: &'static Operation,
    flags: &Flags,
    action: EditAction,
) -> Result<Request, Failure> {
    Ok(Request::Edit(EditCommand {
        action,
        format: flags.format(),
        pretty: flags.has(FlagId::Pretty),
        output: flags.value(FlagId::Output),
        display: flags.display(operation, true)?,
    }))
}

/// The project a prepared edit selects in, as spelled, and whether it is
/// answered directly; file edits always run directly.
fn edit_project(flags: &Flags) -> Result<Option<(String, bool)>, Failure> {
    let direct = flags.has(FlagId::Direct);
    match flags.value(FlagId::Project) {
        Some(config) => Ok(Some((config, direct))),
        None if direct => Err(Failure::new(
            Code::FlagScope,
            "--direct applies to project edits; file edits always run directly",
        )),
        None => Ok(None),
    }
}

/// Applying a saved plan, which `refuse_selection` keeps to the plan alone.
fn apply(operation: &'static Operation, flags: Flags) -> Result<Request, Failure> {
    let plan = flags
        .value(FlagId::Apply)
        .expect("the flag that selects apply takes the plan");
    edit_command(
        operation,
        &flags,
        EditAction::Apply {
            plan: PathBuf::from(plan),
        },
    )
}

/// A batch file's edits, whose targets and fragments `refuse_selection`
/// keeps in the file.
fn batch(operation: &'static Operation, flags: Flags) -> Result<Request, Failure> {
    let project = edit_project(&flags)?;
    let batch = flags
        .value(FlagId::EditBatch)
        .expect("the flag that selects a batch takes its file");
    edit_command(
        operation,
        &flags,
        EditAction::Prepare {
            source: EditSource::Batch(PathBuf::from(batch)),
            project,
            plan: flags.value(FlagId::Plan).map(PathBuf::from),
        },
    )
}

/// Body replacement or statement insertion at one selection.
fn edit(
    operation: &'static Operation,
    kind: EditOperation,
    flags: Flags,
) -> Result<Request, Failure> {
    let project = edit_project(&flags)?;
    let from = PathBuf::from(flags.value(FlagId::From).ok_or_else(|| {
        Failure::new(
            Code::MissingFlag,
            format!("{operation} requires --from FRAGMENT (or --from - for stdin)"),
        )
    })?);
    let file = match flags.files.first() {
        Some(file) if file == "-" => {
            return Err(Failure::new(
                Code::Operands,
                format!("{operation} edits a named FILE, not stdin"),
            ));
        }
        file => file.cloned(),
    };
    let target = match (flags.value(FlagId::At), flags.value(FlagId::Name)) {
        (Some(at), None) => {
            let (file, position) = location(&at, file, &format!("{operation} {}", FlagId::At))?;
            ShowQuery::At { file, position }
        }
        (None, Some(name)) => {
            if project.is_none() && file.is_none() {
                return Err(Failure::new(
                    Code::Operands,
                    format!(
                        "{operation} {} requires a FILE without --project",
                        FlagId::Name
                    ),
                ));
            }
            ShowQuery::Name {
                name,
                file: file.map(PathBuf::from),
            }
        }
        _ => return Err(one_selection(operation)),
    };
    edit_command(
        operation,
        &flags,
        EditAction::Prepare {
            source: EditSource::One {
                operation: kind,
                target,
                from,
            },
            project,
            plan: flags.value(FlagId::Plan).map(PathBuf::from),
        },
    )
}

/// Why an `at` and a file name no location (`locate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unlocated {
    /// `at` is neither `LINE:COLUMN` nor `FILE:LINE:COLUMN`.
    Malformed,
    /// `at` is `FILE:LINE:COLUMN`, and a file is given beside it.
    FileTwice,
    /// `at` is `LINE:COLUMN`, and no file is given.
    NoFile,
}

/// The file and position `at` and `file` name together: `LINE:COLUMN` with
/// `file`, or `FILE:LINE:COLUMN`, the form results print, without one, read
/// from the right so that FILE may contain `:`, as a Windows path does. LINE
/// is one-based and COLUMN a one-based count of UTF-16 units. The one owner
/// of this syntax, for `--at` with the FILE operand and for a batch edit's
/// `at` with its `file`; each words its refusals for its own spelling.
pub(crate) fn locate(
    at: &str,
    file: Option<PathBuf>,
) -> Result<(PathBuf, SourcePosition), Unlocated> {
    let mut parts = at.rsplitn(3, ':');
    let number = |part: Option<&str>| {
        part.and_then(|part| part.parse::<usize>().ok())
            .filter(|&n| n > 0)
    };
    let column = number(parts.next()).ok_or(Unlocated::Malformed)?;
    let line = number(parts.next()).ok_or(Unlocated::Malformed)?;
    let position = SourcePosition { line, column };
    match (parts.next(), file) {
        (Some(""), _) => Err(Unlocated::Malformed),
        (Some(_), Some(_)) => Err(Unlocated::FileTwice),
        (Some(named), None) => Ok((PathBuf::from(named), position)),
        (None, Some(file)) => Ok((file, position)),
        (None, None) => Err(Unlocated::NoFile),
    }
}

/// The location an `--at` value and the FILE operand of `flag` name
/// (`locate`), or the usage failure that each refusal is.
fn location(
    at: &str,
    operand: Option<String>,
    flag: &str,
) -> Result<(PathBuf, SourcePosition), Failure> {
    locate(at, operand.map(PathBuf::from)).map_err(|unlocated| match unlocated {
        Unlocated::Malformed => Failure::new(
            Code::InvalidValue,
            format!("--at expects LINE:COLUMN or FILE:LINE:COLUMN, got {at}"),
        ),
        Unlocated::FileTwice => Failure::new(
            Code::Operands,
            "--at FILE:LINE:COLUMN names the file; omit the FILE operand",
        ),
        Unlocated::NoFile => Failure::new(
            Code::Operands,
            format!("{flag} needs a FILE operand or FILE:LINE:COLUMN"),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{EditAction, EditOperation, EditSource, Request, ServiceAction, parse};
    use crate::cli::registry::{self, FLAGS, FlagId, Kind, OPERATIONS, OperationId};
    use std::ffi::OsString;
    use std::iter::once;

    /// The request, or its usage failure's message.
    fn parsed(args: &[&str]) -> Result<Request, String> {
        parse(args.iter().map(OsString::from))
            .map_err(|invalid| invalid.failure.message().to_string())
    }

    /// The registered operation a request carries out.
    fn operation(request: &Request) -> Option<OperationId> {
        Some(match request {
            Request::Help(_) | Request::Version => return None,
            Request::Parse(_) => OperationId::Parse,
            Request::Doctor(_) => OperationId::Doctor,
            Request::Service(service) => match service.action {
                ServiceAction::Status => OperationId::ReferencesStatus,
                ServiceAction::Stop => OperationId::ReferencesStop,
                ServiceAction::Serve => OperationId::ReferencesServe,
            },
            Request::Query(query) => match query.target.operation() {
                "outline" => OperationId::Outline,
                "show" => OperationId::Show,
                "type" => OperationId::Type,
                "references" => OperationId::References,
                other => panic!("a query for {other}"),
            },
            Request::Edit(command) => match &command.action {
                EditAction::Apply { .. } => OperationId::Apply,
                EditAction::Prepare {
                    source: EditSource::Batch(_),
                    ..
                } => OperationId::EditBatch,
                EditAction::Prepare {
                    source: EditSource::One { operation, .. },
                    ..
                } => match operation {
                    EditOperation::ReplaceBody => OperationId::ReplaceBody,
                    EditOperation::InsertAfter => OperationId::InsertAfter,
                },
            },
        })
    }

    /// The spellings `args` give where a flag may stand, not as a value.
    fn flag_positions<'a>(args: &[&'a str]) -> Vec<&'a str> {
        let mut flags = Vec::new();
        let mut args = args.iter();
        while let Some(&arg) = args.next() {
            if let Some(flag) = registry::flag(arg) {
                flags.push(arg);
                if let Kind::Value(_) = flag.kind {
                    args.next();
                }
            }
        }
        flags
    }

    /// Contract assertion 1: every operation the parser dispatches has an
    /// entry (`operation` maps every request to one), and every entry's
    /// examples are dispatched to it.
    #[test]
    fn every_example_is_a_request_for_its_operation() {
        for entry in &OPERATIONS {
            for example in entry.examples {
                let request = parsed(example.args)
                    .unwrap_or_else(|message| panic!("{:?}: {message}", example.args));
                assert_eq!(operation(&request), Some(entry.id), "{:?}", example.args);
            }
        }
    }

    /// Contract assertion 2, with `only_registered_spellings_are_options`:
    /// every registered name and alias forms a valid request: an example
    /// gives it where a flag stands, or it asks for help or the version.
    #[test]
    fn every_registered_spelling_is_accepted_by_some_operation() {
        for flag in &FLAGS {
            for spelling in once(flag.name).chain(flag.aliases.iter().copied()) {
                let accepted = match flag.id {
                    FlagId::Help => matches!(parsed(&[spelling, "x"]), Ok(Request::Help(_))),
                    FlagId::Version => matches!(parsed(&[spelling, "x"]), Ok(Request::Version)),
                    _ => OPERATIONS.iter().any(|entry| {
                        entry
                            .examples
                            .iter()
                            .any(|example| flag_positions(example.args).contains(&spelling))
                    }),
                };
                assert!(accepted, "{spelling}");
            }
        }
    }

    /// An argument written as an option is a flag only when the registry
    /// spells it exactly; a lone `-` is stdin.
    #[test]
    fn only_registered_spellings_are_options() {
        let mut unregistered = vec![
            "--".to_string(),
            "-x".into(),
            "-5".into(),
            "--apply=plan.json".into(),
        ];
        for flag in &FLAGS {
            for spelling in once(flag.name).chain(flag.aliases.iter().copied()) {
                if let Err(message) = parsed(&[spelling, "value"]) {
                    assert!(
                        !message.starts_with("Unknown option"),
                        "{spelling}: {message}"
                    );
                }
                unregistered.extend([
                    spelling.to_uppercase(),
                    format!("{spelling}s"),
                    format!("{spelling}="),
                    format!("-{spelling}"),
                    spelling[1..].to_string(),
                ]);
            }
        }
        for arg in unregistered {
            // What remains of a short alias without its dash is an operand.
            if !arg.starts_with('-') || arg == "-" || registry::flag(&arg).is_some() {
                continue;
            }
            let Err(message) = parsed(&[&arg]) else {
                panic!("{arg} is accepted");
            };
            assert!(
                message.starts_with(&format!("Unknown option: {arg}"))
                    && message.ends_with("See tsparse --help"),
                "{arg}: {message}"
            );
        }
        assert_eq!(
            parsed(&["-", "-"]).err(),
            Some("Expected one input file".into())
        );
    }

    /// The string literals of this module's code (its tests excluded).
    fn literals(code: &str) -> Vec<String> {
        let mut literals = Vec::new();
        let mut chars = code.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '/' if chars.peek() == Some(&'/') => {
                    chars.by_ref().find(|&c| c == '\n');
                }
                '"' => {
                    let mut literal = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => literal.extend(chars.next()),
                            '"' => break,
                            c => literal.push(c),
                        }
                    }
                    literals.push(literal);
                }
                _ => {}
            }
        }
        literals
    }

    /// Contract assertion 5: arguments become flags only through the
    /// registry: no literal of this module, or of the failure codes' table,
    /// whose meanings name flags, is a flag's spelling, and every flag a
    /// message or meaning names is registered.
    #[test]
    fn messages_name_registered_flags_and_no_literal_is_a_spelling() {
        for (module, source, sample) in [
            (
                "request.rs",
                include_str!("request.rs"),
                "--direct applies to project",
            ),
            (
                "failure.rs",
                include_str!("failure.rs"),
                "such as --at or --depth",
            ),
        ] {
            let code = &source[..source.find("#[cfg(test)]").unwrap()];
            let literals = literals(code);
            assert!(
                literals.iter().any(|literal| literal.contains(sample)),
                "{module}"
            );
            for literal in literals {
                assert!(
                    registry::flag(&literal).is_none(),
                    "{module}: {literal:?} is a spelling"
                );
                for word in literal.split(|c: char| c.is_whitespace() || ",;()?".contains(c)) {
                    let word = word.trim_end_matches(['.', ':']);
                    if word.len() > 1
                        && word.starts_with('-')
                        && word[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '-')
                    {
                        assert!(
                            registry::flag(word).is_some(),
                            "{module}: {literal:?} names {word}"
                        );
                    }
                }
            }
        }
    }
}
