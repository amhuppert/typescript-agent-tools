//! Captured file identity and the replacement lifecycle of one file: the
//! paths a request names, the regular file they resolve to, that file's
//! identity, and its exact bytes. Preparation binds edits to these
//! captures, and plans record them; application checks them again before
//! it replaces anything. No other code decides what makes a file the same
//! file.

use crate::analysis::{InputError, InputErrorKind};
use crate::input::{Case, InputKind};
use std::io;
use std::path::{Path, PathBuf};

/// A file as captured for editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Captured {
    /// The first path a request named the file by, made absolute without
    /// resolving links: analysis names the file by it.
    pub(super) path: PathBuf,
    /// The other paths the request named the same file by, each checked
    /// again as `path` is.
    pub(super) aliases: Vec<PathBuf>,
    /// The regular file the paths resolve to, canonical.
    pub(super) referent: PathBuf,
    pub(super) identity: Identity,
    image: Vec<u8>,
}

/// What identifies a file's contents as the same file, besides its bytes:
/// its device and inode, the permission mode replacement gives the new
/// file, and its link count, which is one: a file with other hard links is
/// not edited. Nothing else about a file is kept or compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Identity {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) mode: u32,
    pub(super) links: u64,
}

fn failure(kind: InputErrorKind, message: impl Into<String>, path: &Path) -> InputError {
    InputError {
        kind,
        message: message.into(),
        path: Some(path.to_path_buf()),
        position: None,
    }
}

/// A name `admit` refuses: for the file's kind, by the product policy that
/// only TypeScript files are edited, or for how it names the file.
#[derive(Debug)]
pub(super) struct Refused {
    pub(super) error: InputError,
    pub(super) file_kind: bool,
}

impl From<Refused> for InputError {
    fn from(refused: Refused) -> Self {
        refused.error
    }
}

/// The name an editable file is analyzed by: `path` exactly, which must be
/// absolute and name a TypeScript file under the case policy. Capturing a
/// file and replaying a recorded one both admit their names here.
pub(super) fn admit(path: &Path) -> Result<&str, Refused> {
    let named = |error| Refused {
        error,
        file_kind: false,
    };
    if !path.is_absolute() {
        return Err(named(failure(
            InputErrorKind::Unsupported,
            "an edited file is named by an absolute path",
            path,
        )));
    }
    let name = crate::input::exact_path(path).map_err(named)?;
    match InputKind::of(name, Case::detect()) {
        InputKind::TypeScript => Ok(name),
        kind => Err(Refused {
            error: failure(
                InputErrorKind::Unsupported,
                format!(
                    "{} files cannot be edited; editing needs a .ts, .tsx, .mts, or .cts file",
                    kind.label()
                ),
                path,
            ),
            file_kind: true,
        }),
    }
}

/// Capture the file `path` names for editing: a TypeScript file by its
/// name (`admit`), resolving to a regular file with one link and no flags,
/// whose contents are UTF-8 after an optional byte order mark
/// (`source::decode`).
pub(super) fn capture(path: &Path) -> Result<Captured, InputError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| failure(InputErrorKind::Io, error.to_string(), path))?;
    admit(&absolute)?;
    let io = |error: io::Error| failure(InputErrorKind::Io, error.to_string(), &absolute);
    let referent = std::fs::canonicalize(&absolute).map_err(io)?;
    let metadata = std::fs::metadata(&referent).map_err(io)?;
    if !metadata.is_file() {
        return Err(failure(
            InputErrorKind::Unsupported,
            "not a regular file",
            &absolute,
        ));
    }
    let identity = identity(&metadata, &absolute)?;
    let image = std::fs::read(&referent).map_err(io)?;
    if crate::source::decode(&image).is_err() {
        return Err(failure(
            InputErrorKind::Io,
            "stream did not contain valid UTF-8",
            &absolute,
        ));
    }
    Ok(Captured {
        path: absolute,
        aliases: Vec::new(),
        referent,
        identity,
        image,
    })
}

#[cfg(unix)]
fn identity(metadata: &std::fs::Metadata, path: &Path) -> Result<Identity, InputError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        return Err(failure(
            InputErrorKind::Unsupported,
            format!(
                "the file has {} hard links; replacing it would separate them",
                metadata.nlink()
            ),
            path,
        ));
    }
    let flags = flags(metadata);
    if flags != 0 {
        return Err(failure(
            InputErrorKind::Unsupported,
            format!(
                "the file has flags ({flags:#x}, such as hidden or uchg) that replacing it would drop"
            ),
            path,
        ));
    }
    Ok(Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        links: metadata.nlink(),
    })
}

/// A file's macOS flags (`chflags`), which a new file would not keep, so a
/// file with any is not edited; elsewhere not inspected.
#[cfg(target_os = "macos")]
fn flags(metadata: &std::fs::Metadata) -> u32 {
    std::os::macos::fs::MetadataExt::st_flags(metadata)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn flags(_: &std::fs::Metadata) -> u32 {
    0
}

impl Captured {
    /// A capture recorded earlier, such as in a plan, which `current`
    /// compares with the file on disk. Its names must be admitted as a
    /// capture's are.
    pub(super) fn recorded(
        path: PathBuf,
        aliases: Vec<PathBuf>,
        referent: PathBuf,
        identity: Identity,
        image: Vec<u8>,
    ) -> Result<Self, Refused> {
        for name in std::iter::once(&path).chain(&aliases) {
            admit(name)?;
        }
        if !referent.is_absolute() {
            return Err(Refused {
                error: failure(
                    InputErrorKind::Unsupported,
                    "a recorded file resolves to an absolute path",
                    &referent,
                ),
                file_kind: false,
            });
        }
        Ok(Self {
            path,
            aliases,
            referent,
            identity,
            image,
        })
    }

    /// The name analysis parses the file by.
    pub(super) fn name(&self) -> &str {
        self.path.to_str().expect("capture names files exactly")
    }

    /// The exact bytes captured.
    pub(super) fn image(&self) -> &[u8] {
        &self.image
    }

    /// The analyzed text of the captured bytes (`source::decode`).
    pub(super) fn text(&self) -> Result<&str, crate::analysis::AnalysisError> {
        crate::source::decode(&self.image).map_err(|_| {
            failure(
                InputErrorKind::Io,
                "stream did not contain valid UTF-8",
                &self.path,
            )
            .into()
        })
    }

    /// The bytes whose analyzed text is `text`, encoded as the captured
    /// bytes are (`source::encode_like`).
    pub(super) fn image_of(&self, text: &str) -> Vec<u8> {
        crate::source::encode_like(&self.image, text)
    }

    /// Whether the captured bytes begin with an encoding prefix
    /// (`source::prefix`), as a plan records.
    pub(super) fn byte_order_mark(&self) -> bool {
        !crate::source::prefix(&self.image).is_empty()
    }

    /// Take `other`, a capture of this file by another name, as one of its
    /// names, if it parses the same way, so that one captured source serves
    /// both and both names are checked again.
    pub(super) fn merge(&mut self, other: Captured) -> Result<(), InputError> {
        let options = |name: &str| {
            let options = crate::ParseOptions::for_file(name);
            (options.jsx, options.declaration_file())
        };
        if self.image != other.image {
            return Err(failure(
                InputErrorKind::Io,
                "the file changed while the request read it",
                &other.path,
            ));
        }
        if options(self.name()) != options(other.name()) {
            return Err(failure(
                InputErrorKind::Unsupported,
                format!(
                    "{} and {} name the same file but would parse differently; name it one way",
                    self.path.display(),
                    other.path.display()
                ),
                &other.path,
            ));
        }
        if other.path != self.path && !self.aliases.contains(&other.path) {
            self.aliases.push(other.path);
        }
        Ok(())
    }
}

/// Which of `names` names the file creating `output`, which must not exist,
/// would create. A name that resolves to a file now is not it. For the
/// others, rather than repeat the file system's rules for which spellings
/// name one file (links, case, Unicode forms), this asks it: `output` is
/// created empty, a name that now resolves to a file names it, since
/// nothing else writes meanwhile, and `output` is removed again. An
/// `output` that cannot be created creates no file, so no name is it;
/// writing the result reports why. Only absence (`input::is_absence`) means
/// a name names nothing; a failure to look one up, naming it, or to remove
/// `output`, saying it is left, is an error, never an answer.
pub(super) fn would_create<'a>(output: &Path, names: &[&'a Path]) -> io::Result<Option<&'a Path>> {
    let resolves = |name: &Path| match std::fs::metadata(name) {
        Ok(_) => Ok(true),
        Err(error) if crate::input::is_absence(&error) => Ok(false),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("cannot look up {}: {error}", name.display()),
        )),
    };
    let mut absent = Vec::new();
    for name in names {
        if !resolves(name)? {
            absent.push(*name);
        }
    }
    if absent.is_empty() {
        return Ok(None);
    }
    if std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .is_err()
    {
        return Ok(None);
    }
    let mut found = Ok(None);
    for name in absent {
        match resolves(name) {
            Ok(false) => {}
            Ok(true) => {
                found = Ok(Some(name));
                break;
            }
            Err(error) => {
                found = Err(error);
                break;
            }
        }
    }
    match (std::fs::remove_file(output), found) {
        (Ok(()), found) => found,
        (Err(error), found) => {
            let named = match found {
                Ok(Some(name)) => format!(" (it names {})", name.display()),
                _ => String::new(),
            };
            Err(io::Error::new(
                error.kind(),
                format!(
                    "was created to compare it with the files the command names{named}, and is left there, empty, since it cannot be removed: {error}"
                ),
            ))
        }
    }
}

/// Why a file on disk is no longer the captured file, or could not be
/// inspected.
#[derive(Debug)]
pub(super) enum Mismatch {
    /// It is absent, another file, or has another identity or other bytes.
    Stale(String),
    /// It could not be read or inspected.
    Io(io::Error),
}

/// Why a file with another identity than the captured file's, or with flags
/// it was captured without, is stale.
const CHANGED: &str = "it was replaced, or its permissions or flags changed";

impl Captured {
    /// Check that `path` still resolves to the captured file with the
    /// captured bytes and identity, linked `links` times (one, or two while a
    /// backup link exists), and still without flags. Timestamps are not
    /// identity; bytes are compared in full.
    fn check(&self, links: u64) -> Result<(), Mismatch> {
        let stale = |reason: &str| Err(Mismatch::Stale(reason.into()));
        for name in std::iter::once(&self.path).chain(&self.aliases) {
            let resolved = match std::fs::canonicalize(name) {
                Ok(resolved) => resolved,
                Err(error) if crate::input::is_absence(&error) => {
                    return Err(Mismatch::Stale(format!("{} was removed", name.display())));
                }
                Err(error) => return Err(Mismatch::Io(error)),
            };
            if resolved != self.referent {
                return Err(Mismatch::Stale(format!(
                    "{} now resolves to another file",
                    name.display()
                )));
            }
        }
        let metadata = match std::fs::metadata(&self.referent) {
            Ok(metadata) => metadata,
            Err(error) if crate::input::is_absence(&error) => return stale("it was removed"),
            Err(error) => return Err(Mismatch::Io(error)),
        };
        if !metadata.is_file() {
            return stale("it is no longer a regular file");
        }
        match current_identity(&metadata, links) {
            Ok(identity) if identity == self.identity => {}
            Ok(_) => return stale(CHANGED),
            Err(mismatch) => return Err(mismatch),
        }
        match std::fs::read(&self.referent) {
            Ok(bytes) if bytes == self.image => Ok(()),
            Ok(_) => stale("its contents changed"),
            Err(error) if crate::input::is_absence(&error) => stale("it was removed"),
            Err(error) => Err(Mismatch::Io(error)),
        }
    }

    /// Check that the file is still the captured file (`check`).
    pub(super) fn current(&self) -> Result<(), Mismatch> {
        self.check(1)
    }
}

#[cfg(unix)]
fn current_identity(metadata: &std::fs::Metadata, links: u64) -> Result<Identity, Mismatch> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != links {
        return Err(Mismatch::Stale(format!(
            "it now has {} hard links",
            metadata.nlink() + 1 - links
        )));
    }
    // A file is captured without flags (`identity`).
    if flags(metadata) != 0 {
        return Err(Mismatch::Stale(CHANGED.into()));
    }
    Ok(Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        // Its own link; a backup link is this process's.
        links: 1,
    })
}

/// The replacement of one captured file by its proposed bytes, from
/// staging through installation and restoration. Staging creates a
/// temporary file beside the original, writes the proposed bytes with the
/// original's permission mode, and links the original under a backup name;
/// installing renames the temporary file over the original; restoring
/// renames the backup back; releasing removes what remains. The state says
/// which of
/// those resources exist from the moment each is made, so every path
/// through it, including a staging failure, releases or reports exactly
/// them, and nothing it did not make.
#[derive(Debug)]
pub(super) struct Replacement<'c> {
    captured: &'c Captured,
    proposed: &'c [u8],
    state: State,
}

#[derive(Debug)]
enum State {
    /// The temporary file exists, its bytes or mode perhaps incomplete.
    Writing { temporary: PathBuf },
    /// The proposed bytes wait in `temporary`; the original is linked at
    /// `backup` too.
    Staged {
        temporary: PathBuf,
        backup: PathBuf,
        staged: (u64, u64),
    },
    /// The proposed file replaced the original, still linked at `backup`.
    Installed { backup: PathBuf, staged: (u64, u64) },
    /// Nothing remains to release.
    Released,
}

/// Why a file could not be restored: what is at its path, and where its
/// original remains.
#[derive(Debug)]
pub(super) struct Unrestored {
    pub(super) observed: String,
    pub(super) original: PathBuf,
}

/// A temporary or backup file that could not be removed.
#[derive(Debug)]
pub(super) struct Leftover {
    pub(super) path: PathBuf,
    pub(super) error: io::Error,
}

/// A staging failure, with what staging had made by then, for the caller
/// to release.
#[derive(Debug)]
pub(super) struct Unstaged<'c> {
    pub(super) mismatch: Mismatch,
    pub(super) partial: Option<Replacement<'c>>,
}

/// Names beside `referent` for this process's resources, never reused.
fn beside(referent: &Path, suffix: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = referent
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    referent.with_file_name(format!(
        ".{name}.tsparse-{}-{}.{suffix}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The most names tried for one resource before giving up.
const ATTEMPTS: usize = 64;

/// Make a resource beside `referent` with `make`, which must fail with
/// `AlreadyExists` when its name is taken: such a name is left alone, never
/// removed, and another is tried.
fn make_beside<T>(
    referent: &Path,
    suffix: &str,
    mut make: impl FnMut(&Path) -> io::Result<T>,
) -> io::Result<(T, PathBuf)> {
    let mut last = None;
    for _ in 0..ATTEMPTS {
        let path = beside(referent, suffix);
        match make(&path) {
            Ok(made) => return Ok((made, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last.expect("an attempt was made"))
}

impl<'c> Replacement<'c> {
    /// Stage the replacement of `captured` by `proposed`, after checking
    /// that it is still the captured file; `link` stands for the backup
    /// link's own failure. A failure returns what was made, to release.
    pub(super) fn stage(
        captured: &'c Captured,
        proposed: &'c [u8],
        link: &mut dyn FnMut() -> io::Result<()>,
    ) -> Result<Self, Unstaged<'c>> {
        let unstaged = |mismatch, partial| Unstaged { mismatch, partial };
        captured
            .current()
            .map_err(|mismatch| unstaged(mismatch, None))?;
        let (file, temporary) = make_beside(&captured.referent, "new", |path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
        })
        .map_err(|error| unstaged(Mismatch::Io(error), None))?;
        let mut replacement = Self {
            captured,
            proposed,
            state: State::Writing {
                temporary: temporary.clone(),
            },
        };
        let staged = match write_staged(captured, proposed, file) {
            Ok(staged) => staged,
            Err(error) => return Err(unstaged(Mismatch::Io(error), Some(replacement))),
        };
        let backup = link().and_then(|()| {
            make_beside(&captured.referent, "orig", |path| {
                std::fs::hard_link(&captured.referent, path)
            })
        });
        match backup {
            Ok(((), backup)) => {
                replacement.state = State::Staged {
                    temporary,
                    backup,
                    staged,
                };
                Ok(replacement)
            }
            Err(error) => Err(unstaged(
                Mismatch::Io(io::Error::new(
                    error.kind(),
                    format!("cannot link the original for restoration: {error}"),
                )),
                Some(replacement),
            )),
        }
    }

    /// Check, while staged, that the original is still the captured file.
    pub(super) fn recheck(&self) -> Result<(), Mismatch> {
        self.captured.check(2)
    }

    /// Replace the original with the staged file, in one rename, so no
    /// reader sees a partly written file.
    pub(super) fn install(&mut self) -> io::Result<()> {
        let State::Staged {
            temporary,
            backup,
            staged,
        } = &self.state
        else {
            unreachable!("only a staged replacement installs");
        };
        std::fs::rename(temporary, &self.captured.referent)?;
        self.state = State::Installed {
            backup: backup.clone(),
            staged: *staged,
        };
        Ok(())
    }

    /// Put the original back, unless the file at its path is no longer the
    /// one installed, which another writer's bytes would be: those are never
    /// overwritten.
    pub(super) fn restore(&mut self, fault: io::Result<()>) -> Result<(), Unrestored> {
        let State::Installed { backup, staged } = &self.state else {
            unreachable!("only an installed replacement restores");
        };
        let unrestored = |observed: String| Unrestored {
            observed,
            original: backup.clone(),
        };
        let referent = &self.captured.referent;
        let installed = std::fs::metadata(referent)
            .map(|metadata| file_id(&metadata) == *staged)
            .and_then(|same| Ok(same && std::fs::read(referent)? == self.proposed));
        let result = match installed {
            Ok(true) => fault
                .and_then(|()| std::fs::rename(backup, referent))
                .map_err(|error| {
                    unrestored(format!(
                        "the edited file, which could not be replaced by the original: {error}"
                    ))
                }),
            Ok(false) => Err(unrestored(
                "neither the edited file nor the original as installed: another writer replaced or changed it"
                    .into(),
            )),
            Err(error) => Err(unrestored(format!("a file that cannot be inspected: {error}"))),
        };
        // Restored, or the backup now belongs to the recovery report.
        self.state = State::Released;
        result
    }

    /// Release what remains: the temporary file of a staging one, both
    /// resources of a staged one, or the backup of an installed one. `fault`
    /// stands for the first removal's own failure.
    pub(super) fn release(mut self, fault: io::Result<()>) -> Vec<Leftover> {
        let paths = match std::mem::replace(&mut self.state, State::Released) {
            State::Writing { temporary } => vec![temporary],
            State::Staged {
                temporary, backup, ..
            } => vec![temporary, backup],
            State::Installed { backup, .. } => vec![backup],
            State::Released => vec![],
        };
        let mut fault = Some(fault);
        paths
            .into_iter()
            .filter_map(|path| {
                let result = fault
                    .take()
                    .unwrap_or(Ok(()))
                    .and_then(|()| std::fs::remove_file(&path));
                result.err().map(|error| Leftover { path, error })
            })
            .collect()
    }
}

/// A file's device and inode.
#[cfg(unix)]
fn file_id(metadata: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

/// Write `proposed` to `file`, a new temporary file, give it the captured
/// file's permission mode, verify that mode, and return the file's device
/// and inode once its bytes are on disk.
#[cfg(unix)]
fn write_staged(
    captured: &Captured,
    proposed: &[u8],
    mut file: std::fs::File,
) -> io::Result<(u64, u64)> {
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    file.write_all(proposed)?;
    let mode = captured.identity.mode;
    file.set_permissions(std::fs::Permissions::from_mode(mode & 0o7777))?;
    let metadata = file.metadata()?;
    if metadata.mode() != mode {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cannot give the new file the original's permissions",
        ));
    }
    file.sync_all()?;
    Ok(file_id(&metadata))
}
