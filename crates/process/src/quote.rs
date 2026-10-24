use std::path::Path;

/// `script` followed by `args`, each quoted so it reaches the script as one argument with no
/// shell expansion.
pub fn with_args(script: &str, args: &[String], cwd: &Path) -> String {
    #[cfg(unix)]
    let quote = {
        let _ = cwd;
        |arg: &String| sh_quote(arg)
    };
    #[cfg(windows)]
    let quote = {
        let double_escape = runs_batch_file(script, cwd);
        move |arg: &String| cmd_quote(arg, double_escape)
    };

    let mut command = script.to_string();
    for arg in args {
        command.push(' ');
        command.push_str(&quote(arg));
    }
    command
}

#[cfg(any(unix, test))]
fn sh_quote(arg: &str) -> String {
    let is_plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if is_plain {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// npm's quoting for `cmd /s /c`: the MSVC rules a program uses to split its command line,
/// then `^` before each cmd metacharacter. A batch file (such as a bin's `.cmd` shim) hands
/// its arguments to another cmd, which strips one level of `^`, so those get two.
#[cfg(any(windows, test))]
fn cmd_quote(arg: &str, double_escape: bool) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    let needs_quotes = arg.contains([' ', '\t', '\n', '\x0b', '"']);
    let quoted = if needs_quotes {
        msvc_quote(arg)
    } else {
        arg.to_string()
    };
    let escape_rounds = if double_escape { 2 } else { 1 };
    (0..escape_rounds).fold(quoted, |text, _| caret_escape(&text))
}

#[cfg(any(windows, test))]
fn msvc_quote(arg: &str) -> String {
    let mut quoted = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                quoted.push(c);
                backslashes = 0;
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(any(windows, test))]
fn caret_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if " !%^&()<>|\"".contains(c) {
            escaped.push('^');
        }
        escaped.push(c);
    }
    escaped
}

/// Whether the script's first word resolves, the way cmd looks it up on `PATH`, to a `.cmd`
/// or `.bat` file.
#[cfg(windows)]
fn runs_batch_file(script: &str, cwd: &Path) -> bool {
    const LOOKUP_ORDER: [&str; 4] = ["com", "exe", "bat", "cmd"];

    let program = first_word(script);
    let is_batch = |name: &str| {
        let lowercase = name.to_ascii_lowercase();
        lowercase.ends_with(".cmd") || lowercase.ends_with(".bat")
    };
    if is_batch(&program) {
        return true;
    }
    let Ok(path) = crate::script_path(cwd) else {
        return false;
    };
    let program = &program;
    std::env::split_paths(&path)
        .flat_map(|dir| {
            LOOKUP_ORDER
                .iter()
                .map(move |extension| dir.join(format!("{program}.{extension}")))
        })
        .find(|candidate| candidate.is_file())
        .is_some_and(|found| is_batch(&found.to_string_lossy()))
}

#[cfg(any(windows, test))]
fn first_word(script: &str) -> String {
    let mut word = String::new();
    let mut is_quoted = false;
    for c in script.trim_start().chars() {
        let ends_word = c.is_whitespace() && !is_quoted;
        if ends_word {
            break;
        }
        if c == '"' {
            is_quoted = !is_quoted;
            continue;
        }
        word.push(c);
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quotes_arguments_for_sh() {
        let args = ["plain-arg_1.js", "a; echo INJECTED", "it's", "", "$HOME"];
        let quoted: Vec<String> = args.iter().map(|arg| sh_quote(arg)).collect();
        assert_eq!(
            quoted,
            [
                "plain-arg_1.js",
                "'a; echo INJECTED'",
                r"'it'\''s'",
                "''",
                "'$HOME'"
            ]
        );
    }

    #[test]
    fn test_quotes_arguments_for_cmd_like_npm() {
        let cases = [
            ("plain-arg_1.js", "plain-arg_1.js"),
            ("", "\"\""),
            ("two words", "^\"two^ words^\""),
            ("a&calc", "a^&calc"),
            ("say \"hi\"", r#"^"say^ \^"hi\^"^""#),
            (r"C:\dir\", r"C:\dir\"),
            (r"C:\my dir\", r#"^"C:\my^ dir\\^""#),
            ("100%", "100^%"),
        ];
        for (arg, expected) in cases {
            assert_eq!(cmd_quote(arg, false), expected, "{arg}");
        }
    }

    #[test]
    fn test_escapes_twice_for_batch_files() {
        assert_eq!(cmd_quote("a&b", true), "a^^^&b");
        assert_eq!(cmd_quote("x y", true), "^^^\"x^^^ y^^^\"");
    }

    #[test]
    fn test_finds_the_program_of_a_script() {
        assert_eq!(first_word("tsc --noEmit"), "tsc");
        assert_eq!(first_word("  \"my tool\" arg"), "my tool");
        assert_eq!(first_word("node"), "node");
    }

    #[cfg(unix)]
    #[test]
    fn test_appends_quoted_arguments() {
        let args = ["--flag".to_string(), "two words".to_string()];
        assert_eq!(
            with_args("vite build", &args, Path::new("/app")),
            "vite build --flag 'two words'"
        );
    }
}
