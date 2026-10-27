//! Package ids: the identity TypeScript gives a file that a lookup finds in a
//! package, from the package's manifest (its `name` and `version`), the
//! file's path in the package, and the versions of the package's peers
//! installed beside it. Files with one id are one program input
//! (`collectFiles`). This ports 5.6.3's `withPackageId`,
//! `readPackageJsonPeerDependencies` and `packageIdToString` and 7.0.2's
//! `getPackageId`, `readPackageJsonPeerDependencies` and `PackageId`; the
//! resolution `Rules` decide where the two differ. Which lookups give a file
//! an id is the lookups' own decision (`Found`), as each site of
//! `withPackageId` decides in TypeScript.

use super::super::manifest::{Kind, PackageInfo};
use super::{Profile, Rules};
use crate::analysis::{InputError, InputErrorKind};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// A file's package id. Ids compare as their compiler compares them: 7.0.2's
/// `PackageId` struct by its four parts, 5.6.3's `packageIdToString` text,
/// which joins them.
#[derive(Debug, Clone)]
pub(in crate::project::program) struct PackageId {
    name: String,
    sub_module: String,
    version: String,
    /// `+name@version` for each installed peer (`peer_names`).
    peers: String,
    /// Whether ids compare by their joined text.
    joined: bool,
}

impl PackageId {
    pub(super) fn new(
        rules: Rules,
        name: &str,
        sub_module: String,
        version: &str,
        peers: String,
    ) -> Self {
        Self {
            name: name.to_owned(),
            sub_module,
            version: version.to_owned(),
            peers,
            joined: rules.0 == Profile::TypeScript56,
        }
    }

    fn parts(&self) -> (&str, &str, &str, &str) {
        (&self.name, &self.sub_module, &self.version, &self.peers)
    }
}

impl PartialEq for PackageId {
    fn eq(&self, other: &Self) -> bool {
        if self.joined || other.joined {
            self.to_string() == other.to_string()
        } else {
            self.parts() == other.parts()
        }
    }
}

impl Eq for PackageId {}

impl Hash for PackageId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.joined {
            self.to_string().hash(state);
        } else {
            self.parts().hash(state);
        }
    }
}

/// `packageIdToString` / `PackageId.String`: `name/subModule@version` and
/// the peers, as both compilers explain a file's inclusion.
impl fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)?;
        if !self.sub_module.is_empty() {
            write!(formatter, "/{}", self.sub_module)?;
        }
        write!(formatter, "@{}{}", self.version, self.peers)
    }
}

/// `parseNodeModuleFromPath` / `ParseNodeModuleFromPath` for a file: the
/// package directory under the last `/node_modules/` of `path`, a scope
/// included, or `None` outside `node_modules`. A file directly in
/// `node_modules` names that directory, separator included, as both
/// compilers slice it.
pub(super) fn package_directory(path: &str) -> Option<&str> {
    const NODE_MODULES: &str = "/node_modules/";
    let after = path.rfind(NODE_MODULES)? + NODE_MODULES.len();
    let next = |from: usize| {
        path.get(from + 1..)
            .and_then(|rest| rest.find('/'))
            .map_or(from, |at| from + 1 + at)
    };
    let mut end = next(after);
    if path.as_bytes().get(after) == Some(&b'@') {
        end = next(end);
    }
    Some(&path[..end])
}

/// The id's path of `path` in the package `directory`: the rest of the path
/// after the directory and a separator, as both compilers slice it by the
/// directory's length, 7.0.2 in bytes and 5.6.3 in UTF-16 units. A path
/// outside the directory whose slice would split a character has no exact
/// text, so it is unsupported rather than another file's name.
pub(super) fn sub_module(rules: Rules, path: &str, directory: &str) -> Result<String, InputError> {
    if let Some(rest) = path
        .strip_prefix(directory)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        return Ok(rest.to_owned());
    }
    let start = match rules.0 {
        Profile::TypeScript7 => Some(directory.len() + 1),
        Profile::TypeScript56 => utf16_offset(path, directory.encode_utf16().count() + 1),
    };
    match start {
        Some(start) if start >= path.len() => Ok(String::new()),
        Some(start) if path.is_char_boundary(start) => Ok(path[start..].to_owned()),
        _ => Err(inexact(path)),
    }
}

/// The byte offset of the first `units` UTF-16 units of `text`, if that
/// falls between characters (all of `text` when it is shorter).
fn utf16_offset(text: &str, units: usize) -> Option<usize> {
    let mut counted = 0;
    for (at, character) in text.char_indices() {
        if counted == units {
            return Some(at);
        }
        counted += character.len_utf16();
        if counted > units {
            return None;
        }
    }
    Some(text.len())
}

fn inexact(path: &str) -> InputError {
    InputError {
        kind: InputErrorKind::Unsupported,
        message:
            "the package id of this file would split a character, so it cannot be named exactly"
                .into(),
        path: Some(PathBuf::from(path)),
        position: None,
    }
}

/// The peers whose installed versions a package's id includes, in the order
/// its suffix lists them, or `None` for no suffix. 7.0.2 reads
/// `peerDependencies` only as a valid string map, its names sorted; 5.6.3
/// reads any object (`readPackageJsonField(…, "object")`), an array too, in
/// JavaScript's `for…in` order.
pub(super) fn peer_names(rules: Rules, package: &PackageInfo) -> Option<Vec<String>> {
    let names = match rules.0 {
        Profile::TypeScript7 => package.peer_dependencies()?.to_vec(),
        Profile::TypeScript56 => match &package.contents.as_ref()?.get("peerDependencies")?.kind {
            Kind::Object(entries) => {
                for_in_order(entries.iter().map(|(name, _)| name.clone()).collect())
            }
            Kind::Array(items) => (0..items.len()).map(|index| index.to_string()).collect(),
            _ => return None,
        },
    };
    (!names.is_empty()).then_some(names)
}

/// JavaScript's own-property order of an object's keys, in which `for…in`
/// visits them: array indices first, ascending, then the other keys in
/// insertion order.
fn for_in_order(keys: Vec<String>) -> Vec<String> {
    let index = |key: &str| {
        let canonical = key == "0" || !key.starts_with('0');
        key.parse::<u32>().ok().filter(|&value| {
            canonical && value != u32::MAX && key.bytes().all(|b| b.is_ascii_digit())
        })
    };
    let (mut indices, names): (Vec<String>, Vec<String>) =
        keys.into_iter().partition(|key| index(key).is_some());
    indices.sort_by_key(|key| index(key));
    indices.extend(names);
    indices
}

/// The `node_modules` directory, with a final separator, beside a package's
/// real directory where each compiler looks for its peers. 7.0.2 takes the
/// last `/node_modules` and has no peers without one; 5.6.3 takes the last
/// `node_modules`, and without one keeps the first eleven UTF-16 units of
/// the path (`lastIndexOf` gives -1), where no peer is normally installed.
pub(super) fn peer_directory(
    rules: Rules,
    real_directory: &str,
) -> Result<Option<String>, InputError> {
    const NODE_MODULES: &str = "node_modules";
    match rules.0 {
        Profile::TypeScript7 => Ok(real_directory
            .rfind("/node_modules")
            .map(|at| format!("{}/", &real_directory[..at + "/node_modules".len()]))),
        Profile::TypeScript56 => {
            let end = match real_directory.rfind(NODE_MODULES) {
                Some(at) => at + NODE_MODULES.len(),
                None => utf16_offset(real_directory, NODE_MODULES.len() - 1)
                    .ok_or_else(|| inexact(real_directory))?,
            };
            Ok(Some(format!("{}/", &real_directory[..end])))
        }
    }
}

/// The `name` and `version` a package id is made of, as each compiler reads
/// them, or `None` for no id. 7.0.2 takes `Name.GetValue()` and
/// `Version.GetValue()` (`getPackageId`), its `Expected[string]` decodes
/// (`manifest::ExpectedString`), and collection uses an id only with a
/// nonempty name (`filesParser.start`, `collectFiles`); 5.6.3 takes the
/// manifest object's last value of each, which must be a string, empty
/// included (`withPackageId`).
pub(super) fn header(rules: Rules, package: &PackageInfo) -> Option<(&str, &str)> {
    match rules.0 {
        Profile::TypeScript7 => {
            let name = package.name().get().filter(|name| !name.is_empty())?;
            Some((name, package.version().get()?))
        }
        Profile::TypeScript56 => {
            let contents = package.contents.as_ref()?;
            Some((
                contents.get("name")?.as_str()?,
                contents.get("version")?.as_str()?,
            ))
        }
    }
}

/// The version an installed peer's manifest contributes to the suffix:
/// 7.0.2's `Version.Value`, the last string its `Expected[string]` decoded
/// (empty after none or a `null`); 5.6.3's template stringification of the
/// last value, which gives `undefined` for a missing version or a manifest
/// it cannot read and `null` or a boolean's text as written. Other values
/// stringify to text this tool does not reproduce (a number's formatting,
/// `[object Object]`), which would merge distinct versions, so they are
/// unsupported.
pub(super) fn peer_version(rules: Rules, peer: &PackageInfo) -> Result<String, InputError> {
    if rules.0 == Profile::TypeScript7 {
        return Ok(peer.version().value.clone());
    }
    let version = peer
        .contents
        .as_ref()
        .and_then(|contents| contents.get("version"));
    match version.map(|version| &version.kind) {
        Some(Kind::String(text)) => Ok(text.clone()),
        None => Ok("undefined".into()),
        Some(Kind::Null) => Ok("null".into()),
        Some(Kind::Bool(value)) => Ok(value.to_string()),
        Some(_) => Err(InputError {
            kind: InputErrorKind::Unsupported,
            message: "the 'version' of this peer package is not a string, which TypeScript 5.6's package id spells in a way this tool does not reproduce".into(),
            path: Some(PathBuf::from(&peer.directory).join("package.json")),
            position: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_package_directory_of_a_file() {
        assert_eq!(
            package_directory("/p/node_modules/a/node_modules/dup/index.d.ts"),
            Some("/p/node_modules/a/node_modules/dup")
        );
        assert_eq!(
            package_directory("/p/node_modules/@s/x/lib/a.d.ts"),
            Some("/p/node_modules/@s/x")
        );
        assert_eq!(
            package_directory("/p/node_modules/a.d.ts"),
            Some("/p/node_modules/")
        );
        assert_eq!(package_directory("/p/src/a.ts"), None);
    }

    #[test]
    fn orders_keys_as_for_in_does() {
        let keys = ["b", "10", "a", "2", "02", "4294967295"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(
            for_in_order(keys),
            ["2", "10", "b", "a", "02", "4294967295"]
        );
    }

    #[test]
    fn slices_a_path_outside_its_directory_by_each_compilers_length() {
        assert_eq!(
            sub_module(Rules::TYPESCRIPT_7, "/p/x/a.d.ts", "/p/x").unwrap(),
            "a.d.ts"
        );
        assert_eq!(
            sub_module(Rules::TYPESCRIPT_7, "/p/other/a.d.ts", "/p/x").unwrap(),
            "her/a.d.ts"
        );
        assert_eq!(
            sub_module(Rules::TYPESCRIPT_5_6, "/p/éé/b.d.ts", "/p/xé").unwrap(),
            "b.d.ts"
        );
        assert_eq!(
            sub_module(Rules::TYPESCRIPT_7, "/p/éé/b.d.ts", "/p/xé").unwrap(),
            "/b.d.ts"
        );
        assert!(sub_module(Rules::TYPESCRIPT_7, "/p/éé/b.d.ts", "/p/xy").is_err());
    }

    #[test]
    fn finds_the_peer_directory_by_each_compilers_rule() {
        assert_eq!(
            peer_directory(Rules::TYPESCRIPT_7, "/p/node_modules/a/node_modules/x").unwrap(),
            Some("/p/node_modules/a/node_modules/".into())
        );
        assert_eq!(
            peer_directory(Rules::TYPESCRIPT_7, "/p/packages/x").unwrap(),
            None
        );
        assert_eq!(
            peer_directory(Rules::TYPESCRIPT_5_6, "/p/my_node_modules/x").unwrap(),
            Some("/p/my_node_modules/".into())
        );
        assert_eq!(
            peer_directory(Rules::TYPESCRIPT_5_6, "/Users/someone/repo/x").unwrap(),
            Some("/Users/some/".into())
        );
    }
}
