//! A project's captured inputs and their project-local identities.

use crate::source::SourceText;
use std::path::PathBuf;
use std::sync::Arc;

/// A project-local file identity. It indexes `Sources` and never escapes the
/// project; only `Sources` creates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct FileId(u32);
// Hash/Eq observes one captured file index, never its path spelling.

impl FileId {
    pub(super) fn index(self) -> usize {
        self.0 as usize
    }
}

/// One captured program input.
#[derive(Debug)]
pub(super) struct SourceFile {
    /// Canonical absolute path, used for identity and query matching.
    pub(super) path: PathBuf,
    /// `/`-separated path relative to the configuration directory.
    pub(super) display: String,
    /// Shared with the file's declaration catalog, which describes it.
    pub(super) text: Arc<SourceText>,
}

#[derive(Debug, Default)]
pub(super) struct Sources {
    files: Vec<SourceFile>,
    reserved: u32,
}

impl Sources {
    /// The id of an input discovered before it is captured.
    pub(super) fn reserve(&mut self) -> FileId {
        let id = FileId(self.reserved);
        self.reserved = self
            .reserved
            .checked_add(1)
            .expect("file count fits in u32");
        id
    }

    /// Capture a reserved input. Inputs are captured in reservation order,
    /// so every table indexed by `FileId` lines up with `Sources`.
    pub(super) fn capture(&mut self, id: FileId, file: SourceFile) {
        assert_eq!(
            id.index(),
            self.files.len(),
            "inputs are captured in reservation order"
        );
        self.files.push(file);
    }

    /// Renumber the captured inputs to follow `order`, each of their ids at
    /// most once, so the program's file order is also its `FileId` order;
    /// an input `order` leaves out is no longer one. Returns each former
    /// id's new id, if any, indexed by the former id.
    pub(super) fn reorder(&mut self, order: &[FileId]) -> Vec<Option<FileId>> {
        assert_eq!(
            self.reserved as usize,
            self.files.len(),
            "inputs are all captured"
        );
        let mut files: Vec<Option<SourceFile>> = std::mem::take(&mut self.files)
            .into_iter()
            .map(Some)
            .collect();
        let mut renumbered = vec![None; files.len()];
        for (index, old) in order.iter().enumerate() {
            renumbered[old.index()] = Some(FileId(index as u32));
            let file = files[old.index()]
                .take()
                .expect("order names an input once");
            self.files.push(file);
        }
        self.reserved = order.len() as u32;
        renumbered
    }

    pub(super) fn get(&self, id: FileId) -> &SourceFile {
        &self.files[id.index()]
    }
    pub(super) fn ids(&self) -> impl Iterator<Item = FileId> + '_ {
        (0..self.files.len()).map(|index| FileId(index as u32))
    }
}
