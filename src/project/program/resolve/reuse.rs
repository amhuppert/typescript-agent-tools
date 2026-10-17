//! Reuse of resolution decisions across successful session generations.
//! Disk owns physical evidence; Host owns first answers; this module owns
//! which logical answers and side effects make a resolved path reusable.

use super::{Host, PackageJson, Resolved, ResolvedPath};
use crate::analysis::InputError;
use crate::input::Case;
use crate::project::binding::ResolutionMode;
use crate::project::program::{config::Options, disk};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(super) type Key = (String, String, Option<ResolutionMode>);

/// Only the last successful generation's reached lookups survive. Paths,
/// rather than FileIds, let loading assign the current generation's identity
/// and admission/provenance independently of a reused resolution.
#[derive(Default)]
pub(in crate::project::program) struct Reuse {
    context: Option<Arc<Context>>,
    modules: HashMap<Key, Arc<Saved<Resolved>>>,
    types: HashMap<Key, Arc<Saved<Option<ResolvedPath>>>>,
}

/// Compare the entire effective option set conservatively. This includes
/// output-to-input remapping settings and keeps future resolution options
/// from accidentally escaping invalidation.
#[derive(PartialEq, Eq)]
struct Context {
    options: Options,
    directory: String,
    file: String,
    case: Case,
}

pub(super) struct State<'a> {
    previous: Option<&'a Reuse>,
    pub(super) next: Reuse,
}

impl<'a> State<'a> {
    pub(super) fn new(
        previous: &'a Reuse,
        options: &Options,
        directory: &str,
        file: &str,
        case: Case,
    ) -> Self {
        let matching = previous.context.as_ref().filter(|context| {
            context.options == *options
                && context.directory == directory
                && context.file == file
                && context.case == case
        });
        let context = matching.cloned().unwrap_or_else(|| {
            Arc::new(Context {
                options: options.clone(),
                directory: directory.to_owned(),
                file: file.to_owned(),
                case,
            })
        });
        Self {
            previous: matching.map(|_| previous),
            next: Reuse {
                context: Some(context),
                ..Reuse::default()
            },
        }
    }

    pub(super) fn module(&self, key: &Key) -> Option<Arc<Saved<Resolved>>> {
        self.previous?.modules.get(key).cloned()
    }

    pub(super) fn types(&self, key: &Key) -> Option<Arc<Saved<Option<ResolvedPath>>>> {
        self.previous?.types.get(key).cloned()
    }

    pub(super) fn save_module(&mut self, key: Key, saved: Arc<Saved<Resolved>>) {
        self.next.modules.insert(key, saved);
    }

    pub(super) fn save_types(&mut self, key: Key, saved: Arc<Saved<Option<ResolvedPath>>>) {
        self.next.types.insert(key, saved);
    }
}

/// A successful primitive answer and its exact evidence, shared by every
/// resolution that consulted it. Failures remain ordinary current-load memo
/// entries and cannot participate in reusable decisions.
pub(super) struct Cached<T> {
    key: String,
    value: T,
    receipt: disk::Receipt,
}

/// Ordinary loads keep their existing inline Result, with no allocation or
/// receipt per probe. Session loads share only successful primitive answers.
pub(super) enum Memo<T> {
    Direct(Result<T, InputError>),
    Shared(Arc<Cached<T>>),
}

impl<T: Clone> Memo<T> {
    pub(super) fn read(
        key: &str,
        recording: bool,
        disk: &disk::Disk,
        read: impl FnOnce() -> Result<T, InputError>,
    ) -> Self {
        if !recording {
            return Self::Direct(read());
        }
        let (value, receipt) = disk.capture(read);
        match value {
            Ok(value) => Self::Shared(Arc::new(Cached {
                key: key.to_owned(),
                value,
                receipt,
            })),
            Err(error) => Self::Direct(Err(error)),
        }
    }

    pub(super) fn answer(&self) -> Result<T, InputError> {
        match self {
            Self::Direct(value) => value.clone(),
            Self::Shared(cached) => Ok(cached.value.clone()),
        }
    }

    fn agrees(&self, before: &Arc<Cached<T>>) -> bool {
        match self {
            // The same typed operation derives its value deterministically
            // from this exact evidence. Raw package-directory spelling is
            // included in the receipt; a different spelling may miss safely.
            Self::Shared(current) => {
                Arc::ptr_eq(current, before) || current.receipt == before.receipt
            }
            Self::Direct(_) => false,
        }
    }
}

type Package = Option<Arc<PackageJson>>;

pub(super) enum Access {
    File(Arc<Cached<bool>>),
    Directory(Arc<Cached<bool>>),
    Package(Arc<Cached<Package>>),
    Canonical(Arc<disk::Receipt>),
}

/// The first use of each logical memo entry, including cache hits. Physical
/// reads alone cannot reconstruct this: reading a package probes its entry
/// without populating Host.files, and those distinct first answers matter.
#[derive(Default)]
pub(super) struct Trace {
    accesses: Vec<Access>,
    seen: HashSet<(u8, usize)>,
    failed: bool,
}

impl Trace {
    pub(super) fn observe<T: Clone>(
        &mut self,
        memo: &Memo<T>,
        access: impl FnOnce(Arc<Cached<T>>) -> Access,
    ) {
        match memo {
            Memo::Shared(cached) => {
                let access = access(cached.clone());
                let identity = match &access {
                    Access::File(cached) => (0, Arc::as_ptr(cached) as usize),
                    Access::Directory(cached) => (1, Arc::as_ptr(cached) as usize),
                    Access::Package(cached) => (2, Arc::as_ptr(cached) as usize),
                    Access::Canonical(receipt) => (3, Arc::as_ptr(receipt) as usize),
                };
                if self.seen.insert(identity) {
                    self.accesses.push(access);
                }
            }
            Memo::Direct(_) => self.failed = true,
        }
    }

    pub(super) fn canonical(&mut self, receipt: disk::Receipt) {
        self.accesses.push(Access::Canonical(Arc::new(receipt)));
    }
}

pub(super) struct Saved<T> {
    pub(super) value: T,
    accesses: Box<[Access]>,
    pub(super) symlinks: Box<[(String, String)]>,
}

impl<T> Saved<T> {
    pub(super) fn new(value: T, trace: Trace, symlinks: &[(String, String)]) -> Option<Arc<Self>> {
        if trace.failed {
            return None;
        }
        Some(Arc::new(Self {
            value,
            accesses: trace.accesses.into_boxed_slice(),
            symlinks: symlinks.into(),
        }))
    }

    /// Validate the whole decision without changing observations, first
    /// answers, errors, or symlinks; only then commit its logical accesses.
    /// Existing current-load memo entries are authoritative and need no disk
    /// reread. Absent entries are checked speculatively at this demand point.
    pub(super) fn adopt(&self, host: &mut Host<'_>) -> bool {
        if !self.accesses.iter().all(|access| match access {
            Access::File(cached) => compatible(&host.files, cached, host.disk),
            Access::Directory(cached) => compatible(&host.directories, cached, host.disk),
            Access::Package(cached) => compatible(&host.packages, cached, host.disk),
            Access::Canonical(receipt) => host.disk.can_replay(receipt),
        }) {
            return false;
        }
        for access in &self.accesses {
            match access {
                Access::File(cached) => adopt(&mut host.files, cached, host.disk),
                Access::Directory(cached) => adopt(&mut host.directories, cached, host.disk),
                Access::Package(cached) => adopt(&mut host.packages, cached, host.disk),
                Access::Canonical(receipt) => host.disk.replay(receipt),
            }
        }
        true
    }
}

fn compatible<T: Clone>(
    current: &HashMap<String, Memo<T>>,
    before: &Arc<Cached<T>>,
    disk: &disk::Disk,
) -> bool {
    match current.get(&before.key) {
        Some(current) => current.agrees(before),
        None => disk.can_replay(&before.receipt),
    }
}

fn adopt<T: Clone>(
    current: &mut HashMap<String, Memo<T>>,
    before: &Arc<Cached<T>>,
    disk: &disk::Disk,
) {
    if let Some(current) = current.get_mut(&before.key) {
        if let Memo::Shared(cached) = current
            && Arc::ptr_eq(cached, before)
        {
            return;
        }
        debug_assert!(
            current.agrees(before),
            "adoption was validated before commit"
        );
        // Equal evidence proves the same typed answer. Share its old owner
        // after that one comparison, so later saved accesses can use ptr_eq.
        *current = Memo::Shared(before.clone());
        return;
    }
    disk.replay(&before.receipt);
    current.insert(before.key.clone(), Memo::Shared(before.clone()));
}
