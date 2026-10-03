//! `ltm update`: replace this binary with the latest GitHub release.
//!
//! No HTTP stack: the latest tag comes from the `/releases/latest` redirect
//! (no API, no rate limit, no JSON), and the download, checksum check and
//! atomic swap are done by install.sh -- embedded here so update and the
//! `curl | sh` install are one code path, and so no script is fetched from
//! `main` at update time.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

const INSTALL_SH: &str = include_str!("../../../../install.sh");
const REPO: &str = "samishal1998/lazy-tmux";

pub fn run(check: bool, version: Option<String>, force: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    // LTM_BASE_URL (mirror / local directory) bypasses tag resolution: the
    // install script reads it directly.
    let mirror = std::env::var("LTM_BASE_URL").ok();
    if mirror.is_some() && version.is_some() {
        bail!("--version has no effect with LTM_BASE_URL set (the mirror decides what is served)");
    }

    let tag = match (&version, &mirror) {
        (Some(v), _) => format!("v{}", v.trim_start_matches('v')),
        (None, Some(_)) => String::new(),
        (None, None) => latest_tag()?,
    };

    if !tag.is_empty() {
        let up_to_date = match (parse(&tag), parse(current)) {
            (Some(latest), Some(cur)) => latest <= cur,
            _ => tag.trim_start_matches('v') == current,
        };
        // An explicit --version may downgrade or reinstall; "latest" never does.
        if version.is_none() && up_to_date && !force {
            println!("ltm {current} is up to date (latest release: {tag})");
            return Ok(());
        }
        println!("ltm {current} -> {tag}");
    }
    if check {
        return Ok(());
    }

    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .context("locating the running ltm binary")?;
    let dir = exe.parent().context("ltm binary has no parent directory")?;
    // The canonical file name, so a symlink to a renamed binary updates the
    // file that actually runs rather than a new `ltm` beside it.
    let name = exe.file_name().context("ltm binary has no file name")?;

    let mut sh = Command::new("sh");
    sh.arg("-s")
        .env("LTM_BIN_DIR", dir)
        .env("LTM_BIN_NAME", name)
        .env("LTM_UPDATE", "1")
        .stdin(Stdio::piped());
    if !tag.is_empty() {
        sh.env("LTM_VERSION", &tag);
    }
    let mut child = sh.spawn().context("running sh")?;
    child
        .stdin
        .take()
        .context("opening sh's stdin")?
        .write_all(INSTALL_SH.as_bytes())?;
    if !child.wait()?.success() {
        bail!("update failed (nothing was changed); see the message above");
    }
    println!(
        "tab completions are snapshots: rerun `ltm completions <shell> --install` if you use them"
    );
    Ok(())
}

/// Tag of the newest release, from where /releases/latest redirects to.
fn latest_tag() -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let out = Command::new("curl")
        .args(["-fsSLI", "-o", "/dev/null", "-w", "%{url_effective}", &url])
        .output()
        .context("checking for updates needs curl (or pass --version vX.Y.Z)")?;
    if !out.status.success() {
        bail!(
            "could not reach {url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let effective = String::from_utf8_lossy(&out.stdout);
    match effective.rsplit_once("/tag/") {
        Some((_, tag)) if !tag.trim().is_empty() => Ok(tag.trim().to_string()),
        _ => bail!("no releases published yet at https://github.com/{REPO}/releases"),
    }
}

/// `v1.2.3` / `1.2.3[-pre]` -> (1, 2, 3). Pre-release suffixes are ignored.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>());
    Some((it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn versions_compare_numerically() {
        assert!(parse("v0.10.0") > parse("0.9.9"));
        assert_eq!(parse("v1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse("nightly"), None);
    }
}
