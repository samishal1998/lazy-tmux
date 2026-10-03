//! `ltm doctor`: check the running tmux server for the classic paper-cuts —
//! Shift-Enter not reaching apps, washed-out colors, laggy Esc in editors —
//! and optionally fix them live or emit a ~/.tmux.conf snippet.

use anyhow::Result;
use lazytmux_core::Tmux;

struct Check {
    /// Short label for the report (options can have several checks).
    label: &'static str,
    option: &'static str,
    /// Flags selecting the option table (server `-s`, global `-g`, …).
    flags: &'static [&'static str],
    ok: fn(&str) -> bool,
    /// Value to set; appended (`-a`) for array options like terminal-features.
    value: &'static str,
    append: bool,
    conf: &'static str,
    why: &'static str,
    /// `--fix` applies this automatically; taste options (mouse) are opt-in.
    auto_fix: bool,
}

fn first_is_on(v: &str) -> bool {
    matches!(
        v.lines().next().unwrap_or(""),
        "on" | "always" | "external" | "all"
    )
}

fn contains_extkeys(v: &str) -> bool {
    v.contains("extkeys")
}

fn contains_rgb(v: &str) -> bool {
    v.contains("RGB") || v.contains("Tc")
}

fn contains_256color(v: &str) -> bool {
    v.contains("256color")
}

fn escape_time_ok(v: &str) -> bool {
    v.trim().parse::<u32>().is_ok_and(|n| n <= 50)
}

fn history_ok(v: &str) -> bool {
    v.trim().parse::<u64>().is_ok_and(|n| n >= 10_000)
}

const CHECKS: &[Check] = &[
    Check {
        label: "extended-keys",
        option: "extended-keys",
        flags: &["-s"],
        ok: first_is_on,
        value: "on",
        append: false,
        conf: "set -s extended-keys on",
        why: "Shift-Enter / Ctrl-Enter reach apps (Claude Code, kitty-protocol apps)",
        auto_fix: true,
    },
    Check {
        label: "extkeys feature",
        option: "terminal-features",
        flags: &["-s"],
        ok: contains_extkeys,
        value: "*:extkeys",
        append: true,
        conf: "set -as terminal-features \"*:extkeys\"",
        why: "advertise extended-key support for every outer terminal",
        auto_fix: true,
    },
    Check {
        label: "true color",
        option: "terminal-features",
        flags: &["-s"],
        ok: contains_rgb,
        value: "*:RGB",
        append: true,
        conf: "set -as terminal-features \"*:RGB\"",
        why: "24-bit color instead of washed-out 256-color approximations",
        auto_fix: true,
    },
    Check {
        label: "default-terminal",
        option: "default-terminal",
        flags: &["-s"],
        ok: contains_256color,
        value: "tmux-256color",
        append: false,
        conf: "set -s default-terminal tmux-256color",
        why: "correct terminfo: keys, italics, and colors inside tmux",
        auto_fix: true,
    },
    Check {
        label: "escape-time",
        option: "escape-time",
        flags: &["-s"],
        ok: escape_time_ok,
        value: "10",
        append: false,
        conf: "set -s escape-time 10",
        why: "the default 500ms Esc delay makes vim/helix feel laggy",
        auto_fix: true,
    },
    Check {
        label: "focus-events",
        option: "focus-events",
        flags: &["-s"],
        ok: first_is_on,
        value: "on",
        append: false,
        conf: "set -s focus-events on",
        why: "apps see focus in/out (vim autoread, prompt refreshes)",
        auto_fix: true,
    },
    Check {
        label: "clipboard (OSC 52)",
        option: "set-clipboard",
        flags: &["-s"],
        ok: first_is_on,
        value: "on",
        append: false,
        conf: "set -s set-clipboard on",
        why: "copy from tmux/apps into the system clipboard",
        auto_fix: true,
    },
    Check {
        label: "passthrough",
        option: "allow-passthrough",
        flags: &["-g", "-w"],
        ok: first_is_on,
        value: "on",
        append: false,
        conf: "set -gw allow-passthrough on",
        why: "escape-sequence passthrough (inline images and friends)",
        auto_fix: true,
    },
    Check {
        label: "history-limit",
        option: "history-limit",
        flags: &["-g"],
        ok: history_ok,
        value: "50000",
        append: false,
        conf: "set -g history-limit 50000",
        why: "the default 2000-line scrollback is tiny",
        auto_fix: true,
    },
    Check {
        label: "mouse",
        option: "mouse",
        flags: &["-g"],
        ok: first_is_on,
        value: "on",
        append: false,
        conf: "# set -g mouse on",
        why: "scroll, click panes, drag borders (a matter of taste)",
        auto_fix: false,
    },
];

fn read(tmux: &Tmux, check: &Check) -> String {
    let mut args: Vec<&str> = vec!["show-options"];
    args.extend(check.flags);
    args.push("-v");
    args.push(check.option);
    tmux.run(args).unwrap_or_default().trim().to_string()
}

fn apply(tmux: &Tmux, check: &Check) -> lazytmux_core::Result<()> {
    let mut args: Vec<&str> = vec!["set-option"];
    args.extend(check.flags);
    if check.append {
        args.push("-a");
    }
    args.push(check.option);
    args.push(check.value);
    tmux.run(args).map(|_| ())
}

pub fn run(tmux: &Tmux, fix: bool, conf: bool) -> Result<()> {
    if conf {
        println!("# lazy-tmux recommended settings (generated by `ltm doctor --conf`)");
        for check in CHECKS {
            println!("{}   # {}", check.conf, check.why);
        }
        return Ok(());
    }

    let mut failing: Vec<&Check> = Vec::new();
    println!("tmux doctor\n");
    for check in CHECKS {
        let value = read(tmux, check);
        let mut display: String = value
            .lines()
            .next()
            .unwrap_or("(unset)")
            .chars()
            .take(14)
            .collect();
        if display.len() < value.lines().next().unwrap_or("").len() {
            display.pop();
            display.push('…');
        }
        if (check.ok)(&value) {
            println!("  ok  {:<18} {:<14} {}", check.label, display, check.why);
        } else {
            println!("  !!  {:<18} {:<14} {}", check.label, display, check.why);
            failing.push(check);
        }
    }

    if failing.is_empty() {
        println!("\nall good — nothing to fix");
        return Ok(());
    }

    if fix {
        println!();
        for check in failing.iter().filter(|c| c.auto_fix) {
            match apply(tmux, check) {
                Ok(()) => println!("  fixed {} -> {}", check.label, check.value),
                Err(e) => println!("  failed to fix {}: {e}", check.label),
            }
        }
        for check in failing.iter().filter(|c| !c.auto_fix) {
            println!(
                "  skipped {} (taste — enable with `ltm options {}`)",
                check.label, check.option
            );
        }
        println!(
            "\napplied to the running server only — persist with:\n  ltm doctor --conf >> ~/.tmux.conf"
        );
    } else {
        println!(
            "\n{} issue(s) — apply live with `ltm doctor --fix`, or persist with:\n  ltm doctor --conf >> ~/.tmux.conf",
            failing.len()
        );
    }
    Ok(())
}
