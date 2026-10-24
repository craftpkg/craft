//! The `.cmd`, `.ps1` and sh shims npm's `cmd-shim` writes for a bin on Windows, where
//! symlinks need Developer Mode. Each runs the bin's shebang program, preferring one installed
//! beside the shim, so `#!/usr/bin/env node` scripts work from cmd, PowerShell and Git Bash.

/// The program a bin's shebang names, with its arguments: `#!/usr/bin/env -S node --flag`
/// is `node` and `--flag`. `None` for a file without a shebang, such as an `.exe`.
pub(crate) fn shebang_program(first_line: &str) -> Option<(String, String)> {
    let command = first_line.strip_prefix("#!")?.trim();
    let mut words = command.split_whitespace().peekable();
    let interpreter = words.next()?;
    let runs_through_env = interpreter.rsplit('/').next() == Some("env");
    if runs_through_env {
        while words
            .peek()
            .is_some_and(|word| word.starts_with('-') || word.contains('='))
        {
            words.next();
        }
    }
    let program = if runs_through_env {
        words.next()?
    } else {
        interpreter
    };
    let program = program.rsplit('/').next().unwrap_or(program).to_string();
    let args = words.collect::<Vec<_>>().join(" ");
    Some((program, args))
}

/// `target` is the bin relative to the shim's directory, with `/` separators.
pub(crate) fn cmd(target: &str, program: Option<&(String, String)>) -> String {
    let target = target.replace('/', "\\");
    let head = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n";
    let Some((program, args)) = program else {
        return format!("{head}\r\n\"%dp0%\\{target}\" %*\r\n");
    };
    let args = spaced(args);
    format!(
        "{head}\r\nIF EXIST \"%dp0%\\{program}.exe\" (\r\n  SET \"_prog=%dp0%\\{program}.exe\"\r\n) ELSE (\r\n  SET \"_prog={program}\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\" {args}\"%dp0%\\{target}\" %*\r\n"
    )
}

pub(crate) fn powershell(target: &str, program: Option<&(String, String)>) -> String {
    let head =
        "#!/usr/bin/env pwsh\n$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n\n";
    let Some((program, args)) = program else {
        return format!(
            "{head}if ($MyInvocation.ExpectingInput) {{\n  $input | & \"$basedir/{target}\" $args\n}} else {{\n  & \"$basedir/{target}\" $args\n}}\nexit $LASTEXITCODE\n"
        );
    };
    let args = spaced(args);
    let invoke = |program_path: &str| {
        format!(
            "  if ($MyInvocation.ExpectingInput) {{\n    $input | & \"{program_path}$exe\" {args}\"$basedir/{target}\" $args\n  }} else {{\n    & \"{program_path}$exe\" {args}\"$basedir/{target}\" $args\n  }}\n  $ret=$LASTEXITCODE\n"
        )
    };
    format!(
        "{head}$exe=\"\"\nif ($PSVersionTable.PSVersion -lt \"6.0\" -or $IsWindows) {{\n  $exe=\".exe\"\n}}\n$ret=0\nif (Test-Path \"$basedir/{program}$exe\") {{\n{}}} else {{\n{}}}\nexit $ret\n",
        invoke(&format!("$basedir/{program}")),
        invoke(program),
    )
}

pub(crate) fn sh(target: &str, program: Option<&(String, String)>) -> String {
    let head = "#!/bin/sh\nbasedir=$(dirname \"$(echo \"$0\" | sed -e 's,\\\\,/,g')\")\n\ncase `uname` in\n    *CYGWIN*|*MINGW*|*MSYS*)\n        if command -v cygpath > /dev/null 2>&1; then\n            basedir=`cygpath -w \"$basedir\"`\n        fi\n    ;;\nesac\n\n";
    let Some((program, args)) = program else {
        return format!("{head}exec \"$basedir/{target}\" \"$@\"\n");
    };
    let args = spaced(args);
    format!(
        "{head}if [ -x \"$basedir/{program}\" ]; then\n  exec \"$basedir/{program}\" {args}\"$basedir/{target}\" \"$@\"\nelse\n  exec {program} {args}\"$basedir/{target}\" \"$@\"\nfi\n"
    )
}

/// The bin a `.cmd` shim runs, relative to the shim's directory with `/` separators.
pub(crate) fn cmd_target(shim: &str) -> Option<String> {
    let (_, rest) = shim.rsplit_once("\"%dp0%\\")?;
    let (target, _) = rest.split_once('"')?;
    Some(target.replace('\\', "/"))
}

fn spaced(args: &str) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!("{args} ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> (String, String) {
        ("node".to_string(), String::new())
    }

    #[test]
    fn test_reads_the_program_from_a_shebang() {
        let cases = [
            ("#!/usr/bin/env node", Some(("node", ""))),
            ("#!/usr/bin/node", Some(("node", ""))),
            (
                "#! /usr/bin/env -S node --no-warnings",
                Some(("node", "--no-warnings")),
            ),
            (
                "#!/usr/bin/env NODE_ENV=production node",
                Some(("node", "")),
            ),
            ("#!/bin/sh", Some(("sh", ""))),
            ("MZ\u{90}", None),
            ("'use strict'", None),
        ];
        for (line, expected) in cases {
            let expected = expected.map(|(program, args)| (program.to_string(), args.to_string()));
            assert_eq!(shebang_program(line), expected, "{line}");
        }
    }

    #[test]
    fn test_cmd_shim_runs_node_with_the_target_and_arguments() {
        let shim = cmd("../typescript/bin/tsc", Some(&node()));
        assert!(shim.contains("SET \"_prog=%dp0%\\node.exe\""));
        assert!(shim.ends_with("\"%_prog%\" \"%dp0%\\..\\typescript\\bin\\tsc\" %*\r\n"));
        let has_bare_newline = shim.replace("\r\n", "").contains('\n');
        assert!(!has_bare_newline);
    }

    #[test]
    fn test_shims_without_a_program_run_the_target_itself() {
        let target = "../esbuild/bin/esbuild.exe";
        assert!(cmd(target, None).ends_with("\"%dp0%\\..\\esbuild\\bin\\esbuild.exe\" %*\r\n"));
        assert!(
            powershell(target, None).contains("& \"$basedir/../esbuild/bin/esbuild.exe\" $args")
        );
        assert!(
            sh(target, None).ends_with("exec \"$basedir/../esbuild/bin/esbuild.exe\" \"$@\"\n")
        );
    }

    #[test]
    fn test_shims_pass_shebang_arguments_before_the_target() {
        let program = ("node".to_string(), "--no-warnings".to_string());
        let target = "../cli/index.js";
        assert!(
            cmd(target, Some(&program))
                .contains("\"%_prog%\" --no-warnings \"%dp0%\\..\\cli\\index.js\" %*")
        );
        assert!(
            powershell(target, Some(&program))
                .contains("& \"node$exe\" --no-warnings \"$basedir/../cli/index.js\" $args")
        );
        assert!(
            sh(target, Some(&program))
                .contains("exec node --no-warnings \"$basedir/../cli/index.js\" \"$@\"")
        );
    }

    #[test]
    fn test_reads_the_target_back_from_a_cmd_shim() {
        for program in [None, Some(node())] {
            let shim = cmd("../@scope/tool/bin/cli.js", program.as_ref());
            assert_eq!(
                cmd_target(&shim).as_deref(),
                Some("../@scope/tool/bin/cli.js")
            );
        }
        assert_eq!(cmd_target("@ECHO off\r\nnode cli.js %*\r\n"), None);
    }
}
