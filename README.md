<div align="center">

# wt — git-worktree-smart

**A lightweight, zero-config Git worktree manager.**

Beautiful, human-friendly Git worktrees without the setup. No bare repositories.
No complex invocation. Just smarter worktrees.

[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/badge/Cargo-0.1.0-brightgreen.svg)](Cargo.toml)

</div>

---

## Why `wt`?

Git worktrees are powerful, but the raw CLI is tedious. Creating a worktree means
hand-typing paths and juggling `-b` flags. Removing one can silently destroy
uncommitted work. `wt` fixes all of that:

- **Zero configuration** — `wt add feature` just works.
- **Smart path inference** — automatically names and places new worktrees.
- **Fuzzy matching** — `wt path log` resolves `feature/login`.
- **Non-destructive by default** — refuses to delete work unless you insist.
- **Beautiful *and* scriptable** — pretty tables for humans, strict JSON for machines.

---

## ✨ Features

| Command | Description |
|---------|-------------|
| `wt` / `wt list` | List every worktree with branch, status, path & commit |
| `wt add <branch>` | Create a worktree, auto-inferring the path |
| `wt switch <query>` | Resolve a worktree and print a shell snippet to switch |
| `wt path <query>` | Print the absolute path (for `cd $(wt path ...)`) |
| `wt remove <query>` | Safely remove a worktree (guards against data loss) |
| `wt prune` | Clean stale references — dry-run by default |

### Smart path inference

```
$ wt add feature/login
✓ Created worktree for feature/login at ~/repo-feature-login
```

`feature/login` becomes a sibling directory `repo-feature-login` — no more
typing out full paths. Slashes are safely converted for the filesystem.

### Fuzzy matching

```
$ wt path log
/home/dev/repo-feature-login
```

Every query goes through an exact → substring → fuzzy resolution pipeline, so
`wt path log`, `wt switch logi`, and `wt rm log` all just work.

### Non-destructive by default

```
$ wt remove feature/login
Error: Worktree at ~/repo-feature-login has uncommitted changes. 💡 Pass --force to delete it anyway.
```

`remove` and `prune` are guarded: dirty worktrees, unpushed commits, and stale
references are all surfaced *before* anything destructive happens.

### Human-friendly *and* scriptable

```bash
$ wt list
┌────────────┬───────────────────┬─────────┬──────────────────────────┬──────────────────┐
│            │ Branch            │ Status  │ Path                     │ Commit           │
╞════════════╪═══════════════════╪═════════╪══════════════════════════╪══════════════════╡
│ *          │ main              │ clean   │ ~/repo                   │ 4d9f1a2 fix bugs  │
│            │ feature/login     │ ahead 2 │ ~/repo-feature-login     │ 8c3b7d1 add auth   │
└────────────┴───────────────────┴─────────┴──────────────────────────┴──────────────────┘

$ wt list --json
[
  {
    "path": "/home/dev/repo",
    "name": "main",
    ...
  }
]
```

---

## 📦 Installation

### From source

```bash
git clone https://github.com/unknownman/git-worktree-smart.git
cd git-worktree-smart
cargo install --path .
```

Or build locally:

```bash
cargo build --release
# binary at target/release/wt
```

### Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.60+ (edition 2021)
- [Git](https://git-scm.com) on your `PATH`

`wt` shells out to the native `git` CLI — no `libgit2` or system libraries required.

---

## 🚀 Quick Start

```bash
# In any Git repository:
wt                          # list your worktrees

# Create a worktree on a new branch:
wt add feature/auth

# Create a worktree branching from main:
wt add hotfix main

# Track a remote branch:
wt add deploy --track origin/deploy

# Find and switch to a worktree by fuzzy query:
wt switch login

# Script-friendly path lookup:
cd "$(wt path feature/auth)"

# Remove safely, or force:
wt remove feature/auth
wt remove --force feature/auth

# Preview then execute a prune:
wt prune
wt prune --yes
```

### Shell integration for `wt switch`

Because a child process cannot change the parent shell's directory, add this to
`~/.zshrc` or `~/.bashrc` to make `wt switch` actually `cd` for you:

```bash
wt() {
    if [ "$1" = "switch" ] || [ "$1" = "cd" ]; then
        shift
        local target_path
        target_path="$(command wt path "$@")"
        if [ $? -eq 0 ] && [ -n "$target_path" ]; then
            cd "$target_path"
        fi
    else
        command wt "$@"
    fi
}
```

---

## 🖥️ Commands

### `wt list` (`ls`)

Displays all worktrees sorted alphabetically, showing the current worktree with
a `*`, the linked branch, status (clean / dirty / ahead / behind), path, and the
latest commit on HEAD.

### `wt add <name> [base] [--track <remote>]`

Creates a new worktree. The path is inferred as a sibling of the repository:
`<repo-dir>` → `<parent>/<repo-name>-<sanitized-branch>`.

- If the branch exists, it is checked out.
- If not, it is created from `base` (defaults to `HEAD`).
- `--track` sets upstream tracking.

### `wt switch <query>`

Resolves a worktree by fuzzy query and prints a shell snippet explaining how to
switch to it (see Shell integration above). With `--json`, prints the resolved
worktree.

### `wt path <query>`

Prints only the absolute path of a resolved worktree to stdout — safe for shell
evaluation with `$(...)`. On error, writes to stderr and exits non-zero so the
shell never `cd`s into garbage.

### `wt remove <query> [--force]`

Safely removes a worktree. Guards:

- The **main** worktree can never be removed.
- A **dirty** worktree or one with **unpushed commits** is refused unless `--force`.

### `wt prune [--yes]`

Wraps `git worktree prune` (cleans references to deleted worktrees). By default
it is a **dry run** showing what would be removed. Pass `--yes` to execute.

---

## ⚙️ Global Options

| Option | Description |
|--------|-------------|
| `--json` | Machine-readable JSON output for every command |
| `-v, --verbose` | Show the underlying `git` commands being executed |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

---

## 🔧 JSON Output

When `--json` is set, output is strictly structured:

- **List / prune**: arrays (`[...]`)
- **Single target** (add / switch / remove): objects (`{...}`)
- **Errors**: `{"error": "<message>"}` written to **stderr**

Errors never corrupt stdout, so `--json` is safe to pipe into tools like `jq`.

---

## 🏗️ Architecture

```
src/
├── main.rs          # Entry point, dispatch, global error handler
├── cli.rs           # clap definitions (branding, flags, examples)
├── error.rs         # AppError — thiserror, actionable messages
├── models/          # WorktreeInfo, WorktreeStatus (serde + Display)
├── git/
│   ├── command.rs   # Thin wrappers over std::process::Command
│   ├── ops.rs       # Mutating operations (add / remove / prune)
│   ├── parse.rs     # Pure parsers for git output (unit-tested)
│   └── resolve.rs   # Exact → substring → fuzzy worktree resolution
├── cmd/             # One module per subcommand
└── output/          # human (comfy-table + owo-colors) and JSON renderers
```

The design favors **small, tested, pure functions** for parsing, and thin shell
invocations for anything that touches Git.

---

## 🧪 Testing

```bash
cargo test         # unit + integration tests
cargo clippy       # lint (zero warnings)
```

Test coverage includes:

- Parsing `git worktree list --porcelain` (incl. detached HEAD, multiple, bare)
- Dirty / ahead / behind status detection
- Prune dry-run parsing (both older and modern git output formats)
- Worktree path sanitization and inference
- Exact / substring / fuzzy resolution with ambiguity detection
- Black-box CLI tests (`assert_cmd`) for JSON, aliases, errors, and round-trips

---

## 📄 License

[MIT](LICENSE)
