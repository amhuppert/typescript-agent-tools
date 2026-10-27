//! Which compiler's rules read a project. This is product policy, not a
//! TypeScript function: the nearest `typescript/package.json` above the
//! configuration's canonical directory, found as Node's `require` finds a
//! package, names the installed compiler. Any TypeScript 5 release selects
//! the TypeScript 5.6 dialect, whose rules and tables are 5.6.3's (the only
//! release validated); no package, or any other major version, selects
//! TypeScript 7, whose rules every other part of this tool ports. Selection
//! reads metadata only, through `Disk`, so a session sees every change that
//! could select another compiler; it never runs one.
//!
//! A dialect composes the rule values that the owners of each difference
//! define and interpret themselves: configuration's `config::Rules`,
//! resolution's `resolve::Rules`, the standard libraries'
//! `libraries::Catalog`, the source-diagnostic `Policy`, the JavaScript
//! binder's `binding::javascript::Rules`, which decides the JavaScript
//! constructs loading refuses, and the compiler `Version` that package
//! ranges are evaluated against. This module only chooses them, and
//! describes the choice for messages and the doctor (`installed_note`,
//! `describe`); no other code asks which dialect was selected.

use super::super::binding;
use super::config;
use super::disk::Disk;
use super::libraries;
use super::resolve;
use super::version::Version;
use super::{io_error, manifest, paths};
use crate::analysis::{InputError, InputErrorKind};
use crate::source::SourceText;
use crate::source_diagnostics;
use std::path::{Path, PathBuf};

/// The compiler one generation reads its project for: the installed one,
/// if any, and the dialect it selects. Equal selections have the same
/// dialect and the same installed compiler: its manifest and version, and
/// the real paths of the manifest and of its package directory, so that
/// relinking either selects anew.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompilerSelection {
    installed: Option<Installed>,
    dialect: Dialect,
}

/// The compiler package selection found.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Installed {
    /// `<directory>/node_modules/typescript/package.json`, where the
    /// directory is the configuration's canonical directory or an ancestor.
    manifest: String,
    /// The manifest's real path, through any symbolic links on the way.
    real_manifest: String,
    /// The real path of the package directory, the manifest's own
    /// directory, which holds the compiler's `lib` directory wherever the
    /// manifest itself links.
    package_dir: String,
    /// The manifest's `version`, exactly as written.
    version: String,
    parsed: Version,
}

/// The rule values of one dialect, each defined by the owner that decides
/// it. Two dialects are equal when they are the same dialect for the same
/// compiler version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Dialect {
    /// As the doctor names it.
    name: &'static str,
    /// The major version of the compilers whose rules these are.
    major: u64,
    config: config::Rules,
    resolution: resolve::Rules,
    libraries: libraries::Catalog,
    source_diagnostics: source_diagnostics::Policy,
    javascript: binding::javascript::Rules,
    /// The version `typesVersions` ranges and `types@` conditions are
    /// evaluated against, as each compiler evaluates them against its own:
    /// the installed release's under the TypeScript 5.6 dialect, 7.0.2's
    /// under TypeScript 7.
    typescript: Version,
}

/// The major version that selects the TypeScript 5.6 dialect.
const TYPESCRIPT_5: u64 = 5;

impl Dialect {
    fn typescript_7() -> Self {
        Self {
            name: "TypeScript 7",
            major: 7,
            config: config::Rules::TYPESCRIPT_7,
            resolution: resolve::Rules::TYPESCRIPT_7,
            libraries: libraries::Catalog::TYPESCRIPT_7,
            source_diagnostics: source_diagnostics::Policy::TYPESCRIPT_7,
            javascript: binding::javascript::Rules::TYPESCRIPT_7,
            typescript: Version::reference(),
        }
    }

    fn typescript_5_6(installed: &Version) -> Self {
        Self {
            name: "TypeScript 5.6",
            major: TYPESCRIPT_5,
            config: config::Rules::TYPESCRIPT_5_6,
            resolution: resolve::Rules::TYPESCRIPT_5_6,
            libraries: libraries::Catalog::TYPESCRIPT_5_6,
            source_diagnostics: source_diagnostics::Policy::TYPESCRIPT_5_6,
            javascript: binding::javascript::Rules::TYPESCRIPT_5_6,
            typescript: installed.clone(),
        }
    }

    pub(super) fn config(&self) -> config::Rules {
        self.config
    }

    pub(super) fn resolution(&self) -> resolve::Rules {
        self.resolution
    }

    pub(super) fn libraries(&self) -> libraries::Catalog {
        self.libraries
    }

    pub(super) fn source_diagnostics(&self) -> source_diagnostics::Policy {
        self.source_diagnostics
    }

    pub(super) fn javascript(&self) -> binding::javascript::Rules {
        self.javascript
    }

    pub(super) fn typescript(&self) -> &Version {
        &self.typescript
    }
}

impl CompilerSelection {
    /// TypeScript 7 with no installed compiler: the selection when no
    /// TypeScript package is installed in or above the configuration's
    /// directory.
    pub(super) fn typescript_7() -> Self {
        Self {
            installed: None,
            dialect: Dialect::typescript_7(),
        }
    }

    pub(super) fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    /// The installed compiler's real package directory, if one is
    /// installed.
    pub(super) fn package_directory(&self) -> Option<&str> {
        self.installed
            .as_ref()
            .map(|installed| installed.package_dir.as_str())
    }

    /// The installed compiler's version, when it is not the major version
    /// the dialect is named for, for a message that reports a rule of the
    /// dialect: `installed compiler: TypeScript 6.0.3`.
    pub(super) fn installed_note(&self) -> Option<String> {
        let installed = self.installed.as_ref()?;
        (installed.parsed.major() != self.dialect.major)
            .then(|| format!("installed compiler: TypeScript {}", installed.version))
    }

    /// One line describing the selection, for the doctor: the installed
    /// compiler's manifest and version, the dialect, the semantic authority,
    /// and the standard library's source, the directory loading reads it
    /// from (`libraries::Catalog::directory`).
    pub(super) fn describe(&self, authority: &str, disk: &Disk) -> Result<String, InputError> {
        let installed = match &self.installed {
            None => "no TypeScript package is installed in or above the configuration's directory"
                .to_owned(),
            Some(installed) => {
                let mut text =
                    format!("TypeScript {} at {}", installed.version, installed.manifest);
                if installed.real_manifest != installed.manifest {
                    text.push_str(&format!(" ({})", installed.real_manifest));
                }
                // The dialect's rules are one release's, the only one
                // validated: exactly that version, so a build of it such as
                // `5.6.3+local`, which selects the same dialect, is not.
                let rules = self.dialect.config.version();
                if self.dialect.major == TYPESCRIPT_5 && installed.version != rules {
                    text.push_str(&format!(", validated for {rules} only"));
                }
                text
            }
        };
        let libraries = self.dialect.libraries;
        let directory = libraries.directory(self.package_directory(), disk)?;
        Ok(format!(
            "{installed}; read with the {} dialect; semantics: TypeScript {authority}; standard library: {}",
            self.dialect.name,
            libraries.source(&directory)
        ))
    }
}

/// The compiler installed for the configuration in `config_dir`, its
/// canonical directory, and the dialect it selects. Each candidate
/// manifest on the way is observed, absent ones included, so that one
/// appearing nearer selects it; the winner's real path and text are
/// observed too, so that a link retargeted or a version edited selects
/// again. A manifest that cannot be read is `Io`; one that is not a JSON
/// object with a version is `Unsupported`, never a reason to select
/// another compiler.
pub(super) fn select(config_dir: &str, disk: &Disk) -> Result<CompilerSelection, InputError> {
    for directory in paths::ancestors(config_dir) {
        // Node looks in no `node_modules/node_modules`.
        if paths::basename(directory) == "node_modules" {
            continue;
        }
        let manifest = paths::combine(directory, "node_modules/typescript/package.json");
        let found = disk
            .file_exists(&manifest)
            .map_err(|error| io_error(Path::new(&manifest), &error))?;
        if found {
            let installed = installed(manifest, disk)?;
            let dialect = if installed.parsed.major() == TYPESCRIPT_5 {
                Dialect::typescript_5_6(&installed.parsed)
            } else {
                Dialect::typescript_7()
            };
            return Ok(CompilerSelection {
                installed: Some(installed),
                dialect,
            });
        }
    }
    Ok(CompilerSelection::typescript_7())
}

/// The compiler `manifest` declares, read as Node reads a package
/// manifest: strict JSON after any byte order mark. Its package directory
/// is the real path of the manifest's directory, not the directory of the
/// manifest's real path: the compiler runs from that directory and reads
/// its libraries beside itself (`getDefaultLibLocation`), whatever file a
/// linked `package.json` names. Both real paths are observed, so that
/// relinking either selects again.
fn installed(manifest: String, disk: &Disk) -> Result<Installed, InputError> {
    let real_path = |path: &Path| {
        let real = disk
            .canonicalize(path)
            .map_err(|error| io_error(path, &error))?;
        paths::from_path(&real)
    };
    let path = Path::new(&manifest);
    let real_manifest = real_path(path)?;
    let package_dir = real_path(Path::new(paths::parent(&manifest)))?;
    let text = disk
        .read_to_string(path)
        .map_err(|error| io_error(path, &error))?;
    let source = SourceText::new(text.to_string());
    let failure = |offset: usize, message: String| InputError {
        kind: InputErrorKind::Unsupported,
        message: format!(
            "The TypeScript compiler manifest {message}, so no dialect can be selected"
        ),
        path: Some(PathBuf::from(&manifest)),
        position: Some(source.position(offset)),
    };
    let value = manifest::parse(&text).map_err(|error| {
        failure(
            error.offset,
            format!("is not valid JSON: {}", error.message),
        )
    })?;
    if value.entries().is_none() {
        return Err(failure(value.offset, "is not a JSON object".into()));
    }
    let Some(version) = value.get("version") else {
        return Err(failure(value.offset, "has no 'version'".into()));
    };
    let Some(text) = version.as_str() else {
        return Err(failure(
            version.offset,
            "has a 'version' that is not a string".into(),
        ));
    };
    let Some(parsed) = Version::parse(text) else {
        return Err(failure(
            version.offset,
            format!("has the 'version' {text:?}, which is not a version"),
        ));
    };
    Ok(Installed {
        manifest: manifest.clone(),
        real_manifest,
        package_dir,
        version: text.to_owned(),
        parsed,
    })
}
