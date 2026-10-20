//! Command lines a caller can follow as printed.

use super::registry;

/// `argv` as one POSIX shell command line: each argument that is not a safe
/// word is single-quoted, with `'` written as `'\''`. A safe word matches
/// `[A-Za-z0-9_./:=@%+,-]+` and begins with `-` only when it spells one of
/// this program's registered flags. The one owner of rendering an argument
/// list as shell text: every printed follow-up command is rendered here.
pub(crate) fn render(argv: &[impl AsRef<str>]) -> String {
    let mut line = String::new();
    for (index, arg) in argv.iter().map(AsRef::as_ref).enumerate() {
        if index > 0 {
            line.push(' ');
        }
        if is_safe(arg) {
            line.push_str(arg);
        } else {
            line.push('\'');
            line.push_str(&arg.replace('\'', r"'\''"));
            line.push('\'');
        }
    }
    line
}

/// Whether `arg` is a word a shell reads as itself, and that reads as a
/// flag only when it is one.
fn is_safe(arg: &str) -> bool {
    !arg.is_empty()
        && arg
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:=@%+,-".contains(&byte))
        && (!arg.starts_with('-') || registry::flag(arg).is_some())
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn safe_words_and_flags_stay_as_written() {
        assert_eq!(
            render(&["tsparse", "--apply", "plans/a-1_b.json"]),
            "tsparse --apply plans/a-1_b.json"
        );
        assert_eq!(
            render(&["AZaz09_./:=@%+,-", "-o", "--output", "--help", "-h"]),
            "AZaz09_./:=@%+,- -o --output --help -h"
        );
    }

    #[test]
    fn every_other_character_is_quoted() {
        for (arg, word) in [
            ("my plan.json", "'my plan.json'"),
            ("a\tb", "'a\tb'"),
            ("a\nb", "'a\nb'"),
            ("$plan", "'$plan'"),
            ("a;b", "'a;b'"),
            ("a&b|c", "'a&b|c'"),
            ("*?[x]", "'*?[x]'"),
            ("~/x", "'~/x'"),
            ("#x", "'#x'"),
            ("(x)", "'(x)'"),
            ("{x}", "'{x}'"),
            ("<x>", "'<x>'"),
            ("!x", "'!x'"),
            ("`x`", "'`x`'"),
            ("\"x\"", "'\"x\"'"),
            ("a\\b", "'a\\b'"),
            ("é.json", "'é.json'"),
            ("", "''"),
        ] {
            assert_eq!(render(&[arg]), word, "{arg:?}");
        }
    }

    #[test]
    fn a_quote_closes_the_quoted_word_around_an_escaped_quote() {
        assert_eq!(render(&["it's.json"]), r"'it'\''s.json'");
        assert_eq!(render(&["'"]), r"''\'''");
        assert_eq!(render(&["''"]), r"''\'''\'''");
    }

    #[test]
    fn a_leading_dash_is_quoted_unless_it_begins_a_flag() {
        for (arg, word) in [
            ("--apply", "--apply"),
            ("-o", "-o"),
            ("-plan.json", "'-plan.json'"),
            ("--plan.json", "'--plan.json'"),
            ("--Apply", "'--Apply'"),
            ("--apply=x", "'--apply=x'"),
            ("-", "'-'"),
            ("--", "'--'"),
            ("a-b", "a-b"),
        ] {
            assert_eq!(render(&[arg]), word, "{arg:?}");
        }
    }

    /// A POSIX shell reads every rendered line back as the arguments it
    /// renders.
    #[cfg(unix)]
    #[test]
    fn a_shell_reads_back_the_exact_arguments() {
        let argv = [
            "my plan.json",
            "it's",
            "'",
            "$HOME",
            "a;b",
            "a\nb\tc",
            "*",
            "~",
            "é",
            "",
            "-plan.json",
            "--apply",
            "x\\y",
            "\"",
            "`echo x`",
        ];
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf '%s\\0' {}", render(&argv)))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let words: Vec<&str> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_terminator('\0')
            .collect();
        assert_eq!(words, argv);
    }
}
