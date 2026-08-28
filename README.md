<div align="center">

# wt — git-worktree-smart

**A lightweight, zero-config Git worktree manager for humans.**

Beautiful, productive, and safe Git worktree workflows without leaving your standard branch mindset.
No bare repositories. No tedious path typing. Just smarter worktrees.

[![CI](https://github.com/unknownman/git-worktree-smart/actions/workflows/ci.yml/badge.svg)](https://github.com/unknownman/git-worktree-smart/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/git-worktree-smart.svg?color=brightgreen)](https://crates.io/crates/git-worktree-smart)
[![Downloads](https://img.shields.io/crates/d/git-worktree-smart.svg)](https://crates.io/crates/git-worktree-smart)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org)

<br />

<div align="center">
  <img src="demo.gif" alt="wt in action - Git worktrees made human" width="100%" />
</div>

</div>

---

## 💡 Why `wt`?

Git worktrees allow you to check out and work on multiple branches simultaneously across independent directories without touching your main working copy or stashing changes.

However, native `git worktree` commands are tedious and error-prone:
- You have to manually craft and type sibling paths for every new branch.
- Checking out or switching requires exact, absolute paths.
- Deleting a worktree can silently destroy uncommitted changes or unpushed work.
- Listing worktrees produces plain text with minimal branch status context.

`wt` streamlines worktrees into a seamless, modern developer experience:

| Feature | Native `git worktree` | `wt` (`git-worktree-smart`) |
| :--- | :--- | :--- |
| **Setup Required** | Often requires complex bare-repo patterns | **Zero config** — works in any normal Git repository |
| **Directory Paths** | Must be hand-typed manually every time | **Smart inference** — `feature/login` $\rightarrow$ `../repo-feature-login` |
| **Navigation & Search** | Exact paths only | **Fuzzy matching** — `wt path log` resolves `feature/login` |
| **Data Safety** | Easy to lose uncommitted changes on delete | **Strict multi-layer safety guards** against data loss |
| **Status Overview** | Raw index listing | **Rich status table** with dirty/ahead/behind badges |
| **Machine Integration** | Ad-hoc stdout parsing | **Deterministic, structured JSON** for scripts & `jq` |

---

## 📦 Installation

### 1. Via Cargo (Recommended)

```bash
cargo install git-worktree-smart
```

This compiles and installs the `wt` binary into `~/.cargo/bin`. Make sure `~/.cargo/bin` is in your `$PATH`.

### 2. Via Cargo Binstall (Fast Pre-Compiled Binary)

If you have [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) installed, you can skip local compilation and install pre-built release binaries instantly:

```bash
cargo binstall git-worktree-smart
```

### 3. Direct Binary Downloads

Download pre-compiled release binaries for your operating system and architecture directly from the [GitHub Releases](https://github.com/unknownman/git-worktree-smart/releases):

| Platform | Architecture | Binary Package |
| :--- | :--- | :--- |
| **Linux** | `x86_64` (GNU / Glibc) | [`git-worktree-smart-*-x86_64-unknown-linux-gnu.tar.gz`](https://github.com/unknownman/git-worktree-smart/releases) |
| **Linux** | `x86_64` (Static / Musl) | [`git-worktree-smart-*-x86_64-unknown-linux-musl.tar.gz`](https://github.com/unknownman/git-worktree-smart/releases) |
| **Linux** | `aarch64` (ARM64) | [`git-worktree-smart-*-aarch64-unknown-linux-gnu.tar.gz`](https://github.com/unknownman/git-worktree-smart/releases) |
| **macOS** | Apple Silicon (`M1/M2/M3/M4`) | [`git-worktree-smart-*-aarch64-apple-darwin.tar.gz`](https://github.com/unknownman/git-worktree-smart/releases) |
| **macOS** | Intel (`x86_64`) | [`git-worktree-smart-*-x86_64-apple-darwin.tar.gz`](https://github.com/unknownman/git-worktree-smart/releases) |
| **Windows** | `x86_64` (MSVC) | [`git-worktree-smart-*-x86_64-pc-windows-msvc.zip`](https://github.com/unknownman/git-worktree-smart/releases) |

### 4. Build From Source

```bash
git clone https://github.com/unknownman/git-worktree-smart.git
cd git-worktree-smart
cargo build --release
# Binary available at target/release/wt
```

**Requirements**:
- [Rust](https://www.rust-lang.org/tools/install) 1.60+ (2021 edition)
- [Git](https://git-scm.com) on your system `$PATH`

---

## ⚡ Quick Start

```bash
# 1. View all active worktrees and status:
wt

# 2. Create a new worktree for a feature branch (inferred sibling path):
wt add feature/auth

# 3. Create a worktree branching from an explicit base (e.g. main):
wt add hotfix/billing main

# 4. Create a worktree tracking an existing remote branch:
wt add deploy --track origin/deploy

# 5. Jump into a worktree using fuzzy lookup:
cd "$(wt path auth)"

# 6. Or switch instantly using the shell integration:
wt switch auth

# 7. Safely remove a worktree when finished:
wt remove feature/auth

# 8. Clean up stale worktree references from deleted folders:
wt prune        # Safe preview (dry-run)
wt prune -y     # Execute cleanup
```

---

## 🐚 Seamless Shell Integration (`wt switch` / `wt cd`)

Because child processes cannot modify the parent shell's working directory, invoking `wt switch` or `wt cd` natively will resolve and display the path with copy-paste instructions.

To enable instant in-terminal directory navigation with `wt switch` and `wt cd`, add this wrapper to your shell configuration (`~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish`):

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
- **Native Navigation**: Intercepts `switch` and `cd` subcommands and seamlessly performs `cd -- "$target_path"`.
- **JSON Safe**: Detects `--json` anywhere in arguments and passes directly through to `command wt` without breaking machine output.
- **Fuzzy Resolution**: Delegates to `wt path`, supporting multi-word fuzzy matching, substring resolution, and error bubbling.

---

## 🖥️ Command Reference

### `wt` / `wt list` (alias: `wt ls`)
Lists all worktrees in the current Git repository formatted into an aligned, colorized table.
- Highlights the currently active worktree with `*`.
- Displays branch name, sync status (`clean`, `dirty`, `ahead N`, `behind N`, `stale`), shortened filesystem path, and the latest commit hash + subject.
- Running `wt` without subcommands defaults to `wt list`.

```text
$ wt
┌───┬───────────────────┬───────────┬──────────────────────────┬─────────────────────────────┐
│   │ Branch            │ Status    │ Path                     │ Commit                      │
├───┼───────────────────┼───────────┼──────────────────────────┼─────────────────────────────╡
│ * │ main              │ clean     │ ~/project                │ 4d9f1a2 Initial commit      │
│   │ feature/auth      │ dirty, ↑1 │ ~/project-feature-auth   │ 8c3b7d1 Add OAuth2 PKCE     │
│   │ hotfix/billing    │ clean     │ ~/project-hotfix-billing │ 4d9f1a2 Initial commit      │
└───┴───────────────────┴───────────┴──────────────────────────┴─────────────────────────────┘
```

### `wt add <name> [base] [--track <remote>] [-p, --path <custom>]`
Creates and registers a new worktree.
- **Automatic Sibling Path**: Infers `<parent>/<repo-name>-<sanitized-branch>`. For example, `feature/auth` becomes `../myrepo-feature-auth`.
- `<name>`: The branch and worktree identifier.
- `[base]`: Optional starting commit or branch (defaults to `HEAD`). Ignored if the branch already exists.
- `--track <remote>`: Sets up upstream tracking for an existing remote branch (e.g. `--track origin/deploy`).
- `-p, --path <custom>`: Overrides the inferred directory with a custom path.

### `wt path <query>`
Resolves a target worktree via exact $\rightarrow$ substring $\rightarrow$ fuzzy resolution and writes **only** its absolute path to stdout.
- Ideal for shell scripting and subshells: `cd "$(wt path <query>)"`.
- On ambiguous matches or errors, formatted messages are written to **stderr** with non-zero exit codes to prevent subshells from changing into unintended directories.

### `wt switch <query>` (alias: `wt cd`)
Resolves a worktree using fuzzy matching.
- Pairs with the shell integration wrapper for instantaneous directory jumping.
- Accepts multiple query tokens: `wt switch feat auth`.

### `wt remove <query> [-f, --force]` (alias: `wt rm`)
Safely removes a worktree and deletes its associated directory.

**Built-in Safety Protections**:
- 🛡️ **Dirty State Guard**: Refuses to remove worktrees with uncommitted modified or untracked files.
- 🛡️ **Unpushed Commits Guard**: Refuses to remove worktrees with commits ahead of upstream.
- 🛡️ **Detached HEAD Guard**: Protects detached worktrees containing commits unreachable from other branches.
- 🛡️ **Root & Active Worktree Protection**: Refuses to remove the main repository root or the worktree your current shell is standing in.
- `-f, --force`: Overrides uncommitted/unpushed safeguards when intentional data deletion is desired.

### `wt prune [-y, --yes]`
Cleans up stale worktree index references in `.git/worktrees` whose directories were removed outside of `wt`.
- **Safe Dry-Run by Default**: `wt prune` lists the stale entries without deleting them.
- `-y, --yes`: Executes the prune operation and purges stale references.

---

## ⚙️ Global Options

| Option | Short | Description |
| :--- | :--- | :--- |
| `--json` | | Output strictly structured JSON for machine consumption |
| `--verbose` | `-v` | Show underlying `git` commands and arguments executed by `wt` |
| `--help` | `-h` | Print help information |
| `--version` | `-V` | Print version |

---

## 🔧 Scripting & Automation (`--json`)

When `--json` is supplied, `wt` guarantees machine-friendly, parseable JSON:

- **List & Prune commands**: JSON arrays (`[...]`)
- **Single-target operations** (`add`, `switch`, `remove`): JSON objects (`{...}`)
- **Error output**: Structured JSON `{"error": "<message>"}` written exclusively to **stderr** with non-zero exit codes.

### Examples with `jq`:

```bash
# Get the absolute paths of all dirty worktrees:
wt list --json | jq -r '.[] | select(.status.dirty == true) | .path'

# Find all worktrees ahead of their remote:
wt list --json | jq -r '.[] | select(.status.ahead > 0) | "\(.name): ahead by \(.status.ahead)"'

# Inspect JSON output when adding a new worktree:
wt add feature/api --json | jq '.path'
```

---

## 🏗️ Architecture

`wt` is architected around lightweight, tested, and pure parsing pipelines wrapped around native Git CLI commands:

```
src/
├── main.rs          # CLI entry point, error handling, dispatch
├── cli.rs           # Clap definitions, command routing, shell integration help
├── error.rs         # Strongly-typed AppError (thiserror) with actionable hints
├── models/          # WorktreeInfo & WorktreeStatus domain models (serde + Display)
├── git/
│   ├── command.rs   # Process execution wrappers with verbose tracing
│   ├── ops.rs       # Mutating operations (add, remove, prune)
│   ├── parse.rs     # Pure parsers for porcelain worktree & status output
│   └── resolve.rs   # Exact → substring → fuzzy resolution engine
├── cmd/             # Subcommand implementations (list, add, switch, path, remove, prune)
└── output/          # Human table renderers (comfy-table + owo-colors) & JSON serializers
```

---

## 🧪 Development & Testing

```bash
# Run unit and integration tests:
cargo test

# Run strict linter:
cargo clippy --all-targets --all-features -- -D warnings

# Check formatting:
cargo fmt --check
```

---

## 📄 License

Distributed under the [MIT](LICENSE) License.
