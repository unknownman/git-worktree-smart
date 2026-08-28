# VHS Demo Tape for git-worktree-smart (`wt`)
# Generates a high-contrast, documentary-grade demo GIF showcasing the full developer workflow.
# Run with: vhs demo.tape

# --- Configuration & Styling ---
Output demo.gif

Set Shell "zsh"
Set FontSize 16
Set Width 1360
Set Height 780
Set LineHeight 1.25
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

# --- Setup Environment (Hidden from Recording) ---
Hide
Type "export PATH=$PWD/target/release:$PWD/target/debug:$PATH"
Enter
Type "rm -rf /tmp/sample-project*"
Enter
Type "mkdir -p /tmp/sample-project && cd /tmp/sample-project"
Enter
Type "git init -q -b main"
Enter
Type "git config user.name 'Developer' && git config user.email 'dev@example.com'"
Enter
Type "git commit --allow-empty -q -m 'Initial commit on main'"
Enter
Type "clear"
Enter
Show
Sleep 500ms

# ==============================================================================
# Act I: First Impressions & Smart Add
# ==============================================================================
# 1. Zero-config on normal repository: active * on main, clean status, shortened path
Type "wt"
Sleep 400ms
Enter
Sleep 1.8s

# 2. Smart sibling path inference: auto-infers ../sample-project-feature-auth
Type "wt add feature/auth"
Sleep 400ms
Enter
Sleep 1.2s

# 3. Branching from an explicit base commit/branch
Type "wt add hotfix/billing main"
Sleep 400ms
Enter
Sleep 1.2s

# 4. View multi-branch worktree table with colors and commit metadata
Type "wt"
Sleep 400ms
Enter
Sleep 1.8s

# ==============================================================================
# Act II: Rich Status & Commits
# ==============================================================================
# Silently create a commit and uncommitted changes in feature/auth
Hide
Type "cd /tmp/sample-project-feature-auth && git commit --allow-empty -q -m 'Add OAuth2 PKCE handler' && echo 'WIP token logic' > token.rs && cd /tmp/sample-project"
Enter
Type "clear"
Enter
Show
Sleep 400ms

# Visual Highlight: Shows updated commit subject and 'dirty' status badge
Type "wt"
Sleep 400ms
Enter
Sleep 2.5s

# ==============================================================================
# Act III: Fuzzy Resolution, Ambiguity Safety & Shell Integration
# ==============================================================================
# 1. Add a similar branch to demonstrate ambiguity protection
Type "wt add feature/audio"
Sleep 400ms
Enter
Sleep 1s

# 2. Ambiguous query triggers formatted candidate suggestion with tips
Type "wt path au"
Sleep 400ms
Enter
Sleep 2.5s

# 3. Disambiguating returns exact, clean path output for scripting
Type "wt path auth"
Sleep 400ms
Enter
Sleep 1.2s

# 4. Switch command provides educational shell integration wrapper guidance
Type "wt switch auth"
Sleep 400ms
Enter
Sleep 2.2s

# ==============================================================================
# Act IV: Bulletproof Safety Guards on Removal (`wt remove`)
# ==============================================================================
# 1. Safety Guard 1: Cannot remove the main repository root
Type "wt remove main"
Sleep 400ms
Enter
Sleep 2s

# 2. Safety Guard 2: Protects worktrees with uncommitted / dirty state
Type "wt remove auth"
Sleep 400ms
Enter
Sleep 2.5s

# 3. Force override: Safely executes when explicitly requested
Type "wt remove --force auth"
Sleep 400ms
Enter
Sleep 1.5s

# ==============================================================================
# Act V: Stale Detection & Prune Preview (`wt prune`)
# ==============================================================================
# Silently simulate a manually deleted folder outside of git worktree remove
Hide
Type "rm -rf /tmp/sample-project-hotfix-billing"
Enter
Type "clear"
Enter
Show
Sleep 400ms

# Visual Highlight: Deleted directory surfaces immediately as 'stale'
Type "wt"
Sleep 400ms
Enter
Sleep 2s

# Safe dry-run preview: Shows what would be pruned without deleting anything
Type "wt prune"
Sleep 400ms
Enter
Sleep 2.5s

# Execute actual prune with confirmation flag
Type "wt prune -y"
Sleep 400ms
Enter
Sleep 1.5s

# ==============================================================================
# Act VI: Automation & Grand Finale
# ==============================================================================
# Machine-readable JSON output for CLI automation and pipelines
Type "wt --json"
Sleep 400ms
Enter
Sleep 1.8s

# Clean terminal and display final pristine state
Type "clear"
Enter
Sleep 300ms

Type "wt"
Sleep 400ms
Enter
Sleep 4s
