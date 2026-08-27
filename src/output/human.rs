use std::path::Path;

use comfy_table::presets::UTF8_FULL_CONDENSED;
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
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(["", "Branch", "Status", "Path", "Commit"]);

    for wt in worktrees {
        let is_current = current_path.map_or(false, |p| p == wt.path);

        let indicator = if is_current {
            "*".bold().cyan().to_string()
        } else {
            String::new()
        };

        let branch = format_branch(&wt);
        let status = format_status(&wt);
        let path = format_path(&wt);
        let commit = format_commit(&wt);

        table.add_row([indicator, branch, status, path, commit]);
    }

    println!("{table}");
}

fn format_branch(wt: &WorktreeInfo) -> String {
    match &wt.branch {
        Some(b) => b.bold().cyan().to_string(),
        None => "HEAD".dimmed().yellow().to_string(),
    }
}

fn format_status(wt: &WorktreeInfo) -> String {
    let s = &wt.status;
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

fn shorten_home(path: &Path) -> String {
    let s = path.to_string_lossy();

    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if let Some(rest) = s.strip_prefix(home_str.as_ref()) {
            return format!("~{rest}");
        }
    }

    s.into_owned()
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

    eprintln!(
        "{} {} for {} at {}",
        "✓".green().bold(),
        "Created worktree".bold(),
        branch.cyan().bold(),
        path.dimmed(),
    );
}

pub fn print_switch_success(info: &WorktreeInfo) {
    let path = shorten_home(&info.path);

    eprintln!(
        "{} {} {}",
        "→".cyan().bold(),
        "Target resolved:".bold(),
        path.cyan(),
    );
    eprintln!(
        "{} {}",
        "💡".dimmed(),
        "A child process cannot change your shell's directory.".dimmed(),
    );
    eprintln!(
        "{} {}",
        "To switch instantly, use:".dimmed(),
        "cd $(wt path <query>)".yellow().bold(),
    );
    eprintln!();
    eprintln!(
        "{}",
        "Pro-tip: Add this to your ~/.zshrc or ~/.bashrc:".dimmed()
    );
    eprintln!(
        "  {}",
        r#"wt() {
    if [ "$1" = "switch" ] || [ "$1" = "cd" ]; then
        local target_path
        target_path="$(command wt path "${@:2}")"
        if [ $? -eq 0 ] && [ -n "$target_path" ]; then
            cd "$target_path"
        fi
    else
        command wt "$@"
    fi
}"#
        .dimmed()
    );
}

pub fn print_remove_success(info: &WorktreeInfo, forced: bool) {
    let branch = info.display_branch();

    let suffix = if forced {
        " (Forced)".red().bold().to_string()
    } else {
        String::new()
    };

    eprintln!(
        "{} {} {}",
        "🗑️".red().bold(),
        "Successfully removed worktree:".bold(),
        format!("{branch}{suffix}").red(),
    );
}

pub fn print_prune_dry_run(stale: &[String]) {
    if stale.is_empty() {
        eprintln!("{} No stale worktrees to prune.", "✨".green());
        return;
    }

    eprintln!(
        "{} The following stale worktrees would be removed:",
        "⚠️".yellow().bold()
    );
    for path in stale {
        eprintln!(
            "   - {}",
            shorten_home(&std::path::PathBuf::from(path)).yellow()
        );
    }
    eprintln!();
    eprintln!(
        "{} Dry run complete. Run with -y or --yes to actually prune these references.",
        "→".yellow().bold()
    );
}

pub fn print_prune_success(stale: &[String]) {
    if stale.is_empty() {
        eprintln!("{} No stale worktrees to prune.", "✨".green());
        return;
    }

    eprintln!(
        "{} {} {}",
        "🧹".green().bold(),
        "Pruned".bold(),
        format!("{} stale worktree(s)", stale.len()).green(),
    );
    for path in stale {
        eprintln!(
            "   - {}",
            shorten_home(&std::path::PathBuf::from(path)).red()
        );
    }
}
