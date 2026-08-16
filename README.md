# lazy-tmux (`ltm`)

A friendlier face for tmux: modern noun-verb subcommands, interactive
pickers when you leave arguments out, and a lazygit-style TUI session
manager — all aware of *where* you are relative to tmux (directly in a
pane, nested inside an nvim terminal, or outside entirely).

```
ltm                      # open the TUI session manager
ltm sessions list        # instead of tmux list-sessions
ltm windows new -s work  # instead of tmux new-window -t work
ltm attach               # interactive picker, then attach/switch
ltm context              # show exactly where you are relative to tmux
```

## Install

```sh
cargo install --path crates/cli
```

The binary is `ltm`. Requires tmux (tested with 3.4) and Linux
(`/proc`-based process-tree detection; elsewhere it degrades to
`$TMUX`-only detection).

## Commands

Nouns take a verb; every verb prompts interactively for whatever you leave
out (single candidates are picked automatically, and everything has short
aliases: `ltm s ls`, `ltm w n`, `ltm a`).

| Command | Verbs |
|---|---|
| `ltm sessions` (`s`) | `list`, `new [name] [-c dir] [--detach]`, `attach [name]`, `kill [name] [--yes]`, `rename [from] [to]`, `detach [name]` |
| `ltm windows` (`w`) `[-s session]` | `list`, `new [name] [-c dir]`, `kill [target]`, `rename [target] [to]`, `select [target]`, `swap [a] [b]`, `move [target] [--to session]`, `layout [target]` |
| `ltm panes` (`p`) `[-s session] [-w window]` | `list`, `split [--right]`, `kill [index]`, `resize <up\|down\|left\|right> [n]`, `zoom [index]`, `swap [a] [b]`, `rename [index] [title]`, `break [index]`, `select [index]`, `join [index] [--to window] [--right]` |
| `ltm options` (`o`) | `list`, `mouse [on\|off]`, `status [on\|off]`, `sync [on\|off]`, `get <name>`, `set <name> <value> [--window]` — no value toggles |
| `ltm macros` (`m`) | `list`, `run [name]`, `show [name]`, `edit` — user-defined tmux command sequences |
| `ltm doctor` | check common paper-cuts; `--fix` applies live, `--conf` prints a ~/.tmux.conf snippet |
| `ltm attach` (`a`) | shortcut for `sessions attach` |
| `ltm kill-server` | kill the server and every session (with confirm) |
| `ltm context` (`ctx`) | where am I relative to tmux, with the annotated process ancestry |
| `ltm interactive` (`i`) | guided menu mode — no commands to remember, Esc goes back a level |
| `ltm completions <shell>` | generate shell completions (bash, zsh, fish, elvish, powershell) |
| `ltm ui` | the TUI (also the default when run with no arguments) |

`windows`/`panes` default to the session/window you are currently in when
run inside tmux, so `ltm w ls` inside tmux just works.

## Shell completions

`ltm completions <shell>` prints a completion script to stdout for `bash`,
`zsh`, `fish`, `elvish`, or `powershell`. Completions cover every
subcommand, alias, and flag; regenerate the file after upgrading `ltm` so
new commands show up.

### bash

Requires the `bash-completion` package (preinstalled on most distros).

```sh
mkdir -p ~/.local/share/bash-completion/completions
ltm completions bash > ~/.local/share/bash-completion/completions/ltm
```

Reopen the shell, or try it in the current one without installing:

```sh
source <(ltm completions bash)
```

### zsh

Put the script in a directory that is in `$fpath` **before** `compinit`
runs:

```sh
mkdir -p ~/.zfunc
ltm completions zsh > ~/.zfunc/_ltm
```

Then in `~/.zshrc` (the `fpath` line must come before `compinit`):

```sh
fpath=(~/.zfunc $fpath)
autoload -Uz compinit && compinit
```

If completions don't appear, rebuild the completion cache:
`rm -f ~/.zcompdump && compinit`.

### fish

fish auto-loads anything in this directory — no config changes needed:

```sh
ltm completions fish > ~/.config/fish/completions/ltm.fish
```

### elvish / powershell

```sh
ltm completions elvish     # add to ~/.config/elvish/rc.elv via eval
ltm completions powershell # dot-source from $PROFILE
```

## The TUI

Stacked Sessions / Windows / Panes panels on the left (lazygit-style), a
context header and live pane preview on the right, refreshed every 2s.

Keys: `j/k` move · `tab`/`h/l` or `1/2/3` switch panel · `enter` attach or
switch to the selection · `n` new · `r` rename (session / window / pane
title) · `d` kill (with confirm) · `D` detach clients · `s`/`v` split pane
below/right · `z` zoom · `H/J/K/L` resize pane · `[`/`]` swap pane or
window with its neighbour · `b` break pane into a window · `m` move window
to another session · `o` cycle window layout · `M` toggle mouse · `R`
refresh · `?` help · `q` quit.

## Macros

Named tmux command sequences, defined in
`~/.config/lazy-tmux/macros.toml` (override with `$LAZY_TMUX_MACROS`).
`ltm macros edit` creates a starter file and opens `$EDITOR`.

```toml
[macros.dev]
description = "Editor + server layout"
steps = [
  "new-session -d -s dev -c ~/projects/app",
  "rename-window -t dev: editor",
  "send-keys -t dev: 'nvim .' Enter",
  "split-window -h -t dev:",
  "new-window -t dev -n server",
  "send-keys -t dev:server 'npm run dev' Enter",
]
attach = "dev"   # optional: attach/switch there afterwards
```

Steps are tmux commands without the `tmux` prefix, split shell-style (so
quoted arguments work) with `~/` expanded. `ltm macros run` with no name
gives a picker; a failing step aborts and reports which step and why.

## Doctor

`ltm doctor` checks the running server for the classic paper-cuts and says
why each one matters:

- **extended-keys** + **terminal-features extkeys** — without these,
  Shift-Enter / Ctrl-Enter never reach apps like Claude Code
- **true color** (`terminal-features RGB`), **default-terminal** — washed
  out colors, broken keys/italics
- **escape-time** — the 500ms default Esc delay that makes vim feel laggy
- **focus-events**, **set-clipboard** (OSC 52), **allow-passthrough**,
  **history-limit**, and **mouse** (flagged but never auto-applied — taste)

`ltm doctor --fix` applies the recommendations to the running server;
`ltm doctor --conf >> ~/.tmux.conf` makes them permanent.

## Context detection

`$TMUX` alone lies: it is inherited by everything a pane spawns, survives
into GUI terminals launched from tmux, and can go stale. So `ltm`
cross-checks the environment against the **process tree**: it walks the
ancestor chain in `/proc` and looks for the root process of any pane on
the server.

- **direct** — your ancestors reach a pane's shell with nothing but shells
  in between;
- **nested** — the chain passes through other programs first (e.g. an nvim
  `:terminal` inside tmux → "nested via nvim");
- **env inherited** — `$TMUX` is set and the pane exists, but this
  terminal is not actually displayed inside tmux;
- **stale env** — `$TMUX` points at a dead server or pane;
- **outside** — no relation at all (works even with a scrubbed
  environment: the process tree alone is enough to detect being inside).

Attaching adapts accordingly: inside tmux it becomes `switch-client`
(no nesting warnings), outside it `exec`s `tmux attach`, and inherited or
stale `$TMUX` values are shed first so tmux does not refuse to nest.
`ltm context` shows the verdict plus the ancestry chain that produced it.

## Architecture

```
crates/
  core/   lazytmux-core  — tmux wrapper, models, context detection, attach planning
  tui/    lazytmux-tui   — ratatui app; returns the chosen target, never execs
  cli/    lazytmux-cli   — the `ltm` binary: clap args, inquire prompts, output,
                           and the attach executor (switch-client or exec)
```

- **core** is UI-free. Queries use tmux format strings with a printable
  unit-separator delimiter (tmux escapes real control characters), so names
  containing spaces/colons round-trip safely; mutations target tmux ids
  (`$n`, `@n`, `%n`), never names.
- **tui** owns terminal state but performs no process replacement: it
  returns the target the user picked, and the CLI attaches *after* the
  terminal is restored.
- **Why no daemon?** The tmux server *is* the daemon — it already owns all
  session state, and every query here is a cheap direct call to it, so a
  second stateful process would only add a cache to keep coherent. If one
  ever earns its place (e.g. watching for session events to push
  notifications), it slots in as `crates/daemon` on top of `core` without
  touching the CLI or TUI.
