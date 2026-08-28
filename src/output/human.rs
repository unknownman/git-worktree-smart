use std::path::Path;

use comfy_table::presets::NOTHING;
use comfy_table::{ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::models::WorktreeInfo;

pub fn print_list(worktrees: &[WorktreeInfo], current_path: Option<&Path>) {
    if worktrees.is_empty() {
        println!("No worktrees found.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(["", "Branch", "Status", "Path", "Commit"]);

    for wt in worktrees {
        let is_current = current_path.is_some_and(|p| p == wt.path);

        let indicator = if is_current {
            "*".bold().cyan().to_string()
        } else {
            String::new()
        };

        let branch = format_branch(wt);
        let status = format_status(wt);
        let path = format_path(wt);
        let commit = format_commit(wt);

        table.add_row([indicator, branch, status, path, commit]);
    }

    println!("{table}");
}

fn format_branch(wt: &WorktreeInfo) -> String {
    match &wt.branch {
        Some(b) => b.bold().cyan().to_string(),
        // Detached HEAD: show the short commit so multiple detached worktrees
        // are easy to tell apart.
        None => {
            let short = &wt.head_hash[..wt.head_hash.len().min(7)];
            format!("HEAD ({short})").dimmed().yellow().to_string()
        }
    }
}

fn format_status(wt: &WorktreeInfo) -> String {
    let s = &wt.status;

    // A stale worktree has no usable directory, so ahead/behind/clean all
    // become moot — surface the broken state unambiguously.
    if s.is_stale {
        return "stale".red().bold().to_string();
    }

    let mut parts = Vec::new();

    if s.is_dirty {
        parts.push("dirty".yellow().to_string());
    } else {
        parts.push("clean".green().to_string());
    }

    if s.ahead > 0 {
        parts.push(format!("↑{}", s.ahead).green().to_string());
    }
    if s.behind > 0 {
        parts.push(format!("↓{}", s.behind).red().to_string());
    }

    parts.join(" ")
}

fn format_path(wt: &WorktreeInfo) -> String {
    let display = shorten_home(&wt.path);
    display.dimmed().to_string()
}

/// Normalize path separators to forward slashes for consistent cross-OS display.
///
/// On Windows `Path::display()` uses `\`, which yields ugly mixed separators
/// like `~/Documents\Repo`. Replacing `\` with `/` keeps the terminal UI clean
/// and uniform on every platform.
fn normalize_separators(s: String) -> String {
    if std::path::MAIN_SEPARATOR == '\\' {
        s.replace('\\', "/")
    } else {
        s
    }
}

fn shorten_home(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        // Use `Path::strip_prefix` (component-aware) so a path like
        // `/home/developer/repo` is never mistaken for being under
        // `/home/dev`, which a naive string prefix would wrongly do.
        //
        // Note: `dunce::canonicalize` strips the Windows `\\?\` UNC wrapper at
        // the source, so no manual stripping is needed here.
        if let Ok(rest) = path.strip_prefix(&home) {
            return normalize_separators(format!("~/{}", rest.display()));
        }
    }

    normalize_separators(path.to_string_lossy().into_owned())
}

fn format_commit(wt: &WorktreeInfo) -> String {
    let hash = &wt.head_hash[..wt.head_hash.len().min(7)];
    let msg = truncate(&wt.head_msg, 40);

    format!("{} {msg}", hash.dimmed())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

pub fn print_add_success(info: &WorktreeInfo) {
    let branch = info.display_branch();
    let path = shorten_home(&info.path);

    println!(
        "{} {} for {} at {}",
        "✓".green().bold(),
        "Created worktree".bold(),
        branch.cyan().bold(),
        path.dimmed(),
    );
}

pub fn print_switch_success(info: &WorktreeInfo) {
    let path = shorten_home(&info.path);

    println!(
        "{} {} {}",
        "→".cyan().bold(),
        "Target resolved:".bold(),
        path.cyan(),
    );
    println!(
        "{} {}",
        "💡".dimmed(),
        "A child process cannot change your shell's directory.".dimmed(),
    );
    println!(
        "{} {}",
        "To switch instantly, use:".dimmed(),
        "cd $(wt path <query>)".yellow().bold(),
    );
}

pub fn print_remove_success(info: &WorktreeInfo, forced: bool) {
    let branch = info.display_branch();

    let suffix = if forced {
        " (Forced)".red().bold().to_string()
    } else {
        String::new()
    };

    println!(
        "{} {} {}",
        "🗑️".red().bold(),
        "Successfully removed worktree:".bold(),
        format!("{branch}{suffix}").red(),
    );
}

pub fn print_prune_dry_run(stale: &[String]) {
    if stale.is_empty() {
        println!("{} No stale worktrees to prune.", "✨".green());
        return;
    }

    println!(
        "{} The following stale worktrees would be removed:",
        "⚠️".yellow().bold()
    );
    for path in stale {
        println!(
            "   - {}",
            shorten_home(&std::path::PathBuf::from(path)).yellow()
        );
    }
    println!();
    println!(
        "{} Dry run complete. Run with -y or --yes to actually prune these references.",
        "→".yellow().bold()
    );
}

pub fn print_prune_success(stale: &[String]) {
    if stale.is_empty() {
        println!("{} No stale worktrees to prune.", "✨".green());
        return;
    }

    println!(
        "{} {} {}",
        "🧹".green().bold(),
        "Pruned".bold(),
        format!("{} stale worktree(s)", stale.len()).green(),
    );
    for path in stale {
        println!(
            "   - {}",
            shorten_home(&std::path::PathBuf::from(path)).red()
        );
    }
}
