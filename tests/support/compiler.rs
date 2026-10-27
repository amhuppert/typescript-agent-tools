//! The TypeScript compiler a test project installs, which selects the rules
//! that read it (docs/references.md, "Compiler selection"). The integration
//! tests reach this through `support`; the crate's own tests include this
//! file by path (`project::program::test_compiler`), so both select a
//! dialect, spell a manifest, and find the pinned 5.6.3 package alike.

use std::path::{Path, PathBuf};

/// Where a project's installed compiler declares its version.
pub const MANIFEST: &str = "node_modules/typescript/package.json";

/// Options both dialects accept and read alike, without standard or
/// automatic type libraries.
pub const BASE_OPTIONS: &str = r#""noLib":true,"types":[],"strict":true,"target":"es2022","module":"esnext","moduleResolution":"bundler","esModuleInterop":true"#;

/// The manifest of a TypeScript package claiming `version`.
pub fn manifest(version: &str) -> String {
    format!(r#"{{"name":"typescript","version":"{version}"}}"#)
}

/// The rules that read a test project, by the compiler it installs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// TypeScript 7.0.2 installed. A project with no TypeScript package
    /// selects the same dialect; tests of that case write no manifest.
    TypeScript7,
    /// TypeScript 5.6.3 installed.
    TypeScript56,
}

/// Both dialects, TypeScript 7 first.
pub const DIALECTS: [Dialect; 2] = [Dialect::TypeScript7, Dialect::TypeScript56];

impl Dialect {
    /// The version of the compiler a project read with this dialect installs.
    pub fn version(self) -> &'static str {
        match self {
            Dialect::TypeScript7 => "7.0.2",
            Dialect::TypeScript56 => "5.6.3",
        }
    }

    /// The installed compiler's manifest, the text of `MANIFEST`.
    pub fn manifest(self) -> String {
        manifest(self.version())
    }
}

/// The pinned TypeScript 5.6.3 package, as `tools/ts56-compiler.mjs`
/// locates it: `TYPE_SCRIPT_56_API`, else the canonical checkout's
/// `.cache/compiler-oracles/typescript-5.6.3/package`. `None`, after saying
/// why, when it is not there: the TypeScript 5.6 dialect reads the
/// installed compiler's own declarations, which only that package has.
pub fn typescript_5_6(test: &str) -> Option<PathBuf> {
    let package = match std::env::var_os("TYPE_SCRIPT_56_API") {
        Some(package) => PathBuf::from(package),
        None => {
            let common = std::process::Command::new("git")
                .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .unwrap();
            let common = String::from_utf8(common.stdout).unwrap();
            Path::new(common.trim())
                .parent()
                .unwrap()
                .join(".cache/compiler-oracles/typescript-5.6.3/package")
        }
    };
    if package.join("package.json").is_file() {
        Some(package)
    } else {
        eprintln!(
            "SKIP {test}: no TypeScript 5.6.3 package at {}; set TYPE_SCRIPT_56_API",
            package.display()
        );
        None
    }
}

/// Install a copy of the TypeScript package at `package` as the compiler of
/// the project in `root`, `node_modules/typescript`: its manifest, its
/// standard library declarations, and `typescript.d.ts`. Returns the
/// copy's directory.
pub fn install(package: &Path, root: &Path) -> PathBuf {
    let installed = root.join("node_modules/typescript");
    std::fs::create_dir_all(installed.join("lib")).unwrap();
    std::fs::copy(package.join("package.json"), installed.join("package.json")).unwrap();
    for entry in std::fs::read_dir(package.join("lib")).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name.ends_with(".d.ts") && !name.starts_with("tsserverlibrary") {
            std::fs::copy(
                package.join("lib").join(&name),
                installed.join("lib").join(&name),
            )
            .unwrap();
        }
    }
    installed
}
