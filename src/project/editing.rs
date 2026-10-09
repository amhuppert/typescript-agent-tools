//! Edit selection on a project snapshot: each edit's file among the searched
//! files of this one generation, found as `show` finds it (a name without a
//! file through the files' indexes), and the text the project captured for
//! each file. Selecting parses nothing and reads no file; preparing the edits
//! against the files on disk is the caller's (`crate::editing::bind`), which
//! selects each site in its file exactly as `show` does.

use super::Project;
use super::navigation::Named;
use super::sources::FileId;
use crate::analysis::{AnalysisError, QueryError};
use crate::editing::{
    EditRequest, Outcome, Selected, SelectedEdit, SelectedFile, Selection, Unselected,
};
use crate::navigation::{Selector, ShowQuery};

impl Project {
    /// Select `request`'s edits in the searched files. A name without a file
    /// selects among every searched file through their indexes; a missing or
    /// ambiguous name is the outcome. Relative paths use the current working
    /// directory and are matched lexically; a position must lie in its file.
    pub fn edit(&self, request: EditRequest) -> Result<Selected, AnalysisError> {
        let mut files: Vec<(FileId, Vec<SelectedEdit>)> = Vec::new();
        for (index, edit) in request.edits.into_iter().enumerate() {
            let (id, selector) = match edit.target {
                ShowQuery::Name {
                    name,
                    file: Some(file),
                } => (self.searched_file(&file)?, Selector::Name(name)),
                ShowQuery::At { file, position } => {
                    let id = self.searched_file(&file)?;
                    self.sources
                        .get(id)
                        .text
                        .offset(position)
                        .map_err(|error| {
                            QueryError::invalid_position(error, &file, Some(position))
                        })?;
                    (id, Selector::At(position))
                }
                ShowQuery::Name { name, file: None } => match self.name_sites(&name, None)? {
                    Named::One(site) => (site.file, Selector::Name(name)),
                    Named::Other(outcome) => {
                        return Ok(Outcome::Unselected(Unselected {
                            edit: index,
                            file: None,
                            query: Selector::Name(name),
                            outcome,
                        }));
                    }
                },
            };
            let selected = SelectedEdit {
                index,
                operation: edit.operation,
                selector,
                fragment: edit.fragment,
            };
            match files.iter_mut().find(|(file, _)| *file == id) {
                Some((_, edits)) => edits.push(selected),
                None => files.push((id, vec![selected])),
            }
        }
        Ok(Outcome::Ready(Selection {
            project: self.config.clone(),
            files: files
                .into_iter()
                .map(|(id, edits)| {
                    let file = self.sources.get(id);
                    SelectedFile {
                        path: file.path.clone(),
                        display: file.display.clone(),
                        text: file.text.as_str().to_owned(),
                        edits,
                    }
                })
                .collect(),
        }))
    }
}
