//! What every complete analysis operation shares: project loading and
//! queries, file navigation, and any later reparse of captured project
//! text. They report failures as `AnalysisError`s, with positions in the
//! analyzed text (`crate::source::SourcePosition`), and parse with one
//! policy: the deep grammar budget, on a thread whose stack fits it, so
//! callers never choose a budget or a stack size, and display options such
//! as an outline depth cannot change which syntax an operation accepts.
//!
//! Parse mode keeps `ParseOptions::default()` on the caller's thread.

use crate::parser::json::{JsonError, JsonText};
use crate::source::{PositionError, SourcePosition, SourceText};
use crate::{Ast, Diagnostic, ParseOptions};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// The grammar budget of every analysis parse: generated declaration files
/// need several hundred grammar entries. `crate::DEEP_PARSE_STACK_BYTES`
/// fits it.
const BUDGET: usize = ParseOptions::DEEP_MAX_DEPTH;

/// The only means of parsing analyzed text. `on_parsing_thread` alone
/// creates one, on a thread whose stack fits `BUDGET`, and it parses only
/// with that budget, so no caller can pair the budget with another stack
/// or parse analyzed text with another budget. It is not `Sync`, so a
/// reference to it cannot leave its thread.
pub(crate) struct Parser {
    _thread: PhantomData<std::cell::Cell<()>>,
}

impl Parser {
    fn options(file_name: &str) -> ParseOptions {
        ParseOptions {
            max_depth: BUDGET,
            ..ParseOptions::for_file(file_name)
        }
    }

    pub(crate) fn jsdoc_tags(
        &self,
        text: &str,
        file_name: &str,
        kind: crate::syntax_kind::SyntaxKind,
        attachment: &crate::parser::jsdoc::Attachment,
        control: Option<crate::resources::Control>,
    ) -> Result<std::sync::Arc<crate::parser::jsdoc::Tags>, crate::parser::Failure> {
        crate::parser::jsdoc::parse_for_node(
            text,
            kind,
            attachment,
            Self::options(file_name),
            control,
        )
    }
    /// Captured source uses the same analysis grammar during loading and a
    /// request. A request shares its recursion guard with the parser.
    pub(crate) fn parse_captured<'s>(
        &self,
        text: &'s str,
        file_name: &str,
        control: Option<crate::resources::Control>,
    ) -> Result<Ast<'s>, crate::parser::Failure> {
        let parser = match control {
            Some(control) => {
                crate::parser::Parser::with_control(text, Self::options(file_name), control)?
            }
            None => crate::parser::Parser::new(text, Self::options(file_name)),
        };
        parser.parse_result()
    }
    /// Parse `text`, the analyzed text of the file named `file_name`, with
    /// the options TypeScript derives from that name
    /// (`ParseOptions::for_file`).
    /// Loading collects facts from the tree and drops it, so it is parsed as
    /// a transient tree.
    pub(crate) fn parse<'s>(&self, text: &'s str, file_name: &str) -> Result<Ast<'s>, Diagnostic> {
        crate::parser::Parser::new(text, Self::options(file_name))
            .transient()
            .parse()
    }

    /// Read `text` as TypeScript reads JSON, the one reader for
    /// configuration files and JSON modules (`parseJSONText`).
    pub(crate) fn parse_json(&self, text: &str) -> Result<JsonText, JsonError> {
        crate::parser::json::parse_json_text(text, BUDGET)
    }
}

/// Run `work` with the `Parser` on a dedicated thread with a stack of
/// `crate::DEEP_PARSE_STACK_BYTES`. Other recursion during project loading,
/// a package `imports` chain of at most `MAX_IMPORTS_CHAIN` lookups, needs
/// under 4 MiB and never runs during a parse. A panic resumes on the
/// caller's thread; failing to start the thread is an I/O error for the
/// caller's own error type.
pub(crate) fn on_parsing_thread<T: Send>(
    name: &str,
    work: impl FnOnce(&Parser) -> T + Send,
) -> std::io::Result<T> {
    std::thread::scope(|scope| {
        let thread = spawn_parsing_thread(scope, name, work)?;
        Ok(thread
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
    })
}

/// Start `work` with its own `Parser` on a new thread of `scope`, with the
/// stack `on_parsing_thread` gives, for work that parses alongside the
/// caller, such as capturing a project's inputs while it loads.
pub(crate) fn spawn_parsing_thread<'scope, T: Send + 'scope>(
    scope: &'scope std::thread::Scope<'scope, '_>,
    name: &str,
    work: impl FnOnce(&Parser) -> T + Send + 'scope,
) -> std::io::Result<std::thread::ScopedJoinHandle<'scope, T>> {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(crate::DEEP_PARSE_STACK_BYTES)
        .spawn_scoped(scope, || {
            work(&Parser {
                _thread: PhantomData,
            })
        })
}

/// Inputs that could not be analyzed: a project that could not be
/// constructed, in which case no partial snapshot exists; a file that
/// navigation could not read or parse; or a file a request names by a path
/// that cannot name it exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputError {
    pub kind: InputErrorKind,
    pub message: String,
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputErrorKind {
    /// A file or configuration could not be read, or the system refused a
    /// resource analysis needs, such as its thread or the working directory
    /// a relative path needs.
    Io,
    /// Invalid configuration content or inheritance.
    Config,
    /// An analyzed input has a syntax error.
    Syntax,
    /// An analyzed input nests deeper than the parser's grammar budget
    /// (`ParseOptions::max_depth`): a limit of this tool, not invalid syntax.
    ResourceLimit,
    /// A required input, option, or construct is outside the supported
    /// profile, including a navigated file that is not TypeScript and a
    /// path that is not Unicode, which names no file exactly.
    Unsupported,
    /// Declarations that cannot share a name.
    Binding,
    /// A module that no file or ambient declaration provides.
    UnresolvedImport,
}

impl InputError {
    /// The failure of parsing the analyzed text `source` of the file at
    /// `path`: the one decision of which kind a parse failure is, for every
    /// operation that parses analyzed text. Exceeding the grammar budget is
    /// a `ResourceLimit`, kept apart from invalid syntax, which is `Syntax`.
    pub(crate) fn unparsable(error: Diagnostic, source: &SourceText, path: &Path) -> Self {
        Self {
            kind: if crate::parser::exhaustion(&error) {
                InputErrorKind::ResourceLimit
            } else {
                InputErrorKind::Syntax
            },
            position: Some(source.position(error.span.start)),
            message: error.message,
            path: Some(path.to_path_buf()),
        }
    }
}

/// A request that does not fit the analyzed inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryError {
    pub kind: QueryErrorKind,
    pub message: String,
    /// The file as the query spelled it for `OutsideProject` and
    /// `InvalidPosition`, which concern the query itself; otherwise the
    /// selected token's file as a canonical absolute path, whichever entry
    /// point selected it.
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
}

/// Serialized as the variant name, the stable spelling tools compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryErrorKind {
    /// A library/service request contains invalid operation options. CLI
    /// parsing rejects the same policy before loading a project.
    InvalidRequest,
    /// The query file is not one of the project's searched files, including
    /// an empty path, which names no file.
    OutsideProject,
    /// A line/column outside the file or inside a surrogate pair.
    InvalidPosition,
    /// The selected identifier is a member, label, or other unsupported target.
    UnsupportedTarget,
    /// The selected identifier, or a name candidate, has no resolvable target.
    UnresolvedTarget,
}

impl QueryError {
    /// A position a request names in `file` that addresses no position in
    /// it, with the file as the request spelled it: every operation's
    /// `InvalidPosition`.
    pub(crate) fn invalid_position(
        error: PositionError,
        file: &Path,
        position: Option<SourcePosition>,
    ) -> Self {
        Self {
            kind: QueryErrorKind::InvalidPosition,
            message: error.to_string(),
            path: Some(file.to_path_buf()),
            position,
        }
    }
}

/// Why an analysis request failed: its inputs could not be analyzed, or the
/// request does not fit them. Every operation, whether on a project, a
/// session, or one navigated file, reports failures as these kinds, each
/// with its own location; they stay distinct until a caller decides the
/// response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisError {
    Input(InputError),
    Query(QueryError),
    /// Boxed: semantic failures carry provenance and causes, and every
    /// analysis result would otherwise pay for their size.
    Semantic(Box<SemanticError>),
}

/// A demanded semantic operation failed. These failures never become loader
/// tolerance, ordinary relation false, or display omissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticError {
    pub kind: SemanticErrorKind,
    pub reason: SemanticReason,
    pub message: String,
    pub path: Option<PathBuf>,
    pub position: Option<SourcePosition>,
    /// Request-internal provenance, captured by the semantic operation that
    /// knows the complete source range. This survives AnalysisError/DemandError
    /// conversions but never extends the public error wire schema. It does not
    /// make an invalid/resource failure eligible for a Q4 unsupported leaf.
    #[serde(skip)]
    pub(crate) exact_location: Option<crate::references::Location>,
    /// Native startup recovery has no source until an actual operation reads
    /// it. This typed marker survives internal error conversions, never wire output.
    #[serde(skip)]
    pub(crate) unbound_recovery: Option<UnboundSemanticRecovery>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<SemanticCause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_causes: Option<usize>,
    /// Classification of all acquired causes, including the omitted suffix.
    /// Internal error conversions retain this; the wire format does not.
    #[serde(skip)]
    pub(crate) cause_kinds: CauseKinds,
}

/// Source-free native installed recoveries, distinct from unlocated global failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnboundSemanticRecovery {
    UnknownSymbolDeclaredType,
    UnknownSignatureReturn,
    NonValueSymbol,
    InstalledGlobalType,
    RelationDepth,
    RelationComplexity,
}

impl SemanticError {
    pub fn resource_limit(reason: SemanticResourceReason, message: impl Into<String>) -> Self {
        let kind = match reason {
            SemanticResourceReason::SemanticMemoryLimit => SemanticErrorKind::SemanticMemoryLimit,
            SemanticResourceReason::SemanticDepthLimit => SemanticErrorKind::SemanticDepthLimit,
            SemanticResourceReason::SemanticDeadline => SemanticErrorKind::SemanticDeadline,
            SemanticResourceReason::OutputLimit => SemanticErrorKind::OutputLimit,
            SemanticResourceReason::InternalFailure => SemanticErrorKind::Internal,
        };
        Self {
            kind,
            reason: reason.into(),
            message: message.into(),
            path: None,
            position: None,
            exact_location: None,
            unbound_recovery: None,
            causes: Vec::new(),
            omitted_causes: None,
            cause_kinds: Default::default(),
        }
    }

    pub(crate) const MAX_CAUSES: usize = 16;

    /// Project the actual acquired failure as a causal dependency. A location
    /// exists only when its semantic producer retained the complete range.
    pub(crate) fn as_cause(&self) -> SemanticCause {
        SemanticCause {
            reason: self.reason,
            message: copy_error_text(&self.message),
            location: self
                .exact_location
                .as_ref()
                .map(crate::references::metadata::copy_location),
            kind: Some(self.kind),
            unbound_recovery: self.unbound_recovery,
        }
    }
    /// Copy established error metadata, including private provenance. This never
    /// changes admission, binds a missing source, or truncates/reorders causes.
    /// Append a complete acquired dependency. Classification is merged before
    /// truncation, so an omitted Invalid cause cannot become an unsupported leaf.
    pub(crate) fn append_dependency(&mut self, dependency: &Self) -> Result<(), AnalysisError> {
        self.record_unclassified_omissions();
        self.cause_kinds.0 |= dependency.cause_kinds.0;
        if dependency.omitted_causes.unwrap_or(0) != 0 && dependency.cause_kinds.0 == 0 {
            self.cause_kinds.include(None);
        }
        self.append_causes(
            std::iter::once(dependency.as_cause()).chain(dependency.causes.iter().cloned()),
            dependency.omitted_causes.unwrap_or(0),
        )
    }

    fn record_unclassified_omissions(&mut self) {
        if self.omitted_causes.unwrap_or(0) != 0 && self.cause_kinds.0 == 0 {
            self.cause_kinds.include(None);
        }
    }

    pub(crate) fn permits_unsupported_leaf(&self) -> bool {
        self.kind == SemanticErrorKind::UnsupportedSemantic
            && self
                .cause_kinds
                .only(SemanticErrorKind::UnsupportedSemantic)
            && (self.omitted_causes.unwrap_or(0) == 0 || self.cause_kinds.0 != 0)
            && self
                .causes
                .iter()
                .all(|cause| cause.kind == Some(SemanticErrorKind::UnsupportedSemantic))
    }

    /// Bind only native source-free recovery on this observation copy. An
    /// ordinary located primary must not suppress a later unbound child.
    /// Bind only typed source-free native recovery to its actual consumer. This
    /// product provenance operation preserves already located/global evidence;
    /// cached unknownSignature/global values retain their original markers.
    pub(crate) fn observe_recovery<'p>(
        &mut self,
        location: &crate::references::Location,
        primary_path: impl FnOnce() -> &'p Path,
    ) {
        let copy_location = || crate::references::metadata::copy_location(location);
        if self.unbound_recovery.take().is_some() {
            self.path = Some((primary_path()).to_path_buf());
            self.position = Some(location.start);

            self.exact_location = Some(copy_location());
        }
        for cause in &mut self.causes {
            if cause.unbound_recovery.take().is_some() {
                cause.location = Some(copy_location());
            }
        }
    }

    pub(crate) fn append_cause(&mut self, cause: SemanticCause) -> Result<(), AnalysisError> {
        self.append_causes(std::iter::once(cause), 0)
    }

    /// Reapply the same bound when a public error's cause prefix is consumed.
    pub(crate) fn bound_causes(&mut self) -> Result<(), AnalysisError> {
        self.append_causes(std::iter::empty(), 0)
    }

    /// Preserve the first bounded cause prefix and count every omitted entry.
    /// Aggregation never replaces the primary failure or reorders its causes.
    fn append_causes(
        &mut self,
        causes: impl IntoIterator<Item = SemanticCause>,
        already_omitted: usize,
    ) -> Result<(), AnalysisError> {
        let overflow = || {
            AnalysisError::from(SemanticError {
                kind: SemanticErrorKind::OutputLimit,
                reason: SemanticResourceReason::OutputLimit.into(),
                message: "semantic cause count exceeds representable output metadata".to_owned(),
                path: None,
                position: None,
                exact_location: None,
                unbound_recovery: None,
                causes: Vec::new(),
                omitted_causes: None,
                cause_kinds: Default::default(),
            })
        };
        // Existing or deserialized rows may not have passed this owner yet.
        // Classify an unknown omitted suffix before new known rows can give
        // the aggregate a nonempty (but incomplete) classification.
        self.record_unclassified_omissions();
        for cause in &self.causes {
            self.cause_kinds.include(cause.kind);
        }
        let excess = self.causes.len().saturating_sub(Self::MAX_CAUSES);
        let mut omitted = self
            .omitted_causes
            .unwrap_or(0)
            .checked_add(already_omitted)
            .and_then(|count| count.checked_add(excess))
            .ok_or_else(overflow)?;

        self.causes.truncate(Self::MAX_CAUSES);
        for cause in causes {
            self.cause_kinds.include(cause.kind);
            if self.causes.len() < Self::MAX_CAUSES {
                self.causes.push(cause);
            } else {
                omitted = omitted.checked_add(1).ok_or_else(overflow)?;
            }
        }
        self.omitted_causes = (omitted != 0).then_some(omitted);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticErrorKind {
    UnsupportedSemantic,
    InvalidSemantic,
    SemanticMemoryLimit,
    SemanticDepthLimit,
    SemanticDeadline,
    OutputLimit,
    Internal,
}

/// Unsupported and invalid dependencies share one vocabulary across type
/// errors, partial details and reference coverage. Operations restrict their
/// admissible categories; resource failures never enter a coverage ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFailureReason {
    FlowRequired,
    JsxTypingRequired,
    GeneratorInferenceRequired,
    DecoratorTypingRequired,
    JavascriptSemanticsRequired,
    ExpandoSemanticsRequired,
    UnsupportedSemanticForm,
    UnrepresentableIdentity,
    UnresolvedSymbol,
    ErrorType,
    OverloadRecovery,
    CircularType,
    InstantiationLimit,
    ConditionalTailLimit,
    UnionComplexityLimit,
    TupleComplexityLimit,
    SubtypeComplexityLimit,
    RelationDepthLimit,
    RelationComplexityLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticResourceReason {
    SemanticMemoryLimit,
    SemanticDepthLimit,
    SemanticDeadline,
    OutputLimit,
    InternalFailure,
}

/// Flat wire spelling while preserving the distinction between failed
/// language dependencies and product resources/internal failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SemanticReason {
    Failure(SemanticFailureReason),
    Resource(SemanticResourceReason),
}

impl From<SemanticFailureReason> for SemanticReason {
    fn from(reason: SemanticFailureReason) -> Self {
        Self::Failure(reason)
    }
}

impl From<SemanticResourceReason> for SemanticReason {
    fn from(reason: SemanticResourceReason) -> Self {
        Self::Resource(reason)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCause {
    pub reason: SemanticReason,
    /// Global compiler dependencies can fail without a declaration site.
    /// Preserve that absence instead of assigning the consuming expression's
    /// position to the original cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<crate::references::Location>,
    pub message: String,
    /// Private acquisition provenance survives internal AnalysisError
    /// round-trips but never changes the public causal wire schema.
    #[serde(skip)]
    pub(crate) kind: Option<SemanticErrorKind>,
    #[serde(skip)]
    pub(crate) unbound_recovery: Option<UnboundSemanticRecovery>,
}

impl SemanticCause {}

/// Bounded aggregate of acquired cause classifications. The high bit records
/// a deserialized cause whose original private classification is unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CauseKinds(u16);
impl CauseKinds {
    fn include(&mut self, kind: Option<SemanticErrorKind>) {
        self.0 |= kind.map_or(1 << 15, |kind| 1 << kind as u16);
    }
    fn only(self, kind: SemanticErrorKind) -> bool {
        self.0 & !(1 << kind as u16) == 0
    }
}

impl From<InputError> for AnalysisError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

impl From<QueryError> for AnalysisError {
    fn from(error: QueryError) -> Self {
        Self::Query(error)
    }
}

impl From<SemanticError> for AnalysisError {
    fn from(error: SemanticError) -> Self {
        Self::Semantic(Box::new(error))
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_location(f, self.path.as_deref(), self.position)?;
        f.write_str(&self.message)
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_location(f, self.path.as_deref(), self.position)?;
        f.write_str(&self.message)
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
            Self::Semantic(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_location(f, self.path.as_deref(), self.position)?;
        f.write_str(&self.message)
    }
}

fn write_location(
    f: &mut fmt::Formatter<'_>,
    path: Option<&Path>,
    position: Option<SourcePosition>,
) -> fmt::Result {
    match (path, position) {
        (Some(path), Some(position)) => write!(
            f,
            "{}:{}:{}: ",
            path.display(),
            position.line,
            position.column
        ),
        (Some(path), None) => write!(f, "{}: ", path.display()),
        _ => Ok(()),
    }
}

impl std::error::Error for InputError {}
impl std::error::Error for QueryError {}
impl std::error::Error for AnalysisError {}
impl std::error::Error for SemanticError {}

fn copy_error_text(text: &str) -> String {
    crate::references::metadata::copy_text(text)
}
