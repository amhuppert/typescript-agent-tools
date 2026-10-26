//! Complete file-only navigation: read one named file, parse it with the
//! shared analysis policy on that policy's thread, project its catalog, and
//! answer; the test view answers from the syntax tree instead, after the
//! same read, classification, and parse (`parsed`). Nothing outside the file
//! is read; the file's project, if any, is irrelevant, and its failures
//! cannot affect the answer.

use super::catalog::{self, Catalog};
use super::test_outline;
use super::{Outline, Selector, Show, ShowOutcome, TestOutline};
use crate::Ast;
use crate::analysis::{AnalysisError, InputError, InputErrorKind};
use crate::input::{Case, InputKind};
use crate::source::{self, SourceText};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The complete declaration outline of a TypeScript file.
pub fn outline_file(path: &Path) -> Result<Outline, AnalysisError> {
    navigate(path, |file, catalog| {
        Ok(Outline {
            project: None,
            file: file.to_owned(),
            entries: catalog.declarations(),
        })
    })
}

/// The declaration a selector chooses in a TypeScript file, with its exact
/// source, or every candidate when it is ambiguous; a name that selects
/// nothing lists the file's specifiers that spell it.
pub fn show_file(path: &Path, selector: &Selector) -> Result<Show, AnalysisError> {
    navigate(path, |file, catalog| {
        let selection = catalog
            .select(selector)
            .map_err(|error| super::invalid_position(error, Path::new(file), selector))?;
        let outcome = match (catalog::outcome(file, catalog, &selection), selector) {
            (ShowOutcome::NotFound { .. }, Selector::Name(name)) => ShowOutcome::NotFound {
                specifiers: catalog.specifiers(name),
            },
            (outcome, _) => outcome,
        };
        Ok(Show {
            project: None,
            file: Some(file.to_owned()),
            query: selector.clone(),
            outcome,
        })
    })
}

/// The test calls of a TypeScript file, its test view, answered from its
/// syntax tree (`test_outline`).
pub fn test_outline_file(path: &Path) -> Result<TestOutline, AnalysisError> {
    parsed(path, |file, ast, source| {
        Ok(TestOutline {
            file: file.to_owned(),
            entries: test_outline::entries(&ast, source),
        })
    })
}

fn failure(kind: InputErrorKind, message: impl Into<String>, path: &Path) -> AnalysisError {
    AnalysisError::Input(InputError {
        kind,
        message: message.into(),
        path: Some(path.to_path_buf()),
        position: None,
    })
}

/// Read, classify, and parse `path`, then answer from its catalog, with the
/// file's absolute name, on the parsing thread. The syntax tree is dropped
/// before answering.
fn navigate<T: Send>(
    path: &Path,
    answer: impl FnOnce(&str, &Catalog) -> Result<T, AnalysisError> + Send,
) -> Result<T, AnalysisError> {
    parsed(path, |file, ast, source| {
        let catalog = Catalog::new(&ast, Arc::clone(source));
        drop(ast);
        answer(file, &catalog)
    })
}

/// Read, classify, and parse `path`, then answer from its syntax tree and
/// the source it was parsed from, with the file's absolute name, on the
/// parsing thread: the one input step of file navigation, whose failures
/// every file operation shares.
fn parsed<T: Send>(
    path: &Path,
    answer: impl for<'s> FnOnce(&str, Ast<'s>, &'s Arc<SourceText>) -> Result<T, AnalysisError> + Send,
) -> Result<T, AnalysisError> {
    let file: PathBuf = std::path::absolute(path)
        .map_err(|error| failure(InputErrorKind::Io, error.to_string(), path))?;
    let name = crate::input::exact_path(&file)?;
    match InputKind::of(name, Case::detect()) {
        InputKind::TypeScript => {}
        kind @ (InputKind::JavaScript | InputKind::Json) => {
            return Err(super::not_navigation_input(kind, &file).into());
        }
        InputKind::Other => {
            return Err(failure(
                InputErrorKind::Unsupported,
                "navigation input needs a .ts, .tsx, .mts, or .cts extension",
                &file,
            ));
        }
    }
    let work = |parser: &crate::analysis::Parser| {
        let text = source::read_text(&file)
            .map_err(|error| failure(InputErrorKind::Io, error.to_string(), &file))?;
        let source = Arc::new(SourceText::new(text));
        let ast = parser
            .parse(source.as_str(), name)
            .map_err(|error| AnalysisError::Input(InputError::unparsable(error, &source, &file)))?;
        answer(name, ast, &source)
    };
    crate::analysis::on_parsing_thread("tsparse-navigation", work).unwrap_or_else(|error| {
        Err(failure(
            InputErrorKind::Io,
            format!("cannot start the parsing thread: {error}"),
            &file,
        ))
    })
}
