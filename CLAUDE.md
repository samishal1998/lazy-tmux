# CLAUDE.md — project context for lazy-tmux

Working notes for AI/dev sessions on this repo. The README is the
user-facing doc; this file records how the project is built, why it is
shaped this way, and the tmux gotchas already paid for.

## What this is

`lazy-tmux` wraps tmux with modern noun-verb subcommands, interactive
prompts, a lazygit-style ratatui TUI, user macros, and tmux-health
tooling. Single installed binary: **`ltm`** (from `crates/cli`).

- Repo: https://github.com/samishal1998/lazy-tmux (branch `main`)
- Install: `cargo install --git https://github.com/samishal1998/lazy-tmux lazytmux-cli`
- Toolchain: stable Rust (2021 edition), tmux 3.4 on Linux is the tested target.

## Workspace layout & dependency rules

```
crates/core  lazytmux-core  — tmux wrapper, models, context detection,
                             attach planning, macro engine. NO UI deps.
crates/tui   lazytmux-tui   — ratatui app. Depends on core only.
crates/cli   lazytmux-cli   — bin `ltm`: clap args, inquire prompts,
                             output tables, attach executor, doctor.
```

Hard rules:
- Nothing touches tmux except through `core::Tmux`.
- The TUI never execs or switches clients; it **returns** the chosen
  target from `lazytmux_tui::run()` and the CLI acts after the terminal
  is restored (`crates/cli/src/exec.rs`).
- There is deliberately **no daemon**: the tmux server already owns all
  state and queries are cheap. If one ever earns its place (event
  watching/notifications), it slots in as `crates/daemon` on top of core.

## Design decisions (and why)

- **Target ids, never names.** All mutations target tmux ids (`$n`
  session, `@n` window, `%n` pane); window targets are `"$sid:index"`
  (`windows::target_of`). Names can contain `:` and `.` which break
  target parsing.
- **Field separator U+241F `␟`** (`core::model::SEP`). tmux *escapes
  control characters* in format output — a real 0x1F becomes the literal
  text `\037` — so the delimiter must be printable. Tab also survives
  but can appear in names; `␟` effectively cannot.
- **Context detection** (`core/src/context.rs`) cross-checks `$TMUX`
  against the **/proc ancestry**, because `$TMUX` is inherited (nvim
  terminals, GUI terminals launched from tmux) and can go stale. The
  chain is matched against `pane_pid` of every pane on the server.
  Outcomes: `Inside{location: DirectPane | NestedDescendant{via} |
  EnvInherited}`, `StaleEnv`, `Outside{server_running}`. Shells between
  self and the pane don't count as nesting (`SHELLS` list); consecutive
  duplicate programs are collapsed (nvim spawns an embedded nvim).
  Works with a scrubbed env too (ancestry alone). Non-Linux: no /proc →
  trusts `$TMUX` and assumes DirectPane. Unit tests cover the
  classification (`cargo test -p lazytmux-core`).
- **Attach planning** (`core/src/attach.rs`): inside tmux →
  `switch-client` (no nesting warning); `EnvInherited`/`StaleEnv` →
  exec attach with `TMUX`/`TMUX_PANE` env removed (tmux refuses to nest
  otherwise); outside → plain exec attach. When env is absent but we're
  inside (process-tree detection), the client tty is resolved via
  `list-clients` and passed as `switch-client -c`. The CLI executor
  falls back from `switch-client` to exec attach on the "no current
  client" error (happens for send-keys-driven/headless panes).
- **Options toggling**: `set-option` with **no value toggles** on/off
  options (verified on tmux 3.4) — that's how `ltm options mouse`
  toggles and reads back the new state.
- **Doctor** (`cli/src/commands/doctor.rs`): a static `CHECKS` table of
  option, table-selection flags (`-s` server / `-g` global / `-g -w`),
  predicate, fix value (append `-a` for array options like
  `terminal-features`), tmux.conf line, and rationale. Key pair for
  Shift-Enter/Ctrl-Enter reaching apps (e.g. Claude Code):
  `extended-keys on` **plus** `terminal-features "*:extkeys"`. `mouse`
  is checked but `auto_fix: false` — taste, never auto-applied.
  `--fix` mutates only the running server; `--conf` prints a snippet.
- **Macros** (`core/src/macros.rs`): TOML at
  `~/.config/lazy-tmux/macros.toml` (`$LAZY_TMUX_MACROS` override,
  XDG-aware). Steps are tmux commands without the `tmux` prefix, split
  with `shlex` (quoted args survive), leading `~/` expanded. Fail-fast
  with step number + tmux error. Optional `attach = "session"` runs
  through the normal attach planner. Engine is in core so the TUI can
  adopt it later.
- **Completions**: generated at runtime from the clap tree, so they are
  always in sync with the binary. `--install` writes to the standard
  per-shell path (bash: XDG data dir; fish: XDG config dir; zsh:
  detects oh-my-zsh → `$ZSH_CUSTOM/completions/_ltm` which is already
  on fpath, else `~/.zfunc` + fpath hint). Installed files are
  snapshots — rerun `--install` after upgrades. elvish/powershell have
  no standard dir → error with stdout-redirect hint.
- **SIGPIPE**: Rust ignores it, so `ltm ... | head` used to panic; main
  restores `SIG_DFL` first thing (`libc`).
- **TUI structure**: `app.rs` = state + update (no drawing), `ui.rs` =
  drawing (no mutation), `lib.rs` = terminal lifecycle/event loop with
  a panic hook restoring the terminal. Modals are one enum: `Input`,
  `Confirm`, `Pick` (generic (id,label) list — used for move-window),
  `Help`. Data auto-refreshes every 2s.
- **TUI self-preview guard**: previewing the pane the TUI runs in would
  capture itself (hall of mirrors); `App::is_own_pane` (via the ctx
  pane id, excluding `EnvInherited`) shows a "you are here" placeholder
  instead.
- **Interactive layers** — three, deliberately: prompts fill any omitted
  CLI arg (inquire; Esc = cancel, exit code 130; single candidate
  auto-picked); `ltm interactive` is a pure menu mode (Esc goes back a
  level); the TUI is the visual manager. All three call the same
  command fns with `None` args.

## Testing recipes (headless, no real terminal)

Drive everything through a detached tmux session:

```sh
tmux new-session -d -s t -x 120 -y 30       # fixed size for stable captures
tmux send-keys -t t "target/debug/ltm" Enter
sleep 1.5
tmux capture-pane -p -t t                    # "screenshot" of the TUI
tmux send-keys -t t "3" ; tmux send-keys -t t "s"   # drive keys
tmux kill-session -t t                       # ALWAYS clean up
```

Gotchas learned the hard way:
- Unix socket paths are limited to ~108 bytes — the session scratchpad
  path is too long for `tmux -S`. Use a short path like `/tmp/ltm-x-$$`
  for isolated test servers.
- Test `doctor --fix` / option mutations against an **isolated server**
  (`tmux -S <short-sock> new-session -d`, then run ltm with
  `TMUX="<sock>,1,0"`), never the user's real server.
- `switch-client` from a send-keys-driven pane fails with "no current
  client" (no attached client) — expected; the exec-attach fallback
  covers it.
- Watch for state leakage into real sessions: `windows move` tests once
  dropped a window into the user's session `0`. Verify and clean up.
- The interactive prompts and TUI need a tty; piping stdin skips
  prompts (`interactive::is_interactive`).

Quality bar before committing: `cargo clippy --workspace --all-targets`
clean, `cargo test --workspace` green, and exercise changed CLI verbs
against a scratch session as above.

## Conventions

- **Commits: plain author only — no Co-Authored-By / AI trailers**
  (owner's explicit request). Author `samishal1998 <samishal.1998@gmail.com>`.
- Push to `main` on GitHub `samishal1998/lazy-tmux` (gh CLI is
  authenticated on this machine).
- Keep the README command table + TUI key list in sync with new verbs;
  completions need no repo update (runtime-generated).
- New subcommand checklist: core method → args enum variant → command fn
  (with interactive fallback for omitted args) → main dispatch → menu
  entry → TUI key if it fits → README row.
- Workspace deps/versions live in the root `Cargo.toml`
  `[workspace.dependencies]`; crates use `.workspace = true`.

## State / environment notes

- Owner's interactive shell: zsh via oh-my-zsh (login shell bash);
  completions installed for bash, zsh (`~/.oh-my-zsh/custom/completions/_ltm`),
  and fish on this machine.
- The repo lives under `~/projects/go/` despite being Rust — historical,
  don't "fix" it.

## Ideas parked, not committed to

- `crates/daemon` for session-event notifications (see "no daemon" above).
- TUI: run macros from a Pick modal; join-pane flow via a marked pane.
- Session "reorder" is meaningless in tmux (sessions are sorted) — `[`/`]`
  on the sessions panel intentionally reports that instead of acting.
