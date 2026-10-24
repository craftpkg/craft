# Windows

craft runs natively on Windows (x64) and needs neither administrator rights nor Developer Mode. Projects keep the same `node_modules` layout and the same `craft.lock` as on macOS and Linux; a few things are done differently underneath.

## Directory links are junctions

Windows only lets administrators, or users with Developer Mode on, create symbolic links. craft links directories with **junctions** instead, which any user can create and which Node.js follows like symlinks.

Junctions must point to absolute paths, so unlike on macOS and Linux, a project's `node_modules` stops working if you move the project directory. Run `craft install` again after moving it.

## Bins are shims

Each bin in `node_modules/.bin` is three small files, the same ones npm writes:

| File | Used by |
| --- | --- |
| `tsc.cmd` | cmd.exe, and tools that look commands up through `PATHEXT` |
| `tsc.ps1` | PowerShell |
| `tsc` | Git Bash and other sh shells |

A shim runs the program named in the bin's shebang line (`#!/usr/bin/env node` runs `node`), preferring a `node.exe` next to it, and passes the exit code through. A bin without a shebang, such as an `.exe`, runs directly.

A bin whose shebang names a shell, `#!/bin/sh`, needs `sh` on `PATH`, as with npm: run it from Git Bash, or install Git for Windows with its tools on `PATH`.

## Scripts run in cmd

`craft run` and lifecycle scripts run through `cmd.exe /d /s /c`, exactly as written in `package.json`. Write scripts that work in cmd: use `node -e` or a cross-platform tool instead of `rm -rf`, `cp` or `$VAR`.

Arguments you pass, as in `craft run test --grep "a & b"`, are escaped the way npm escapes them, so characters that are special to cmd (`& | < > ^ % ! ( )` and quotes) reach the script unchanged, including through a bin's `.cmd` shim.

## Long paths

craft uses Windows' long-path APIs, so deep `node_modules` trees don't hit the 260-character `MAX_PATH` limit. Some other tools still do; if one fails on a long path, enable long paths in Windows (`LongPathsEnabled`).

## Git dependencies

Git dependencies need `git` on `PATH`; [Git for Windows](https://gitforwindows.org) provides it.
