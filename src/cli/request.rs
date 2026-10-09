//! The one parsed-request boundary: command-line arguments become one
//! executable request or a usage error. Handlers never re-decide which
//! flag combinations make sense.

use super::delivery::Format;
use super::navigation::Depth;
use std::path::PathBuf;
use typescript_parser::editing::Operation;
use typescript_parser::navigation::{Selector, ShowQuery};
use typescript_parser::project::ProjectRequest;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

pub(crate) enum Request {
    Help,
    Version,
    Parse(Parse),
    Service(Service),
    Query(Query),
    Edit(EditCommand),
}

/// An editing command and how its outcomes are delivered.
pub(crate) struct EditCommand {
    pub(crate) action: EditAction,
    pub(crate) format: Format,
    /// Indent the document of `--json`.
    pub(crate) pretty: bool,
    pub(crate) output: Option<String>,
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
        operation: Operation,
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
    /// The flag as written, for the message of platforms without a service.
    #[cfg_attr(unix, allow(dead_code))]
    pub(crate) flag: String,
    pub(crate) project: String,
    pub(crate) pretty: bool,
    pub(crate) output: Option<String>,
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
    /// Indent documents: navigation's with `--json`, and reference results.
    pub(crate) pretty: bool,
    /// The outline levels to display; parsing and the catalog never depend
    /// on it, and other operations ignore it.
    pub(crate) depth: Depth,
    pub(crate) output: Option<String>,
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
            } => "references",
            Self::Project {
                request: ProjectRequest::Edit(_),
                ..
            } => "edit",
        }
    }
}

/// Every flag as written, before validation.
#[derive(Default)]
struct Flags {
    pretty: bool,
    jsx: bool,
    check: bool,
    references: bool,
    references_direct: bool,
    service_action: Option<String>,
    outline: bool,
    show: bool,
    /// `--replace-body` or `--insert-after`.
    edit: Option<Operation>,
    edit_batch: Option<String>,
    apply: Option<String>,
    from: Option<String>,
    plan: Option<String>,
    json: bool,
    direct: bool,
    depth: Option<String>,
    files: Vec<String>,
    output: Option<String>,
    project: Option<String>,
    at: Option<String>,
    name: Option<String>,
    /// The first value option given more than once.
    repeated: Option<&'static str>,
}

impl Flags {
    /// Record `value` for the value option `flag`, which `slot` holds.
    fn set(
        &mut self,
        slot: fn(&mut Self) -> &mut Option<String>,
        flag: &'static str,
        value: String,
    ) {
        if slot(self).replace(value).is_some() {
            self.repeated.get_or_insert(flag);
        }
    }
}

/// The request `args` (without the program name) describe, or a usage error.
pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> Result<Request, String> {
    let mut flags = Flags::default();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| format!("Missing value for {flag}"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(Request::Help),
            "-V" | "--version" => return Ok(Request::Version),
            "--pretty" => flags.pretty = true,
            "--tsx" => flags.jsx = true,
            "--check" => flags.check = true,
            "--references" => flags.references = true,
            "--references-direct" => {
                flags.references = true;
                flags.references_direct = true;
            }
            "--references-serve" | "--references-status" | "--references-stop" => {
                if flags.service_action.replace(arg).is_some() {
                    return Err("Expected one reference service action".into());
                }
                flags.references = true;
            }
            "--outline" => flags.outline = true,
            "--show" => flags.show = true,
            "--replace-body" | "--insert-after" => {
                let operation = if arg == "--replace-body" {
                    Operation::ReplaceBody
                } else {
                    Operation::InsertAfter
                };
                if flags
                    .edit
                    .replace(operation)
                    .is_some_and(|other| other != operation)
                {
                    return Err("Choose one of --replace-body and --insert-after".into());
                }
            }
            "--edit-batch" => flags.set(
                |f| &mut f.edit_batch,
                "--edit-batch",
                value("--edit-batch")?,
            ),
            "--apply" => flags.set(|f| &mut f.apply, "--apply", value("--apply")?),
            "--from" => flags.set(|f| &mut f.from, "--from", value("--from")?),
            "--plan" => flags.set(|f| &mut f.plan, "--plan", value("--plan")?),
            "--json" => flags.json = true,
            "--direct" => flags.direct = true,
            "--depth" => flags.set(|f| &mut f.depth, "--depth", value("--depth")?),
            "-o" | "--output" => flags.set(|f| &mut f.output, "-o", value("-o")?),
            "--project" => flags.set(|f| &mut f.project, "--project", value("--project")?),
            "--at" => flags.set(|f| &mut f.at, "--at", value("--at")?),
            "--name" => flags.set(|f| &mut f.name, "--name", value("--name")?),
            "-" => flags.files.push(arg),
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ => flags.files.push(arg),
        }
    }
    if flags.files.len() > 1 {
        return Err("Expected one input file".into());
    }
    if [
        flags.references,
        flags.outline,
        flags.show,
        flags.edit.is_some(),
        flags.edit_batch.is_some(),
        flags.apply.is_some(),
    ]
    .into_iter()
    .filter(|&operation| operation)
    .count()
        > 1
    {
        return Err("Choose one operation: --references, --outline, --show, --replace-body, --insert-after, --edit-batch, or --apply".into());
    }
    if flags.edit.is_none()
        && flags.edit_batch.is_none()
        && (flags.from.is_some() || flags.plan.is_some())
    {
        return Err(
            "--from and --plan apply to --replace-body, --insert-after, and --edit-batch".into(),
        );
    }
    if flags.edit.is_some() || flags.edit_batch.is_some() || flags.apply.is_some() {
        editing(flags)
    } else if flags.references {
        references(flags)
    } else if flags.outline || flags.show {
        navigation(flags)
    } else {
        parse_mode(flags)
    }
}

fn parse_mode(flags: Flags) -> Result<Request, String> {
    if flags.project.is_some() || flags.at.is_some() || flags.name.is_some() {
        return Err(
            "--project, --at, and --name require --references, --outline, or --show".into(),
        );
    }
    navigation_only(&flags)?;
    if flags.direct {
        return Err("--direct applies to project queries".into());
    }
    Ok(Request::Parse(Parse {
        file: flags.files.into_iter().next(),
        pretty: flags.pretty,
        jsx: flags.jsx,
        check: flags.check,
        output: flags.output,
    }))
}

/// Parse and reference output is always JSON and has no levels.
fn navigation_only(flags: &Flags) -> Result<(), String> {
    if flags.json {
        return Err("--json applies to --outline and --show".into());
    }
    if flags.depth.is_some() {
        return Err("--depth applies to --outline".into());
    }
    Ok(())
}

fn references(flags: Flags) -> Result<Request, String> {
    navigation_only(&flags)?;
    if flags.check || flags.jsx {
        return Err("--check and --tsx do not apply to --references".into());
    }
    let project = flags
        .project
        .ok_or("--references requires --project TSCONFIG")?;
    if let Some(flag) = flags.service_action {
        if flags.at.is_some()
            || flags.name.is_some()
            || !flags.files.is_empty()
            || flags.references_direct
            || flags.direct
        {
            return Err("Service actions do not take a reference query".into());
        }
        let action = match flag.as_str() {
            "--references-status" => ServiceAction::Status,
            "--references-stop" => ServiceAction::Stop,
            _ => ServiceAction::Serve,
        };
        return Ok(Request::Service(Service {
            action,
            flag,
            project,
            pretty: flags.pretty,
            output: flags.output,
        }));
    }
    let file = flags.files.into_iter().next();
    if file.as_deref() == Some("-") {
        return Err("--references reads files from the project, not stdin".into());
    }
    let query = match (flags.at, flags.name) {
        (Some(at), None) => {
            let (file, position) = location(&at, file, "--references --at")?;
            ReferenceQuery::At { file, position }
        }
        (None, Some(name)) => {
            // Reference candidates are identifiers, which never contain
            // `.`; qualified names are navigation's containment language.
            if name.contains('.') {
                return Err(format!(
                    "--references --name takes one identifier, not {name}; select a declaration by containment with --show --name, or a token with --at FILE:LINE:COLUMN"
                ));
            }
            ReferenceQuery::Name {
                name,
                file: file.map(PathBuf::from),
            }
        }
        _ => return Err("--references requires exactly one of --at or --name".into()),
    };
    Ok(Request::Query(Query {
        target: Target::Project {
            config: project,
            request: ProjectRequest::References(query),
            direct: flags.references_direct || flags.direct,
        },
        // A reference result's one form is its document; failures are
        // diagnostics.
        format: Format::Text,
        pretty: flags.pretty,
        depth: Depth::default(),
        output: flags.output,
    }))
}

/// Outline and show: file navigation, or a project request with
/// `--project`, whose files then narrow or locate the selection.
fn navigation(flags: Flags) -> Result<Request, String> {
    let operation = if flags.outline { "--outline" } else { "--show" };
    // Parse and reference requests keep their established last-value rule.
    if let Some(flag) = flags.repeated {
        return Err(format!("{operation} takes {flag} at most once"));
    }
    if flags.direct && flags.project.is_none() {
        return Err(
            "--direct applies to project queries; file navigation always runs directly".into(),
        );
    }
    if flags.check || flags.jsx {
        return Err(format!("--check and --tsx do not apply to {operation}"));
    }
    if flags.pretty && !flags.json {
        return Err(format!("--pretty applies to {operation} only with --json"));
    }
    let file = match flags.files.into_iter().next() {
        Some(file) if file == "-" => {
            return Err(format!("{operation} reads a named FILE, not stdin"));
        }
        file => file,
    };
    let format = if flags.json {
        Format::Json
    } else {
        Format::Text
    };
    let query = |target, depth| {
        Ok(Request::Query(Query {
            target,
            format,
            pretty: flags.pretty,
            depth,
            output: flags.output,
        }))
    };
    if flags.outline {
        if flags.at.is_some() || flags.name.is_some() {
            return Err("--outline does not take --at or --name".into());
        }
        let depth = match flags.depth.as_deref() {
            None => Depth::default(),
            Some("all") => Depth::All,
            Some(text) => match text.parse::<usize>() {
                Ok(levels) if levels > 0 => Depth::Levels(levels),
                _ => {
                    return Err(format!(
                        "--depth expects a positive number of levels or all, got {text}"
                    ));
                }
            },
        };
        let file = PathBuf::from(file.ok_or("--outline requires a FILE")?);
        let target = match flags.project {
            Some(config) => Target::Project {
                config,
                request: ProjectRequest::Outline { file },
                direct: flags.direct,
            },
            None => Target::File {
                file,
                operation: FileOperation::Outline,
            },
        };
        return query(target, depth);
    }
    if flags.depth.is_some() {
        return Err("--depth applies to --outline".into());
    }
    let direct = flags.direct;
    let target = match (flags.at, flags.name, flags.project) {
        (Some(at), None, project) => {
            let (file, position) = location(&at, file, "--show --at")?;
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
            file: PathBuf::from(file.ok_or("--show requires a FILE without --project")?),
            operation: FileOperation::Show(Selector::Name(name)),
        },
        _ => return Err("--show requires exactly one of --at or --name".into()),
    };
    query(target, Depth::default())
}

/// Body replacement, statement insertion, batches, and applying a plan.
fn editing(flags: Flags) -> Result<Request, String> {
    let operation = match (flags.edit, &flags.edit_batch, &flags.apply) {
        (Some(Operation::ReplaceBody), _, _) => "--replace-body",
        (Some(Operation::InsertAfter), _, _) => "--insert-after",
        (None, Some(_), _) => "--edit-batch",
        _ => "--apply",
    };
    if let Some(flag) = flags.repeated {
        return Err(format!("{operation} takes {flag} at most once"));
    }
    if flags.check || flags.jsx || flags.depth.is_some() {
        return Err(format!(
            "--check, --tsx, and --depth do not apply to {operation}"
        ));
    }
    if flags.pretty && !flags.json {
        return Err(format!("--pretty applies to {operation} only with --json"));
    }
    if flags.direct && flags.project.is_none() {
        return Err("--direct applies to project edits; file edits always run directly".into());
    }
    let format = if flags.json {
        Format::Json
    } else {
        Format::Text
    };
    let command = |action| {
        Ok(Request::Edit(EditCommand {
            action,
            format,
            pretty: flags.pretty,
            output: flags.output.clone(),
        }))
    };
    if let Some(plan) = &flags.apply {
        if flags.project.is_some()
            || flags.at.is_some()
            || flags.name.is_some()
            || !flags.files.is_empty()
        {
            return Err("--apply takes only the plan: it applies the recorded files and edits without selecting them again".into());
        }
        return command(EditAction::Apply {
            plan: PathBuf::from(plan),
        });
    }
    let project = flags.project.clone().map(|config| (config, flags.direct));
    let plan = flags.plan.clone().map(PathBuf::from);
    if let Some(batch) = &flags.edit_batch {
        if flags.at.is_some()
            || flags.name.is_some()
            || flags.from.is_some()
            || !flags.files.is_empty()
        {
            return Err(
                "--edit-batch names every edit's target and fragment in the batch file".into(),
            );
        }
        return command(EditAction::Prepare {
            source: EditSource::Batch(PathBuf::from(batch)),
            project,
            plan,
        });
    }
    let from = PathBuf::from(flags.from.clone().ok_or(format!(
        "{operation} requires --from FRAGMENT (or --from - for stdin)"
    ))?);
    let file = match flags.files.into_iter().next() {
        Some(file) if file == "-" => {
            return Err(format!("{operation} edits a named FILE, not stdin"));
        }
        file => file,
    };
    let target = match (flags.at, flags.name) {
        (Some(at), None) => {
            let (file, position) = location(&at, file, &format!("{operation} --at"))?;
            ShowQuery::At { file, position }
        }
        (None, Some(name)) => {
            if project.is_none() && file.is_none() {
                return Err(format!(
                    "{operation} --name requires a FILE without --project"
                ));
            }
            ShowQuery::Name {
                name,
                file: file.map(PathBuf::from),
            }
        }
        _ => {
            return Err(format!(
                "{operation} requires exactly one of --at or --name"
            ));
        }
    };
    command(EditAction::Prepare {
        source: EditSource::One {
            operation: flags.edit.expect("a single edit has an operation"),
            target,
            from,
        },
        project,
        plan,
    })
}

/// The file and position an `--at` value and the FILE operand name
/// together: `--at LINE:COLUMN FILE`, or `--at FILE:LINE:COLUMN` with no
/// operand, the form results print, read from the right so that FILE may
/// contain `:`, as a Windows path does. LINE is one-based and COLUMN a
/// one-based count of UTF-16 units.
pub(crate) fn location(
    at: &str,
    operand: Option<String>,
    flag: &str,
) -> Result<(PathBuf, SourcePosition), String> {
    let invalid = || format!("--at expects LINE:COLUMN or FILE:LINE:COLUMN, got {at}");
    let mut parts = at.rsplitn(3, ':');
    let number = |part: Option<&str>| {
        part.and_then(|part| part.parse::<usize>().ok())
            .filter(|&n| n > 0)
    };
    let column = number(parts.next()).ok_or_else(invalid)?;
    let line = number(parts.next()).ok_or_else(invalid)?;
    let position = SourcePosition { line, column };
    match (parts.next(), operand) {
        (Some(""), _) => Err(invalid()),
        (Some(_), Some(_)) => {
            Err("--at FILE:LINE:COLUMN names the file; omit the FILE operand".into())
        }
        (Some(file), None) => Ok((PathBuf::from(file), position)),
        (None, Some(file)) => Ok((PathBuf::from(file), position)),
        (None, None) => Err(format!("{flag} needs a FILE operand or FILE:LINE:COLUMN")),
    }
}
