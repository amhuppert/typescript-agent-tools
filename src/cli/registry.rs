//! The CLI vocabulary: every flag `tsparse` accepts, with its aliases,
//! kind, summary, and repeat policy; every operation, with its flag,
//! summary, details, usage, accepted flags, examples, exit codes, related
//! operations, document, repeated-value policy, `--pretty` rule, and the
//! lists it bounds with their default caps; and what each exit code means. The one owner of that vocabulary: the
//! parser's flag match and flag scopes (`super::request`), the flags
//! printed commands leave unquoted (`super::command_line`), and help and
//! the generated reference (`super::help`) derive from these tables.

use crate::process_limits::{
    default_idle_secs, default_memory_mb, default_service_dir_prefix, default_timeout_ms,
};
use FlagId as F;
use std::fmt;

/// A registered flag. The parser matches these, never a spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FlagId {
    Help,
    Version,
    Outline,
    Show,
    Type,
    References,
    Constructs,
    ReplaceBody,
    InsertAfter,
    EditBatch,
    Apply,
    ReferencesStatus,
    ReferencesStop,
    ReferencesServe,
    Doctor,
    Project,
    Direct,
    At,
    Name,
    Depth,
    Limit,
    Expand,
    Calls,
    NoTests,
    File,
    Missing,
    AllLiterals,
    From,
    Plan,
    Tsx,
    Check,
    Json,
    Pretty,
    Output,
}

impl FlagId {
    const COUNT: usize = Self::Output as usize + 1;

    /// This flag's entry.
    pub(crate) fn flag(self) -> &'static Flag {
        &FLAGS[self as usize]
    }
}

/// A flag's registered name.
impl fmt::Display for FlagId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.flag().name)
    }
}

/// Whether a flag takes the next argument as its value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Boolean,
    /// The value's placeholder in usage.
    Value(&'static str),
}

/// What a flag given more than once does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Repeat {
    /// Nothing: a switch that is on stays on.
    Redundant,
    /// What the operation's [`RepeatedValue`] says: every value-taking flag.
    PerOperation,
    /// A usage error in every operation.
    Reject,
}

pub(crate) struct Flag {
    pub(crate) id: FlagId,
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) kind: Kind,
    pub(crate) summary: &'static str,
    pub(crate) repeat: Repeat,
}

/// A switch, which a repeat leaves on.
const fn switch(id: FlagId, name: &'static str, summary: &'static str) -> Flag {
    Flag {
        id,
        name,
        aliases: &[],
        kind: Kind::Boolean,
        summary,
        repeat: Repeat::Redundant,
    }
}

/// A flag whose value is the next argument, whatever it begins with.
const fn value(
    id: FlagId,
    name: &'static str,
    placeholder: &'static str,
    summary: &'static str,
) -> Flag {
    Flag {
        id,
        name,
        aliases: &[],
        kind: Kind::Value(placeholder),
        summary,
        repeat: Repeat::PerOperation,
    }
}

/// A service action, which is given once.
const fn action(id: FlagId, name: &'static str, summary: &'static str) -> Flag {
    Flag {
        repeat: Repeat::Reject,
        ..switch(id, name, summary)
    }
}

/// Every flag, in [`FlagId`] order.
pub(crate) static FLAGS: [Flag; FlagId::COUNT] = [
    Flag {
        aliases: &["-h"],
        ..switch(FlagId::Help, "--help", "Print help and exit.")
    },
    Flag {
        aliases: &["-V"],
        ..switch(FlagId::Version, "--version", "Print the version and exit.")
    },
    switch(
        FlagId::Outline,
        "--outline",
        "List one file's declarations, indented by containment.",
    ),
    switch(
        FlagId::Show,
        "--show",
        "Print one declaration's exact source with its line numbers.",
    ),
    switch(
        FlagId::Type,
        "--type",
        "Inspect the type of a declaration or token in one project.",
    ),
    switch(
        FlagId::References,
        "--references",
        "Group a symbol's declarations and occurrences in one project.",
    ),
    switch(
        FlagId::Constructs,
        "--constructs",
        "List the object literals that construct a type in one project, and the properties each lacks.",
    ),
    switch(
        FlagId::ReplaceBody,
        "--replace-body",
        "Preview replacing the complete body of the selected declaration.",
    ),
    switch(
        FlagId::InsertAfter,
        "--insert-after",
        "Preview inserting statements after the selected declaration's statement.",
    ),
    value(
        FlagId::EditBatch,
        "--edit-batch",
        "EDITS",
        "Preview the edits a JSON batch file names, selected together.",
    ),
    value(
        FlagId::Apply,
        "--apply",
        "PLAN",
        "Write a saved plan's exact bytes after checking every file it names.",
    ),
    action(
        FlagId::ReferencesStatus,
        "--references-status",
        "Report whether the project's service is running.",
    ),
    action(
        FlagId::ReferencesStop,
        "--references-stop",
        "Stop the project's service.",
    ),
    action(
        FlagId::ReferencesServe,
        "--references-serve",
        "Run the project's service in the foreground.",
    ),
    action(
        FlagId::Doctor,
        "--doctor",
        "Check what project queries depend on, one line per check.",
    ),
    value(
        FlagId::Project,
        "--project",
        "TSCONFIG",
        "The project's configuration file, as spelled; no ancestor directory is searched.",
    ),
    switch(
        FlagId::Direct,
        "--direct",
        "Load a fresh project in this process instead of asking the project's service.",
    ),
    value(
        FlagId::At,
        "--at",
        "[FILE:]LINE:COLUMN",
        "Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand.",
    ),
    value(
        FlagId::Name,
        "--name",
        "NAME",
        "Select by exact declared name; a dotted name such as RunHost.run selects by containment.",
    ),
    value(
        FlagId::Depth,
        "--depth",
        "DEPTH",
        "Outline levels to show (N or all, default 2), or type edges to expand (0 through 32, default 3).",
    ),
    value(
        FlagId::Limit,
        "--limit",
        "N|all",
        "Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes.",
    ),
    switch(
        FlagId::Expand,
        "--expand",
        "Add a bounded structural graph of the type.",
    ),
    switch(
        FlagId::Calls,
        "--calls",
        "Keep only direct call, new, and tagged-template targets.",
    ),
    switch(
        FlagId::NoTests,
        "--no-tests",
        "Omit results in test paths: references keep the selected declarations, and constructs skips the literals there.",
    ),
    value(
        FlagId::File,
        "--file",
        "FILE",
        "Keep only results in FILE, one of the searched files: occurrences, or construction sites; unlike the FILE operand, it does not narrow the candidates.",
    ),
    value(
        FlagId::Missing,
        "--missing",
        "KEY",
        "Keep only the construction sites whose constructed type declares the string key KEY, compared exactly, and that do not certainly provide it.",
    ),
    switch(
        FlagId::AllLiterals,
        "--all-literals",
        "Examine every object literal, also those whose named members name only keys the target does not declare; several times slower on a large project.",
    ),
    value(
        FlagId::From,
        "--from",
        "FRAGMENT",
        "The file holding the new body or statements; - reads stdin.",
    ),
    value(
        FlagId::Plan,
        "--plan",
        "PLAN",
        "Save the prepared edits to PLAN, a new file, for --apply.",
    ),
    switch(
        FlagId::Tsx,
        "--tsx",
        "Parse JSX, as a .tsx or .jsx file name already does.",
    ),
    switch(
        FlagId::Check,
        "--check",
        "Validate syntax, printing one verdict line instead of the AST.",
    ),
    switch(
        FlagId::Json,
        "--json",
        "Write one result document, also for a failure the analysis reports.",
    ),
    switch(
        FlagId::Pretty,
        "--pretty",
        "Indent JSON output: alone where the output is JSON, and with --json elsewhere.",
    ),
    Flag {
        aliases: &["-o"],
        ..value(
            FlagId::Output,
            "--output",
            "OUTPUT",
            "Write the result to OUTPUT instead of stdout; editing writes it only as a new file.",
        )
    },
];

/// The flags every operation accepts, which its own list leaves out.
pub(crate) static GLOBAL_FLAGS: &[FlagId] = &[FlagId::Help, FlagId::Version];

/// The output flags the help index describes once; each operation lists
/// those it accepts.
pub(crate) static OUTPUT_FLAGS: &[FlagId] = &[FlagId::Json, FlagId::Pretty, FlagId::Output];

/// What an exit code reports, for every operation or for one.
pub(crate) struct ExitMeaning {
    pub(crate) code: u8,
    pub(crate) meaning: &'static str,
}

/// What each exit code means for every operation. `super::delivery`
/// decides the code of each outcome; this states those decisions, and each
/// operation's [`Operation::exits`] names its own outcomes.
pub(crate) static EXIT_CODES: [ExitMeaning; 3] = [
    ExitMeaning {
        code: 0,
        meaning: "Success: a complete result, a preview, or a plan applied or changing nothing.",
    },
    ExitMeaning {
        code: 1,
        meaning: "No complete answer: a syntax error, an analysis failure, an ambiguous or missing selection, a partial result, a rejected edit, or a plan that no longer applies to the files.",
    },
    ExitMeaning {
        code: 2,
        meaning: "A usage error, an input that cannot be read, an invalid position or a file outside the project, a service that cannot be started or reached, a failed write, or a rollback or recovery while applying.",
    },
];

/// The registered flag spelled exactly `spelling`, by name or alias.
pub(crate) fn flag(spelling: &str) -> Option<&'static Flag> {
    FLAGS
        .iter()
        .find(|flag| flag.name == spelling || flag.aliases.contains(&spelling))
}

/// The registered flag nearest `spelling`, an unknown option, when one is
/// close. TypeScript asks this of an unknown command-line option with
/// `getSpellingSuggestion` over its option names (`createUnknownOptionError`),
/// so it is asked of the library's owner of that rule, over names only, as
/// TypeScript offers no short alias. Names are compared as written, dashes
/// included, because here the prefix is part of the name: `-output` is not
/// `--output`.
pub(crate) fn nearest(spelling: &str) -> Option<&'static Flag> {
    let name =
        typescript_parser::spelling_suggestion(spelling, FLAGS.iter().map(|flag| flag.name))?;
    self::flag(name)
}

/// A registered operation, which the parser dispatches on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OperationId {
    Parse,
    Outline,
    Show,
    Type,
    References,
    Constructs,
    ReplaceBody,
    InsertAfter,
    EditBatch,
    Apply,
    ReferencesStatus,
    ReferencesStop,
    ReferencesServe,
    Doctor,
}

impl OperationId {
    const COUNT: usize = Self::Doctor as usize + 1;

    /// This operation's entry.
    pub(crate) fn operation(self) -> &'static Operation {
        &OPERATIONS[self as usize]
    }
}

/// What an operation does with a value-taking flag given more than once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RepeatedValue {
    /// The last value is used.
    LastValue,
    /// A usage error.
    Reject,
}

/// Where `--pretty` is valid, for an operation that takes it. It indents
/// JSON output, so it stands alone only where the output is JSON and needs
/// `--json` elsewhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pretty {
    /// The output, if any, is JSON: the AST, or a reference answer or a
    /// service's status, whose text form `--pretty` alone replaces with its
    /// indented document (decisions.md, step 4).
    Alone,
    /// The output is text unless `--json` is given.
    WithJson,
}

/// A list whose display an operation bounds: `--limit` sets the bound, and
/// without it the list's default applies unless `-o` is given (design,
/// step 4: "Rules shared by every renderer").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum List {
    Occurrences,
    Entries,
    Candidates,
    Lines,
    DiffLines,
    Sites,
    Lacks,
    Gaps,
}

impl List {
    /// Its name in help and in a document's `display`.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Occurrences => "occurrences",
            Self::Entries => "entries",
            Self::Candidates => "candidates",
            Self::Lines => "lines",
            Self::DiffLines => "diff_lines",
            Self::Sites => "sites",
            Self::Lacks => "lacks",
            Self::Gaps => "gaps",
        }
    }
}

/// A bounded list, with the bound it has when `--limit` is not given.
pub(crate) struct Bounded {
    pub(crate) list: List,
    pub(crate) default: usize,
    /// What it bounds, as help states it.
    pub(crate) summary: &'static str,
}

/// The default caps, the one place they are named: the design's starting
/// values, which calibrating against real projects may change
/// (decisions.md, step 4). Every operation that bounds a list names one of
/// these, and so does every renderer that cuts it (`super::delivery::Cap`).
pub(crate) const OCCURRENCES: Bounded = Bounded {
    list: List::Occurrences,
    default: 200,
    summary: "occurrences counted across groups",
};
pub(crate) const ENTRIES: Bounded = Bounded {
    list: List::Entries,
    default: 200,
    summary: "entries of the levels --depth shows",
};
pub(crate) const CANDIDATES: Bounded = Bounded {
    list: List::Candidates,
    default: 50,
    summary: "candidates of an ambiguous selection",
};
pub(crate) const LINES: Bounded = Bounded {
    list: List::Lines,
    default: 400,
    summary: "lines of the declaration from its first line",
};
pub(crate) const SITES: Bounded = Bounded {
    list: List::Sites,
    default: 200,
    summary: "construction sites",
};
pub(crate) const LACKS: Bounded = Bounded {
    list: List::Lacks,
    default: 20,
    summary: "properties each construction lacks",
};
pub(crate) const GAPS: Bounded = Bounded {
    list: List::Gaps,
    default: 50,
    summary: "gaps of a partial answer",
};
pub(crate) const DIFF_LINES: Bounded = Bounded {
    list: List::DiffLines,
    default: 400,
    summary: "diff lines per changed file",
};

/// A command line that teaches one shape of an operation.
pub(crate) struct Example {
    /// The arguments after the program name.
    pub(crate) args: &'static [&'static str],
    pub(crate) summary: &'static str,
}

pub(crate) struct Operation {
    pub(crate) id: OperationId,
    pub(crate) name: &'static str,
    /// The flag that selects it; parse mode is the absence of every
    /// operation's flag.
    pub(crate) flag: Option<FlagId>,
    /// One line, for the help index.
    pub(crate) summary: &'static str,
    /// What a caller needs beyond the summary: selection, inputs, and
    /// effects. Its document has the rest.
    pub(crate) details: &'static str,
    pub(crate) usage: &'static [&'static str],
    /// The flags it accepts besides its own flag and [`GLOBAL_FLAGS`].
    pub(crate) flags: &'static [FlagId],
    /// Each a request for this operation whose arguments a shell reads as
    /// written.
    pub(crate) examples: &'static [Example],
    /// What each exit code reports for this operation.
    pub(crate) exits: &'static [ExitMeaning],
    pub(crate) related: &'static [OperationId],
    /// Its document, relative to the repository.
    pub(crate) doc: &'static str,
    pub(crate) repeat: RepeatedValue,
    /// Where `--pretty` is valid; `None` where the operation does not take
    /// it.
    pub(crate) pretty: Option<Pretty>,
    /// The lists it bounds, which `--limit` sets: an operation accepts
    /// `--limit` exactly when it has one.
    pub(crate) limits: &'static [Bounded],
}

impl Operation {
    /// Every flag a request for this operation may give, in the order help
    /// lists them: its own flag, its list, and [`GLOBAL_FLAGS`].
    pub(crate) fn accepted(&self) -> impl Iterator<Item = FlagId> {
        self.flag
            .into_iter()
            .chain(self.flags.iter().copied())
            .chain(GLOBAL_FLAGS.iter().copied())
    }

    /// Whether a request for this operation may give `flag`.
    pub(crate) fn accepts(&self, flag: FlagId) -> bool {
        self.accepted().any(|accepted| accepted == flag)
    }
}

/// How messages name an operation: by its flag, and parse mode by the
/// absence of one.
impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.flag {
            Some(flag) => flag.fmt(f),
            None => f.write_str("parse mode (no operation flag)"),
        }
    }
}

/// The operation `flag` selects: the one whose flag it is.
pub(crate) fn selected_by(flag: FlagId) -> Option<&'static Operation> {
    OPERATIONS
        .iter()
        .find(|operation| operation.flag == Some(flag))
}

/// The operations that accept `flag`, in registry order.
pub(crate) fn accepting(flag: FlagId) -> impl Iterator<Item = &'static Operation> {
    OPERATIONS
        .iter()
        .filter(move |operation| operation.accepts(flag))
}

const EDIT_FLAGS: &[FlagId] = &[
    F::Project,
    F::Direct,
    F::At,
    F::Name,
    F::From,
    F::Plan,
    F::Limit,
    F::Json,
    F::Pretty,
    F::Output,
];

/// Stop prints only its verdict line on stdout and serve only a failure,
/// neither a document, so neither takes an output flag.
const SERVICE_FLAGS: &[FlagId] = &[F::Project];

/// Status prints text, or with `--json` its document (decisions.md,
/// step 5).
const STATUS_FLAGS: &[FlagId] = &[F::Project, F::Json, F::Pretty, F::Output];

/// An edit's selection, as show's, and each changed file's diff.
const EDIT_LIMITS: &[Bounded] = &[CANDIDATES, DIFF_LINES];

/// The exit codes of outline and show differ only in what they select.
const NAVIGATION_FAILURES: ExitMeaning = ExitMeaning {
    code: 2,
    meaning: "A usage error, a file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result that cannot be written.",
};

/// The exit codes of a previewed edit, wherever its edits come from.
const PREVIEW: ExitMeaning = ExitMeaning {
    code: 0,
    meaning: "A preview, or no_change when the edits change nothing; with --plan, the plan is saved.",
};
const UNPREPARED: ExitMeaning = ExitMeaning {
    code: 1,
    meaning: "An ambiguous or missing selection, a rejected edit, or an analysis failure such as a syntax error.",
};
const EDIT_EXITS: &[ExitMeaning] = &[
    PREVIEW,
    UNPREPARED,
    ExitMeaning {
        code: 2,
        meaning: "A usage error, a fragment or file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result or plan that cannot be written.",
    },
];

/// What every editing preview shares, as a literal `concat!` takes.
macro_rules! preview_details {
    () => {
        "The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N."
    };
}

/// What the three service actions administer, whatever their names say: the
/// one service that answers every project query (design, step 5, "Refusal
/// wording"). The help index states it, and so does each action's entry.
macro_rules! service_administration {
    () => {
        "--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --type, --constructs, and --outline, --show, and edits with --project."
    };
}

/// [`service_administration`] as the help index states it.
pub(crate) const SERVICE_ADMINISTRATION: &str = service_administration!();

/// Every operation, in [`OperationId`] order.
pub(crate) static OPERATIONS: [Operation; OperationId::COUNT] = [
    Operation {
        id: OperationId::Parse,
        name: "parse",
        flag: None,
        summary: "With no operation flag: parse one TypeScript file, or stdin, into a compiler-style JSON AST, or with --check only validate it.",
        details: "Reads stdin when FILE is omitted or is -. A .tsx or .jsx file name, in any case, enables JSX, as --tsx does for stdin. The AST has TypeScript's node kinds and positions; a syntax error is FILE:LINE:COLUMN: message on stderr, with a one-based line and UTF-16 column. --check prints no AST but one line on stdout, ok: FILE parses as TypeScript 7 (jsx: off), or jsx: on, with - for stdin; it checks syntax only, and --pretty and --output change nothing then.",
        usage: &["tsparse [--tsx] [--check] [--pretty] [--output OUTPUT] [FILE|-]"],
        flags: &[F::Tsx, F::Check, F::Pretty, F::Output],
        examples: &[
            Example {
                args: &["--pretty", "example.ts"],
                summary: "Print the indented AST of example.ts.",
            },
            Example {
                args: &["--check", "--tsx"],
                summary: "Validate stdin as TSX, printing ok: - parses as TypeScript 7 (jsx: on) instead of the AST.",
            },
            Example {
                args: &["-o", "view.ast.json", "view.tsx"],
                summary: "Write the AST to a file; the .tsx name enables JSX.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "The AST was written, or with --check the input parses and its verdict line was printed.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A syntax error, as FILE:LINE:COLUMN: message on stderr (- for stdin).",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, an input that cannot be read as UTF-8, or a result that cannot be written.",
            },
        ],
        related: &[OperationId::Outline, OperationId::Show],
        doc: "docs/ast-json.md",
        repeat: RepeatedValue::LastValue,
        pretty: Some(Pretty::Alone),
        limits: &[],
    },
    Operation {
        id: OperationId::Outline,
        name: "outline",
        flag: Some(F::Outline),
        summary: "List the declarations of one file in source order, one per line, indented by containment.",
        details: "Each line is LINE:COLUMN, kind, and signature or label; an import declaration takes one line, at its first name, listing the names it imports. --depth shows N levels (default 2; all shows every level) and counts what it hides. Without --project only FILE is read. With --project, FILE must be one of the files --references searches, the project's service answers (this process with --direct), and a project that fails to load answers nothing, never falling back to FILE alone. Text names files absolutely; project JSON names them relative to the configuration's directory.",
        usage: &[
            "tsparse --outline FILE [--depth N|all] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
            "tsparse --outline --project TSCONFIG FILE [--direct] [--depth N|all] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::Depth,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[
            Example {
                args: &["--outline", "src/app.ts"],
                summary: "Two levels of declarations, counting what is hidden.",
            },
            Example {
                args: &[
                    "--outline",
                    "--project",
                    "tsconfig.json",
                    "src/app.ts",
                    "--depth",
                    "all",
                    "--json",
                    "--output",
                    "outline.json",
                ],
                summary: "Every level of a project file, as one document in a file.",
            },
            Example {
                args: &["--outline", "src/app.ts", "--limit", "all"],
                summary: "Every entry of the two levels, however many; the default shows 200.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "The outline, which is empty for a file without declarations.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A syntax error, unsupported input, or a project analysis failure.",
            },
            NAVIGATION_FAILURES,
        ],
        related: &[OperationId::Show, OperationId::References],
        doc: "docs/navigation.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[ENTRIES],
    },
    Operation {
        id: OperationId::Show,
        name: "show",
        flag: Some(F::Show),
        summary: "Print one declaration's exact source with its original line numbers.",
        details: "--name selects an exact declared name, and a.b.c selects c inside declarations named b and then a; an import or export specifier has a name only where it renames, and a default import never does. --at selects the declaration whose name or anchor is at the position. Overloads, merged declarations, and same-named locals are separate declarations: an ambiguous or missing selection exits 1 and lists each candidate's FILE:LINE:COLUMN and selector, on stderr or, with --json, in the document. With --project, FILE must be one of the files --references searches, --name without FILE searches all of them, and a printed FILE:LINE:COLUMN selects its declaration from any directory. Text names files absolutely; project JSON names them relative to the configuration's directory.",
        usage: &[
            "tsparse --show (--name NAME FILE | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
            "tsparse --show --project TSCONFIG (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--direct] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::At,
            F::Name,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[
            Example {
                args: &["--show", "--name", "run", "src/host.ts"],
                summary: "A bare name: an ambiguous one lists its candidates.",
            },
            Example {
                args: &[
                    "--show",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "RunHost.run",
                ],
                summary: "A dotted name: run inside RunHost, in any searched file.",
            },
            Example {
                args: &[
                    "--show",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    "src/host.ts:12:3",
                ],
                summary: "The FILE:LINE:COLUMN form results print.",
            },
            Example {
                args: &["--show", "--at", "12:3", "src/host.ts"],
                summary: "The same position, with the file as the operand.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "One declaration found.",
            },
            ExitMeaning {
                code: 1,
                meaning: "An ambiguous or missing selection, a syntax error, unsupported input, or a project analysis failure.",
            },
            NAVIGATION_FAILURES,
        ],
        related: &[
            OperationId::Outline,
            OperationId::Type,
            OperationId::ReplaceBody,
        ],
        doc: "docs/navigation.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[LINES, CANDIDATES],
    },
    Operation {
        id: OperationId::Type,
        name: "type",
        flag: Some(F::Type),
        summary: "Inspect the type of the selected declaration or token in one explicit project.",
        details: concat!(
            "--name takes --show's declaration containment selectors, and FILE narrows the seeds. --expand adds a bounded structural graph, and --depth counts its type edges (default 3, 0 through 32). Semantic dependencies that cannot be established are located failures and partial expansion is explicit; this is not full TypeScript checking. A query runs under TSPARSE_MAX_MEMORY_MB (default ",
            default_memory_mb!(),
            " MiB) and TSPARSE_QUERY_TIMEOUT_MS (default ",
            default_timeout_ms!(),
            "), positive integers checked before any service starts; a service keeps the values it started with, and a limit failure exits the process that hit it."
        ),
        usage: &[
            "tsparse --type --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--expand [--depth 0..32]] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::At,
            F::Name,
            F::Expand,
            F::Depth,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[
            Example {
                args: &[
                    "--type",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    "src/host.ts:42:10",
                ],
                summary: "The type at a position the results print.",
            },
            Example {
                args: &[
                    "--type",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "createRunHost",
                    "--expand",
                    "--depth",
                    "2",
                    "--json",
                    "--direct",
                ],
                summary: "Two type edges of structure, as a document, loaded in this process.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "A complete answer.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A partial answer, an ambiguous or missing selection, an unsupported or unresolved target, an analysis failure, or a memory, time, depth, or output limit.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, a file that cannot be read or another I/O failure, an invalid position, a file outside the project, an internal failure, a service that cannot be started or reached, or a result that cannot be written.",
            },
        ],
        related: &[OperationId::Show, OperationId::References],
        doc: "docs/types.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[CANDIDATES],
    },
    Operation {
        id: OperationId::References,
        name: "references",
        flag: Some(F::References),
        summary: "Group the declarations and occurrences of the selected symbol in one explicit project.",
        details: "--at selects the identifier at the position (one-based line, UTF-16 column). A bare NAME selects every lexical declaration, local import, or exported alias spelled exactly NAME and lists the matching members it did not search; a dotted NAME such as RunHost.observe selects a member, whose identity follows resolved receiver types, not spelling. FILE narrows the candidates; --file FILE then keeps only their occurrences in FILE. Import and export aliases join their underlying symbol; shadowed bindings form separate groups; an incomplete search reports partial coverage and its gaps. Text prints a header with the selection, its counts, and the filters that bound them, each group with its declarations and its occurrences by file (a renamed alias's spelling in parentheses), each gap, and one line counting the members a bare name did not search and naming the --at that searches one; files are named absolutely, and an ambiguous or missing selection is a diagnostic on stderr. --json writes the result document instead, which also delivers analysis failures as documents and names files relative to the configuration's directory; --pretty without --json writes that document indented. Supported: one configuration and its inherited settings; .ts, .tsx, .mts, and .cts sources and declarations; node16, nodenext, and bundler resolution; installed packages with declarations; and JSON modules where resolveJsonModule allows them, though positions inside JSON are not selectable. Standard libraries follow target, lib, noLib, and lib references, from the TypeScript 7.0.2 declarations vendored with this build: their declarations join groups, but their files are never searched. Project references are not followed, so a configuration that needs them is rejected rather than searched partially, and JavaScript inputs are not supported.",
        usage: &[
            "tsparse --references --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--calls] [--no-tests] [--file FILE] [--limit N|all] [--json] [--pretty] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::At,
            F::Name,
            F::Calls,
            F::NoTests,
            F::File,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[
            Example {
                args: &[
                    "--references",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "run",
                ],
                summary: "Every lexical declaration, local import, or exported alias spelled run.",
            },
            Example {
                args: &[
                    "--references",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "run",
                    "src/app.ts",
                    "--file",
                    "src/api.ts",
                ],
                summary: "Only the candidates declared in src/app.ts (the FILE operand), and only their occurrences in src/api.ts (--file).",
            },
            Example {
                args: &[
                    "--references",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "RunHost.observe",
                    "--calls",
                    "--no-tests",
                ],
                summary: "A member, by containment: its calls outside test paths.",
            },
            Example {
                args: &[
                    "--references",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    "12:8",
                    "src/app.ts",
                    "--pretty",
                ],
                summary: "The symbol at a position, as the indented document --pretty writes without --json.",
            },
            Example {
                args: &[
                    "--references",
                    "--direct",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    "src/app.ts:12:8",
                ],
                summary: "Answered in this process instead of by the project's service.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "A complete result, including one with no groups for a token-free position or an unmatched name.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A partial member search, an ambiguous or missing dotted name, an unsupported or unresolved selected target, invalid configuration content or inheritance, a syntax error, unsupported required input, invalid binding, a memory or time limit, or a name resolution looks up and does not find (an import, a types entry, or a /// <reference> target).",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error or invalid position; a file the request or configuration names (the --project file, an extends target, a files entry) that does not exist, or a file or directory that cannot be read; a query file that is not one of the project's searched files; a service that cannot be started or reached; or a result that cannot be written.",
            },
        ],
        related: &[
            OperationId::Show,
            OperationId::Type,
            OperationId::ReferencesStatus,
        ],
        doc: "docs/references.md",
        repeat: RepeatedValue::LastValue,
        pretty: Some(Pretty::Alone),
        limits: &[OCCURRENCES],
    },
    Operation {
        id: OperationId::Constructs,
        name: "constructs",
        flag: Some(F::Constructs),
        summary: "List the object literals that construct the selected interface, class, or type alias in one explicit project, and the properties each does not certainly provide.",
        details: concat!(
            "--name takes --show's declaration containment selectors and FILE narrows the seeds; --at selects the declaration's name. By default the object literals of the searched files examined are those with a member naming a key the target declares, or with no named member ({}, or only spreads and computed names that are not literals). --all-literals examines every literal, as a target always does when an alternative has an index signature or is an empty object type or the global Object type: also those whose named members name only other keys, which construct the target, for example, under a type assertion such as {other: 1} as Options, beside a union member those keys choose, in a union context with a member accepting any key such as View | {}, or with an excess-property error. A site is one whose contextual type, as declared and as TypeScript discriminates it, is the target or one of its union alternatives, generic instantiations and aliases of intersections included; bases, intersection parts, and mapped forms such as Partial<T> are other types. Each site lists, per constructed alternative, its properties the literal lacks, quoted, marked ? when optional, absent or of unknown presence in the compiler's type of the literal (a spread of an optional property, an index signature, or any); this is not the runtime object. A literal spread into another literal belongs to that literal's site. --missing KEY keeps the sites that lack the string key KEY, compared exactly. Literals whose context cannot be established are located gaps, and the coverage is partial. A query runs under TSPARSE_MAX_MEMORY_MB (default ",
            default_memory_mb!(),
            " MiB) and TSPARSE_QUERY_TIMEOUT_MS (default ",
            default_timeout_ms!(),
            "), positive integers checked before any service starts; a service keeps the values it started with, and a limit failure exits the process that hit it."
        ),
        usage: &[
            "tsparse --constructs --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--missing KEY] [--all-literals] [--no-tests] [--file FILE] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::At,
            F::Name,
            F::Missing,
            F::AllLiterals,
            F::NoTests,
            F::File,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[
            Example {
                args: &[
                    "--constructs",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "RunView",
                ],
                summary: "Every literal that builds a RunView, and the properties each lacks.",
            },
            Example {
                args: &[
                    "--constructs",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "RunView",
                    "--missing",
                    "lane",
                    "--no-tests",
                ],
                summary: "Only the sites outside test paths that do not certainly provide lane.",
            },
            Example {
                args: &[
                    "--constructs",
                    "--project",
                    "tsconfig.json",
                    "--at",
                    "src/views.ts:12:18",
                    "--file",
                    "src/fixtures.ts",
                    "--json",
                ],
                summary: "The type declared at a position, its sites in one file, as a document.",
            },
            Example {
                args: &[
                    "--constructs",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "Options",
                    "--all-literals",
                    "--json",
                    "-o",
                    "sites.json",
                ],
                summary: "Every literal examined, also those naming only keys Options does not declare; the whole document goes to a file.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "A complete answer, including one with no site.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A partial answer, an ambiguous or missing selection, an unsupported target, a --missing KEY the target does not declare, an analysis failure, or a memory, time, depth, or output limit.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, a file that cannot be read or another I/O failure, an invalid position, a file outside the project, an internal failure, a service that cannot be started or reached, or a result that cannot be written.",
            },
        ],
        related: &[OperationId::Type, OperationId::References],
        doc: "docs/constructs.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[SITES, LACKS, GAPS, CANDIDATES],
    },
    Operation {
        id: OperationId::ReplaceBody,
        name: "replace-body",
        flag: Some(F::ReplaceBody),
        summary: "Preview replacing the complete body of the selected function, method, constructor, accessor, or arrow function.",
        details: concat!(
            "FRAGMENT is the complete new body: a block { ... }, or an expression for an arrow function; a variable or property whose initializer is directly a function or arrow function has that function's body replaced. Selection is --show's, in FILE or, with --project, in the project's searched files; an ambiguous or missing selection changes nothing. ",
            preview_details!()
        ),
        usage: &[
            "tsparse --replace-body (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: EDIT_FLAGS,
        examples: &[
            Example {
                args: &[
                    "--replace-body",
                    "--name",
                    "start",
                    "src/host.ts",
                    "--from",
                    "body.ts",
                ],
                summary: "body.ts holds the complete new body, { ... }; prints the diff.",
            },
            Example {
                args: &[
                    "--replace-body",
                    "--project",
                    "tsconfig.json",
                    "--name",
                    "RunHost.start",
                    "--from",
                    "body.ts",
                    "--plan",
                    "start.plan.json",
                ],
                summary: "Save the previewed edit as a plan for --apply.",
            },
        ],
        exits: EDIT_EXITS,
        related: &[
            OperationId::InsertAfter,
            OperationId::EditBatch,
            OperationId::Apply,
        ],
        doc: "docs/editing.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: EDIT_LIMITS,
    },
    Operation {
        id: OperationId::InsertAfter,
        name: "insert-after",
        flag: Some(F::InsertAfter),
        summary: "Preview inserting complete statements after the statement the selected declaration designates.",
        details: concat!(
            "FRAGMENT holds complete statements, inserted after the statement the selected declaration designates (a binding's variable statement, a specifier's import or export) in its file, block, module block, or case clause. Selection is --show's, in FILE or, with --project, in the project's searched files; an ambiguous or missing selection changes nothing. ",
            preview_details!()
        ),
        usage: &[
            "tsparse --insert-after (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: EDIT_FLAGS,
        examples: &[Example {
            args: &[
                "--insert-after",
                "--at",
                "src/host.ts:3:7",
                "--from",
                "statements.ts",
                "--json",
            ],
            summary: "Insert the statements in statements.ts after the statement declaring the name at 3:7, previewed as a document.",
        }],
        exits: EDIT_EXITS,
        related: &[
            OperationId::ReplaceBody,
            OperationId::EditBatch,
            OperationId::Apply,
        ],
        doc: "docs/editing.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: EDIT_LIMITS,
    },
    Operation {
        id: OperationId::EditBatch,
        name: "edit-batch",
        flag: Some(F::EditBatch),
        summary: "Preview several edits from a JSON file, all selected in the original files.",
        details: concat!(
            "EDITS is {\"edits\": [{\"operation\": \"replace_body\" or \"insert_after\", \"name\" or \"at\", \"file\", \"fragment\" or \"from\"}]}: at is LINE:COLUMN with file, or FILE:LINE:COLUMN, paths are relative to the working directory, a member that is null is absent, and unknown fields are errors. An invalid batch reports every issue by its JSON path, such as edits[1].extra, before anything is read or prepared. Every edit is selected in the original files and the edits are prepared together. ",
            preview_details!()
        ),
        usage: &[
            "tsparse --edit-batch EDITS [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]",
        ],
        flags: &[
            F::Project,
            F::Direct,
            F::Plan,
            F::Limit,
            F::Json,
            F::Pretty,
            F::Output,
        ],
        examples: &[Example {
            args: &[
                "--edit-batch",
                "edits.json",
                "--project",
                "tsconfig.json",
                "--plan",
                "edits.plan.json",
            ],
            summary: "Prepare every edit in edits.json together and save the plan.",
        }],
        exits: &[
            PREVIEW,
            UNPREPARED,
            ExitMeaning {
                code: 2,
                meaning: "A usage error, a batch file that cannot be read or is not an edit batch, a fragment or file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result or plan that cannot be written.",
            },
        ],
        related: &[
            OperationId::ReplaceBody,
            OperationId::InsertAfter,
            OperationId::Apply,
        ],
        doc: "docs/editing.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: EDIT_LIMITS,
    },
    Operation {
        id: OperationId::Apply,
        name: "apply",
        flag: Some(F::Apply),
        summary: "Write a saved plan's exact bytes, after checking that every file still has its original bytes and identity.",
        details: "Writes exactly the bytes --plan saved, restoring every file if a later one cannot be replaced, and reports the effect on the files. Keep other writers away from the files while applying. A plan records absolute paths, so it applies from any directory; it reads no TSPARSE_* variable and selects nothing again. --output is written only as a new file, never as a file the plan names.",
        usage: &["tsparse --apply PLAN [--json [--pretty]] [--output OUTPUT]"],
        flags: &[F::Json, F::Pretty, F::Output],
        examples: &[
            Example {
                args: &["--apply", "start.plan.json"],
                summary: "Apply a plan without selecting anything again.",
            },
            Example {
                args: &["--apply", "start.plan.json", "--json", "-o", "applied.json"],
                summary: "Report the outcome and the effect on the files as a document in a new file.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "Every file replaced (effect applied), or a plan that changes nothing (no_change).",
            },
            ExitMeaning {
                code: 1,
                meaning: "A plan another build wrote or whose edits no longer give its text, or a file changed since preparing (effect unchanged); preparing again resolves it.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error; a plan that cannot be read or is not a plan; a file that cannot be read, a staging failure, or a failed first replacement (unchanged); a later failure after which every file was restored (rolled_back); a file that could not be restored (recovery_required); or a result that cannot be written.",
            },
        ],
        related: &[
            OperationId::ReplaceBody,
            OperationId::InsertAfter,
            OperationId::EditBatch,
        ],
        doc: "docs/editing.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[],
    },
    Operation {
        id: OperationId::ReferencesStatus,
        name: "references-status",
        flag: Some(F::ReferencesStatus),
        summary: "Report the state of the project's service; it loads nothing, does not check freshness, and does not extend the service's idle time.",
        details: concat!(
            service_administration!(),
            " On macOS and Linux, project queries reuse one service per configuration spelling and build, started automatically, which reloads the project when its inputs change. Status prints running: with the service's pid, generation (the number of snapshots it has published), build, idle timeout, and the idle time it has left, then limits: with the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with; or no service: TSCONFIG. --json prints the document instead: running, pid, generation, build, limits (max_memory_mb, query_timeout_ms), idle_secs, idle_remaining_secs, and started_unix; without a service running is false, generation 0, and every other field null; --pretty, with or without --json, prints it indented. It reads only TSPARSE_SERVICE_DIR, the private runtime directory (default ",
            default_service_dir_prefix!(),
            "UID, owned by this user with mode 0700)."
        ),
        usage: &[
            "tsparse --references-status --project TSCONFIG [--json] [--pretty] [--output OUTPUT]",
        ],
        flags: STATUS_FLAGS,
        examples: &[
            Example {
                args: &["--references-status", "--project", "tsconfig.json"],
                summary: "Whether the service runs, and its pid, generation, build, idle time, and limits.",
            },
            Example {
                args: &[
                    "--references-status",
                    "--project",
                    "tsconfig.json",
                    "--pretty",
                ],
                summary: "The status document, indented.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "The status, also when no service is running.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, a runtime directory that is not private, a service that cannot be reached, or a result that cannot be written.",
            },
        ],
        related: &[OperationId::ReferencesStop, OperationId::ReferencesServe],
        doc: "docs/references.md",
        repeat: RepeatedValue::LastValue,
        pretty: Some(Pretty::Alone),
        limits: &[],
    },
    Operation {
        id: OperationId::ReferencesStop,
        name: "references-stop",
        flag: Some(F::ReferencesStop),
        summary: "Stop the project's service and release its cache.",
        details: concat!(
            service_administration!(),
            " It prints stopped: pid N when it stopped a service, or no service: TSCONFIG when none was running, on stdout. The next project query starts a new service. A service keeps the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with, so stop it after changing them; --doctor --project TSCONFIG reports a service whose limits differ from this environment's. It reads only TSPARSE_SERVICE_DIR."
        ),
        usage: &["tsparse --references-stop --project TSCONFIG"],
        flags: SERVICE_FLAGS,
        examples: &[Example {
            args: &["--references-stop", "--project", "tsconfig.json"],
            summary: "Release the service and its cache, printing stopped: pid N or no service: tsconfig.json.",
        }],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "The service stopped, or none was running.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, a runtime directory that is not private, or a service that cannot be reached or stopped.",
            },
        ],
        related: &[OperationId::ReferencesStatus, OperationId::ReferencesServe],
        doc: "docs/references.md",
        repeat: RepeatedValue::LastValue,
        pretty: None,
        limits: &[],
    },
    Operation {
        id: OperationId::ReferencesServe,
        name: "references-serve",
        flag: Some(F::ReferencesServe),
        summary: "Run the project's service in the foreground, for a supervisor, until it is idle.",
        details: concat!(
            service_administration!(),
            " A service exits after TSPARSE_SERVICE_IDLE_SECS idle seconds (default ",
            default_idle_secs!(),
            "). Its runtime directory is TSPARSE_SERVICE_DIR (default ",
            default_service_dir_prefix!(),
            "UID): owned by this user with mode 0700, and short enough for a socket path of about 100 bytes. Its queries run under TSPARSE_MAX_MEMORY_MB (default ",
            default_memory_mb!(),
            " MiB) and TSPARSE_QUERY_TIMEOUT_MS (default ",
            default_timeout_ms!(),
            "); a limit failure exits the service and the next request starts a replacement, and memory sampling can overshoot. TSPARSE_SERVICE_IDLE_SECS, TSPARSE_MAX_MEMORY_MB, and TSPARSE_QUERY_TIMEOUT_MS must each be a positive integer. It prints nothing but a failure."
        ),
        usage: &["tsparse --references-serve --project TSCONFIG"],
        flags: SERVICE_FLAGS,
        examples: &[Example {
            args: &["--references-serve", "--project", "tsconfig.json"],
            summary: "Serve in this terminal, printing any startup failure.",
        }],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "The service exited when idle or stopped, or another process already serves the project.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A query exceeded TSPARSE_MAX_MEMORY_MB or TSPARSE_QUERY_TIMEOUT_MS: the service answered it with that failure and exited, and the next request starts a replacement.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, an invalid TSPARSE_* variable, a runtime directory that cannot be created or is not private, or a failure before the service listens, such as a project that fails to load.",
            },
        ],
        related: &[OperationId::ReferencesStatus, OperationId::ReferencesStop],
        doc: "docs/references.md",
        repeat: RepeatedValue::LastValue,
        pretty: None,
        limits: &[],
    },
    Operation {
        id: OperationId::Doctor,
        name: "doctor",
        flag: Some(F::Doctor),
        summary: "Check what project queries depend on, one line per check; it loads nothing and starts nothing.",
        details: "Checks, in order: build (the version, build stamp, platform, and whether the project service, --apply, and memory monitoring work here); environment (each TSPARSE_* variable, its value or default, and each invalid value); runtime directory (its path, that it is a private directory of this user's with mode 0700 or does not exist yet, and the length of its socket paths beside the longest a socket address holds). With --project: configuration (the spelling as given, its absolute form, and that it can be read; another spelling of the same file has its own service); service (no service, with any startup diagnostic an earlier start left, which the next start removes; or its pid, generation, build, idle timeout and the time it has left, and when it started); and service limits (the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with beside this environment's, since a service keeps the environment it started with). Each check is one line, CHECK: detail; a finding adds its why and next lines, and a last line says ok: N checks pass or findings: and the checks with one. A check that cannot be performed is a finding. --json prints one document with an entry per check: name, status (pass or finding), detail, and a finding's why, next.argv, or remedy. Every transport failure of a project query ends with the doctor's command for its configuration.",
        usage: &["tsparse --doctor [--project TSCONFIG] [--json [--pretty]]"],
        flags: &[F::Project, F::Json, F::Pretty],
        examples: &[
            Example {
                args: &["--doctor"],
                summary: "The build, the environment, and the runtime directory.",
            },
            Example {
                args: &["--doctor", "--project", "tsconfig.json", "--json"],
                summary: "Every check for one project, its service included, as one document.",
            },
        ],
        exits: &[
            ExitMeaning {
                code: 0,
                meaning: "Every check passes.",
            },
            ExitMeaning {
                code: 1,
                meaning: "A check found something, or could not be performed.",
            },
            ExitMeaning {
                code: 2,
                meaning: "A usage error, or a report that cannot be written.",
            },
        ],
        related: &[OperationId::ReferencesStatus, OperationId::ReferencesStop],
        doc: "docs/references.md",
        repeat: RepeatedValue::Reject,
        pretty: Some(Pretty::WithJson),
        limits: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::{
        EXIT_CODES, FLAGS, FlagId, GLOBAL_FLAGS, Kind, OPERATIONS, OUTPUT_FLAGS, OperationId,
        Pretty, Repeat, accepting, flag, nearest, selected_by,
    };
    use crate::cli::command_line;
    use std::iter::once;
    use std::path::Path;

    #[test]
    fn each_entry_stands_at_its_id() {
        for (index, flag) in FLAGS.iter().enumerate() {
            assert_eq!(flag.id as usize, index, "{}", flag.name);
        }
        for (index, operation) in OPERATIONS.iter().enumerate() {
            assert_eq!(operation.id as usize, index, "{}", operation.name);
        }
    }

    /// One spelling names one flag, so no flag is a switch in one entry and
    /// takes a value in another.
    #[test]
    fn every_spelling_names_one_flag() {
        let spellings: Vec<&str> = FLAGS
            .iter()
            .flat_map(|flag| once(flag.name).chain(flag.aliases.iter().copied()))
            .collect();
        for spelling in &spellings {
            assert_eq!(
                spellings.iter().filter(|other| other == &spelling).count(),
                1,
                "{spelling}"
            );
            assert!(flag(spelling).is_some(), "{spelling}");
        }
        for entry in &FLAGS {
            assert!(entry.name.starts_with("--"), "{}", entry.name);
            for alias in entry.aliases {
                assert!(alias.len() == 2 && alias.starts_with('-'), "{alias}");
            }
            assert!(!entry.summary.is_empty(), "{}", entry.name);
            // A value's repeat is its operation's question; a switch's never is.
            assert_eq!(
                matches!(entry.kind, Kind::Value(_)),
                entry.repeat == Repeat::PerOperation,
                "{}",
                entry.name
            );
        }
        for operation in &OPERATIONS {
            assert_eq!(
                OPERATIONS
                    .iter()
                    .filter(|other| other.name == operation.name)
                    .count(),
                1,
                "{}",
                operation.name
            );
        }
    }

    #[test]
    fn every_flag_is_accepted_and_each_operation_flag_selects_its_operation() {
        for entry in &FLAGS {
            assert!(accepting(entry.id).next().is_some(), "{}", entry.name);
        }
        assert_eq!(
            OPERATIONS
                .iter()
                .filter(|operation| operation.flag.is_none())
                .map(|operation| operation.id)
                .collect::<Vec<_>>(),
            [OperationId::Parse]
        );
        for operation in &OPERATIONS {
            if let Some(own) = operation.flag {
                assert_eq!(selected_by(own).map(|o| o.id), Some(operation.id));
                assert!(!operation.flags.contains(&own), "{}", operation.name);
            }
            for &accepted in operation.flags {
                assert!(!GLOBAL_FLAGS.contains(&accepted), "{}", operation.name);
                // An accepted flag selects no other operation, so the
                // operations the flags name are the ones a request runs.
                assert!(
                    selected_by(accepted).is_none_or(|selected| selected.id == operation.id),
                    "{}: {accepted}",
                    operation.name
                );
            }
            // Selection is unambiguous: no other operation takes this one's
            // flag while this one takes the other's.
            for other in OPERATIONS.iter().filter(|other| other.id != operation.id) {
                if let (Some(own), Some(theirs)) = (operation.flag, other.flag) {
                    assert!(
                        !(operation.accepts(theirs) && other.accepts(own)),
                        "{} and {}",
                        operation.name,
                        other.name
                    );
                }
            }
        }
        for &global in GLOBAL_FLAGS {
            assert!(OPERATIONS.iter().all(|operation| operation.accepts(global)));
        }
    }

    /// Contract assertion 3, and what help prints of each operation.
    #[test]
    fn every_operation_has_a_summary_usage_examples_related_operations_and_a_document() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for operation in &OPERATIONS {
            let name = operation.name;
            assert!(!operation.summary.is_empty(), "{name}");
            assert!(!operation.summary.contains('\n'), "{name}");
            assert!(!operation.details.is_empty(), "{name}");
            // Codes in order, each once, each one the program uses; every
            // operation can succeed and can fail to be asked.
            let codes: Vec<u8> = operation.exits.iter().map(|exit| exit.code).collect();
            assert!(codes.is_sorted_by(|a, b| a < b), "{name}: {codes:?}");
            assert!(
                codes.first() == Some(&0) && codes.last() == Some(&2),
                "{name}"
            );
            for exit in operation.exits {
                assert!(!exit.meaning.is_empty(), "{name}: {}", exit.code);
            }
            assert!(!operation.usage.is_empty(), "{name}");
            for line in operation.usage {
                assert!(line.starts_with("tsparse "), "{name}: {line}");
                if let Some(own) = operation.flag {
                    assert!(line.contains(&format!("{own} ")), "{name}: {line}");
                }
            }
            assert!(!operation.examples.is_empty(), "{name}");
            for example in operation.examples {
                assert!(!example.summary.is_empty(), "{name}: {:?}", example.args);
            }
            assert!(!operation.related.is_empty(), "{name}");
            assert!(!operation.related.contains(&operation.id), "{name}");
            assert!(
                root.join(operation.doc).is_file(),
                "{name}: {}",
                operation.doc
            );
        }
    }

    #[test]
    fn usage_names_only_flags_its_operation_accepts() {
        for operation in &OPERATIONS {
            for line in operation.usage {
                for word in line.split(|c: char| c.is_whitespace() || "[]()|".contains(c)) {
                    if word.len() > 1 && word.starts_with('-') {
                        let named = flag(word).unwrap_or_else(|| panic!("{line}: {word}"));
                        assert!(operation.accepts(named.id), "{line}: {word}");
                    }
                }
            }
        }
    }

    #[test]
    fn exit_codes_are_each_code_once_in_order() {
        let codes: Vec<u8> = EXIT_CODES.iter().map(|exit| exit.code).collect();
        assert_eq!(codes, [0, 1, 2]);
    }

    /// The output flags the help index names once are shared by several
    /// operations, each of which lists them; only `--help` and `--version`
    /// are global (decisions.md, step 2).
    #[test]
    fn output_flags_are_shared_and_not_global() {
        for &output in OUTPUT_FLAGS {
            assert!(!GLOBAL_FLAGS.contains(&output), "{output}");
            assert!(selected_by(output).is_none(), "{output}");
            assert!(accepting(output).count() > 1, "{output}");
        }
    }

    /// An example prints as written: no argument needs a shell's quotes.
    #[test]
    fn every_example_reads_as_written() {
        for operation in &OPERATIONS {
            for example in operation.examples {
                for arg in example.args {
                    assert_eq!(command_line::render(&[arg]), *arg, "{:?}", example.args);
                }
            }
        }
    }

    /// The examples teach the shapes callers get wrong: `--at
    /// FILE:LINE:COLUMN` beside `--at LINE:COLUMN FILE`, a bare name beside
    /// a dotted one, a `--from` file holding a complete body, and `--pretty`
    /// alone wherever it stands alone on output.
    #[test]
    fn examples_teach_the_failure_prone_shapes() {
        let value = |args: &[&'static str], flag: FlagId| {
            args.iter()
                .position(|arg| *arg == flag.flag().name)
                .map(|index| args[index + 1])
        };
        for id in [OperationId::Show, OperationId::References] {
            let examples = id.operation().examples;
            let at: Vec<(&str, bool)> = examples
                .iter()
                .filter_map(|example| {
                    let at = value(example.args, FlagId::At)?;
                    let operand = example
                        .args
                        .iter()
                        .zip(example.args.iter().skip(1))
                        .any(|(before, arg)| *before == at && !arg.starts_with('-'));
                    Some((at, operand))
                })
                .collect();
            assert!(
                at.iter()
                    .any(|&(at, operand)| at.matches(':').count() == 2 && !operand),
                "{id:?} FILE:LINE:COLUMN"
            );
            assert!(
                at.iter()
                    .any(|&(at, operand)| at.matches(':').count() == 1 && operand),
                "{id:?} LINE:COLUMN FILE"
            );
            let names: Vec<&str> = examples
                .iter()
                .filter_map(|example| value(example.args, FlagId::Name))
                .collect();
            assert!(names.iter().any(|name| !name.contains('.')), "{id:?}");
            assert!(names.iter().any(|name| name.contains('.')), "{id:?}");
        }
        assert!(
            OperationId::ReplaceBody
                .operation()
                .examples
                .iter()
                .any(|example| value(example.args, FlagId::From).is_some()
                    && example.summary.contains("{ ... }"))
        );
        // An operation states where --pretty is valid exactly when it takes
        // it; stop and serve print no document for it to indent.
        for operation in &OPERATIONS {
            assert_eq!(
                operation.pretty.is_some(),
                operation.accepts(FlagId::Pretty),
                "{}",
                operation.name
            );
        }
        for operation in OPERATIONS
            .iter()
            .filter(|operation| operation.pretty == Some(Pretty::Alone))
        {
            assert!(
                operation.examples.iter().any(|example| {
                    example.args.contains(&FlagId::Pretty.flag().name)
                        && !example.args.contains(&FlagId::Json.flag().name)
                }),
                "{}",
                operation.name
            );
        }
    }

    /// An operation takes `--limit` exactly when it bounds a list, each list
    /// once, with a positive default.
    #[test]
    fn every_operation_that_bounds_a_list_takes_limit() {
        for operation in &OPERATIONS {
            assert_eq!(
                operation.accepts(FlagId::Limit),
                !operation.limits.is_empty(),
                "{}",
                operation.name
            );
            for (index, bounded) in operation.limits.iter().enumerate() {
                assert!(bounded.default > 0, "{}", operation.name);
                assert!(!bounded.summary.is_empty(), "{}", operation.name);
                assert!(
                    operation.limits[..index]
                        .iter()
                        .all(|earlier| earlier.list != bounded.list),
                    "{}",
                    operation.name
                );
            }
        }
    }

    /// The suggestion offers registered names, never an alias.
    #[test]
    fn the_nearest_flag_is_a_close_name() {
        for (spelling, name) in [
            ("--ouput", "--output"),
            ("--Apply", "--apply"),
            ("-help", "--help"),
        ] {
            assert_eq!(
                nearest(spelling).map(|flag| flag.name),
                Some(name),
                "{spelling}"
            );
        }
        for spelling in ["-O", "-H", "-v", "--frobnicate", "--"] {
            assert!(nearest(spelling).is_none(), "{spelling}");
        }
    }
}
