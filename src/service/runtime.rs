//! The service's runtime directory: which one a client uses, and why. The
//! choice reads only the environment's strings
//! (`Environment::runtime_variables`) and the platform's socket address,
//! never the file system, so every client in one environment chooses the
//! same directory; a directory that turns out to be unusable is reported
//! with the rule that chose it, never replaced by another. A client passes
//! the absolute directory it chose to the service it starts, which takes it
//! as `TSPARSE_SERVICE_DIR`.

use super::SocketBudget;
use crate::cli::Failure;
use crate::cli::failure::Code;
use crate::process_limits::{Environment, RuntimeVariables};
use std::fmt;
use std::path::{Path, PathBuf};

/// The default runtime directory, before this user's id, as the one literal
/// both `choose` and help (`cli::registry`) take it from.
macro_rules! default_service_dir_prefix {
    () => {
        "/tmp/tsparse-"
    };
}
pub(crate) use default_service_dir_prefix;

/// A runtime directory, and the rule that chose it.
pub(crate) struct RuntimeDirectory {
    pub(crate) path: PathBuf,
    pub(crate) rule: Rule,
}

/// The rule that chose a runtime directory.
pub(crate) enum Rule {
    /// `TSPARSE_SERVICE_DIR` names it.
    Variable,
    /// `TMPDIR/tsparse-UID`, as `TSPARSE_SERVICE_DIR` is not set.
    Tmpdir,
    /// `/tmp/tsparse-UID`, as neither variable chose one, and why `TMPDIR`
    /// did not.
    Default(Ignored),
}

/// Why `TMPDIR` chose no runtime directory.
pub(crate) enum Ignored {
    Unset,
    Empty,
    /// It is relative, so it would name another directory in each working
    /// directory, and clients of one environment would disagree.
    Relative,
    /// The socket paths in `TMPDIR/tsparse-UID` would take `length` bytes,
    /// more than the `limit` a socket address holds.
    Long {
        length: usize,
        limit: usize,
    },
}

/// The runtime directory `variables` choose for the user `user`: the one
/// `TSPARSE_SERVICE_DIR` names, any path, as it is given; otherwise
/// `TMPDIR/tsparse-UID` when `TMPDIR` is a non-empty absolute path and a
/// socket address holds the socket paths in that directory; otherwise
/// `/tmp/tsparse-UID`. `TMPDIR` is taken by its components, so a trailing
/// or repeated slash names the same directory, and so the same service.
pub(crate) fn choose(variables: RuntimeVariables, user: u32) -> RuntimeDirectory {
    if let Some(directory) = variables.service_dir {
        return RuntimeDirectory {
            path: directory.into(),
            rule: Rule::Variable,
        };
    }
    let ignored = match variables.tmpdir {
        None => Ignored::Unset,
        Some(tmpdir) if tmpdir.is_empty() => Ignored::Empty,
        Some(tmpdir) if !Path::new(&tmpdir).is_absolute() => Ignored::Relative,
        Some(tmpdir) => {
            let path = Path::new(&tmpdir)
                .components()
                .collect::<PathBuf>()
                .join(format!("tsparse-{user}"));
            let budget = SocketBudget::of(&path);
            if budget.fits() {
                return RuntimeDirectory {
                    path,
                    rule: Rule::Tmpdir,
                };
            }
            Ignored::Long {
                length: budget.length,
                limit: budget.limit,
            }
        }
    };
    RuntimeDirectory {
        path: PathBuf::from(format!(concat!(default_service_dir_prefix!(), "{}"), user)),
        rule: Rule::Default(ignored),
    }
}

/// The runtime directory this process's environment chooses for this user
/// (`choose`), made absolute: a relative `TSPARSE_SERVICE_DIR` names a
/// directory in the working directory.
pub(super) fn chosen() -> Result<RuntimeDirectory, Failure> {
    let RuntimeDirectory { path, rule } =
        choose(Environment::runtime_variables(), super::user_id());
    let path = std::path::absolute(&path).map_err(|error| {
        Failure::new(
            Code::Directory,
            format!(
                "Project service directory {} cannot be made absolute: {error}",
                path.display()
            ),
        )
    })?;
    Ok(RuntimeDirectory { path, rule })
}

impl Rule {
    /// The remedy for a runtime directory this rule chose that cannot be
    /// created, inspected, or used for a lock: another directory, named
    /// by the variables that choose one, and why `TMPDIR` chose none.
    pub(super) fn remedy(&self) -> String {
        match self {
            Rule::Variable => "set TSPARSE_SERVICE_DIR to a path in a directory this user can write, or unset it and set TMPDIR to such a directory".into(),
            Rule::Tmpdir => "set TMPDIR to a directory this user can write, or TSPARSE_SERVICE_DIR to a path in one".into(),
            Rule::Default(ignored) => format!("set TMPDIR to an absolute directory this user can write, or TSPARSE_SERVICE_DIR to a path in one; TMPDIR {ignored}"),
        }
    }
}

/// The rule, as the doctor names it beside the directory.
impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Rule::Variable => f.write_str("TSPARSE_SERVICE_DIR"),
            Rule::Tmpdir => f.write_str("TMPDIR/tsparse-UID, as TSPARSE_SERVICE_DIR is not set"),
            Rule::Default(ignored) => write!(
                f,
                concat!(
                    default_service_dir_prefix!(),
                    "UID, as TSPARSE_SERVICE_DIR is not set and TMPDIR {}"
                ),
                ignored
            ),
        }
    }
}

/// Why `TMPDIR` chose no directory, after its name.
impl fmt::Display for Ignored {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ignored::Unset => f.write_str("is not set"),
            Ignored::Empty => f.write_str("is empty"),
            Ignored::Relative => f.write_str("is not an absolute path"),
            Ignored::Long { length, limit } => write!(
                f,
                "makes socket paths of {length} bytes, more than the {limit} a socket address holds"
            ),
        }
    }
}
