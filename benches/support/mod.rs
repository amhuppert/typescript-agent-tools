use std::{fs, path::Path};
pub struct Source {
    pub name: String,
    pub text: String,
}
pub fn sources(paths: &[String]) -> Vec<Source> {
    let mut files = Vec::new();
    for path in paths {
        collect(Path::new(path), &mut files);
    }
    files.sort();
    files
        .into_iter()
        .map(|file| Source {
            name: file.to_string_lossy().into_owned(),
            text: fs::read_to_string(file).unwrap(),
        })
        .collect()
}
fn collect(path: &Path, files: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some("node_modules" | "target" | "vendor" | ".git" | ".cache")
            ) {
                continue;
            }
            collect(&entry.path(), files);
        }
    } else if path
        .extension()
        .is_some_and(|ext| matches!(ext.to_str(), Some("ts" | "tsx" | "mts" | "cts")))
    {
        files.push(path.to_owned());
    }
}
pub fn parse(source: &Source) -> typescript_parser::Ast<'_> {
    typescript_parser::parse_with_options(
        &source.text,
        typescript_parser::ParseOptions::for_file(source.name.as_str()),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", source.name))
}
