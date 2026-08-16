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
| `ltm sessions` (`s`) | `list`, `new [name] [-c dir] [--detach]`, `attach [name]`, `kill [name] [--yes]`, `rename [from] [to]` |
| `ltm windows` (`w`) `[-s session]` | `list`, `new [name] [-c dir]`, `kill [index\|name]`, `rename [target] [to]`, `select [target]` |
| `ltm panes` (`p`) `[-s session] [-w window]` | `list`, `split [index] [--right]`, `kill [index]` |
| `ltm attach` (`a`) | shortcut for `sessions attach` |
| `ltm context` (`ctx`) | where am I relative to tmux, with the annotated process ancestry |
| `ltm ui` | the TUI (also the default when run with no arguments) |

`windows`/`panes` default to the session/window you are currently in when
run inside tmux, so `ltm w ls` inside tmux just works.

## The TUI

Stacked Sessions / Windows / Panes panels on the left (lazygit-style), a
context header and live pane preview on the right, refreshed every 2s.

Keys: `j/k` move · `tab`/`h/l` or `1/2/3` switch panel · `enter` attach or
switch to the selection · `n` new · `r` rename · `d` kill (with confirm) ·
`s`/`v` split pane below/right · `R` refresh · `?` help · `q` quit.

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
