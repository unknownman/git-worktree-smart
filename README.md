<div align="center">

# wt — git-worktree-smart

**A lightweight, zero-config Git worktree manager.**

Beautiful, human-friendly Git worktrees without the setup. No bare repositories.
No complex invocation. Just smarter worktrees.

[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/git-worktree-smart.svg)](https://crates.io/crates/git-worktree-smart)

<br />

<div align="center">
  <img src="demo.gif" alt="wt in action - Git worktrees made human" width="100%" />
</div>

</div>

---

## Why `wt`?

Git worktrees are powerful, but the raw CLI is tedious. Creating a worktree means
hand-typing paths and juggling `-b` flags. Removing one can silently destroy
uncommitted work. `wt` fixes all of that:

- **Zero configuration** — `wt add feature` just works in any existing Git repository.
- **Smart path inference** — automatically names and places new worktrees alongside your repo.
- **Fuzzy matching** — `wt path log` seamlessly resolves `feature/login`.
- **Non-destructive by default** — guards against dirty states, unpushed commits, and orphaned detached commits.
- **Beautiful *and* scriptable** — rich, colorful tables for humans; clean, deterministic JSON for tooling.

---

## ✨ Features & Capabilities

| Command | Alias | Description |
|---------|-------|-------------|
| `wt` / `wt list` | `wt ls` | List every worktree with branch, status, path & commit |
| `wt add <name> [base]` | | Create a worktree with smart path inference and optional tracking |
| `wt switch <query>` | `wt cd` | Resolve a worktree and switch to it via shell wrapper |
| `wt path <query>` | | Print absolute path to stdout (perfect for `cd "$(wt path ...)"`) |
| `wt remove <query>` | `wt rm` | Safely remove a worktree with multi-layer data loss protection |
| `wt prune` | | Clean stale index references — safe dry-run preview by default |

### Smart path inference

```bash
$ wt add feature/login
✓ Created worktree for feature/login at ~/repo-feature-login
```

`feature/login` becomes a sibling directory `repo-feature-login` — no more
typing out full paths. Slashes are safely converted for the filesystem.

### Fuzzy matching

```bash
$ wt path log
/home/dev/repo-feature-login
```

Every query goes through an exact → substring → fuzzy resolution pipeline, so
`wt path log`, `wt switch logi`, and `wt rm log` all resolve accurately.

### Safe by default

```bash
$ wt remove feature/login
Error: Worktree at ~/repo-feature-login has uncommitted changes.
💡 Pass --force to delete it anyway.
```

`wt remove` enforces strict safety guards:
- **Dirty working trees** with uncommitted changes are protected.
- **Unpushed commits** ahead of the upstream branch are blocked.
- **Detached HEAD** commits unreachable from any other branch are protected from becoming orphaned.
- **The main repository root** and the **currently active worktree** cannot be accidentally removed.

### Human-friendly *and* scriptable

```text
$ wt list
┌───┬───────────────────┬─────────┬──────────────────────────┬──────────────────┐
│   │ Branch            │ Status  │ Path                     │ Commit           │
├───┼───────────────────┼─────────┼──────────────────────────┼──────────────────╡
│ * │ main              │ clean   │ ~/repo                   │ 4d9f1a2 fix bugs │
│   │ feature/login     │ ahead 2 │ ~/repo-feature-login     │ 8c3b7d1 add auth │
└───┴───────────────────┴─────────┴──────────────────────────┴──────────────────┘
```

```bash
$ wt list --json
[
  {
    "path": "/home/dev/repo",
    "name": "main",
    "status": {
      "clean": true,
      "dirty": false,
      "ahead": 0,
      "behind": 0
    },
    ...
  }
]
```

---

## 📦 Installation

### Via Cargo (Recommended)

```bash
cargo install git-worktree-smart
```

This installs the `wt` binary directly to `~/.cargo/bin`.

### From source

```bash
git clone https://github.com/unknownman/git-worktree-smart.git
cd git-worktree-smart
cargo install --path .
```

Or build the release binary locally:

```bash
cargo build --release
# binary produced at target/release/wt
```

### Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.60+ (edition 2021)
- [Git](https://git-scm.com) on your `PATH`

`wt` shells out to the native `git` CLI — no `libgit2` or C system dependencies required.

---

## ⚡ Quick Start

Follow the typical developer workflow (as shown in the demo):

```bash
# 1. View all active worktrees and their sync status:
wt

# 2. Spin up a new worktree for a feature branch (branch created from HEAD):
wt add feature/auth

# 3. Create a worktree branching from a specific base (e.g. main):
wt add hotfix main

# 4. Create a worktree tracking an existing remote branch:
wt add deploy --track origin/deploy

# 5. Jump into a worktree using fuzzy lookup:
cd "$(wt path auth)"

# 6. Or switch instantly using the shell integration:
wt switch auth

# 7. Safely remove a worktree when finished:
wt remove feature/auth

# 8. Clean up stale worktree references from deleted folders:
wt prune        # preview what would be cleaned up (dry run)
wt prune -y     # execute the cleanup
```

---

## 🐚 Shell Integration (`wt switch` / `wt cd`)

Because a child process cannot modify the parent shell's working directory, invoking `wt switch` directly in a terminal will resolve and print the path with instructions.

To enable instant directory navigation directly with `wt switch` or `wt cd`, add this wrapper to your `~/.zshrc`, `~/.bashrc`, or shell profile:

```bash
wt() {
    local has_json=false
    for arg in "$@"; do
        if [ "$arg" = "--json" ]; then has_json=true; fi
    done

    local is_switch=false
    if [ "$1" = "switch" ] || [ "$1" = "cd" ]; then
        is_switch=true
    fi

    if [ "$is_switch" = true ] && [ "$has_json" = false ]; then
        shift
        local target_path
        target_path="$(command wt path "$@")"
        if [ $? -eq 0 ] && [ -n "$target_path" ]; then
            cd -- "$target_path"
        fi
    else
        command wt "$@"
    fi
}
```

### Why this wrapper is robust:
- Seamlessly intercepts both `switch` and `cd` subcommands.
- Inspects arguments for `--json` and safely passes the command through to `command wt` to avoid corrupting machine output.
- Resolves paths via `wt path`, which handles exact, substring, and fuzzy matching with error handling.

---

## 🖥️ Command Reference

### `wt list` (alias: `wt ls`)
Lists all worktrees in the current repository sorted alphabetically.
- Displays an active indicator (`*`), linked branch, status (clean / dirty / ahead / behind), filesystem path, and short commit hash + message.
- Running `wt` with no arguments defaults to `wt list`.

### `wt add <name> [base] [--track <remote>] [--path <custom>]`
Creates a new worktree.
- `<name>`: The branch and worktree name.
- `[base]`: Optional starting commit or branch (defaults to `HEAD`). Ignored if the branch already exists.
- `--track <remote>`: Sets up upstream tracking for an existing remote branch (conflicts with `base`).
- `-p, --path <custom>`: Overrides the inferred sibling path to place the worktree at a custom directory location.

### `wt switch <query>` (alias: `wt cd`)
Resolves a worktree via exact, substring, or fuzzy matching.
- Designed to work in tandem with the shell integration wrapper for instant `cd`.
- Multiple words are combined into a single fuzzy query (e.g. `wt switch feat auth`).

### `wt path <query>`
Resolves a worktree and writes **only** its absolute path to stdout.
- Ideal for scripts and subshells: `cd "$(wt path <query>)"`.
- On failure or ambiguous matches, errors are printed to stderr and the process exits with a non-zero code, preventing subshells from changing into unintended directories.

### `wt remove <query> [-f, --force]` (alias: `wt rm`)
Safely removes a worktree and deletes its directory.
- **Safety guards**: Blocks removal if the worktree contains uncommitted changes, unpushed commits, or unreachable commits on a detached HEAD.
- **Main & active protection**: Refuses to remove the main repository root or the worktree currently occupied by your shell.
- `-f, --force`: Overrides uncommitted/unpushed safeguards to force deletion.

### `wt prune [-y, --yes]`
Cleans up stale worktree references in `.git/worktrees` whose directories were removed manually.
- **Safe dry-run by default**: `wt prune` lists the stale entries without deleting them.
- `-y, --yes`: Executes the prune operation to clean up the index.

---

## ⚙️ Global Options

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | | Output structured JSON on stdout (for scripts, pipelines, and tools) |
| `--verbose` | `-v` | Show underlying `git` commands and arguments executed by `wt` |
| `--help` | `-h` | Print help information |
| `--version` | `-V` | Print version |

---

## 🔧 JSON & Scripting Support

When `--json` is supplied, `wt` guarantees machine-friendly, parseable output:

- **List & Prune commands**: JSON arrays (`[...]`)
- **Single-target operations** (`add`, `switch`, `remove`): JSON objects (`{...}`)
- **Error output**: JSON errors `{"error": "<message>"}` written strictly to **stderr** with non-zero exit codes.

Stdout is kept clean and pipeable into tools like `jq`:

```bash
# Get the paths of all dirty worktrees
wt list --json | jq -r '.[] | select(.status.dirty == true) | .path'
```

---

## 🏗️ Architecture

```
src/
├── main.rs          # Entry point, CLI dispatch, global error formatting
├── cli.rs           # Clap definitions, command routing, shell integration help
├── error.rs         # Strongly-typed AppError (thiserror) with actionable hints
├── models/          # WorktreeInfo & WorktreeStatus domain models (serde + Display)
├── git/
│   ├── command.rs   # Process execution wrappers with verbose tracing
│   ├── ops.rs       # Mutating Git operations (add, remove, prune)
│   ├── parse.rs     # Pure parsers for porcelain worktree & status output
│   └── resolve.rs   # Exact → substring → fuzzy worktree resolution engine
├── cmd/             # Modular subcommand implementations (list, add, switch, path, remove, prune)
└── output/          # Human-friendly tables (comfy-table + owo-colors) & JSON serializers
```

---

## 🧪 Testing & Quality

```bash
cargo test         # Run unit and integration test suite
cargo clippy       # Run linter checks
```

Test coverage includes:
- Parsing `git worktree list --porcelain` across standard, bare, and detached HEAD configurations.
- Dirty, ahead, and behind state detection.
- Prune dry-run parsing across Git versions.
- Worktree path sanitization and collision prevention.
- Exact, substring, and fuzzy matching with ambiguity detection.
- End-to-end integration tests (`assert_cmd`) verifying CLI flags, JSON output, and safety invariants.

---

## 📄 License

Distributed under the [MIT](LICENSE) License.
