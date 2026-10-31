//! **The crate boundary, enforced.**
//!
//! `bevy_speech_bubbles` is a dialog kit: balloons, a conversation graph and a screen dialog box. It
//! takes `bevy` (a trimmed feature set), `ab_glyph` for glyph rasterization and `serde` for the
//! conversation model — and it must never learn anything about a particular game. (`ron` is a
//! dev-dependency of the tests, outside the table this scan reads.)
//!
//! The source scan below forbids more than crate names: `MainCamera`, `SquadMember` and `MenuState` are
//! there because each is a specific temptation this crate already faced. It USED to name a camera
//! marker from the game it was extracted from, and the fix — making the tracking system generic over
//! the marker — is what stops it silently breaking in any project with a second 3D camera. Naming one
//! again would undo that, and it would compile.
//!
//! The crate ships ONE plugin, `SpeechBubblesPlugin<C>`, generic over the camera marker, and no game
//! knowledge: speakers are strings the host resolves, fonts and skins are handed in.

use std::path::{Path, PathBuf};

/// Everything this crate is allowed to depend on. `unicode-segmentation` (pure Rust, no dependencies,
/// nothing to do with an engine) is there for the grapheme clusters a too-long word is broken between.
const ALLOWED_DEPS: &[&str] = &["bevy", "ab_glyph", "serde", "unicode-segmentation"];

/// Crate names that would mean the boundary has been crossed. `bevy_math` is glam types and is
/// allowed above; anything that draws, schedules, or knows what a game is, is not.
const FORBIDDEN_DEP_MARKERS: &[&str] =
    &["avian", "emerge", "foundation_vs_slop", "MainCamera", "SquadMember", "MenuState"];

fn crate_root() -> PathBuf {
    // Cargo runs a test binary with the cwd set to its own package root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Strip `//` line comments and `/* */` blocks so a doc comment *about* Bevy — of which this crate has
/// several, deliberately — is not mistaken for an import of it.
///
/// Deliberately simple: it does not track string literals, so a `"//"` inside a string would end the
/// line early. That is acceptable here because the only thing this feeds is a substring search for
/// crate names, and an over-eager strip can only ever cause a false *pass* on a line that already
/// contains no dependency.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    for line in src.lines() {
        let mut rest = line;
        loop {
            if in_block {
                match rest.find("*/") {
                    Some(end) => {
                        in_block = false;
                        rest = &rest[end + 2..];
                    }
                    None => break,
                }
            } else {
                let line_at = rest.find("//");
                let block_at = rest.find("/*");
                match (line_at, block_at) {
                    (Some(l), b) if b.is_none_or(|b| l < b) => {
                        out.push_str(&rest[..l]);
                        break;
                    }
                    (_, Some(b)) => {
                        out.push_str(&rest[..b]);
                        in_block = true;
                        rest = &rest[b + 2..];
                    }
                    _ => {
                        out.push_str(rest);
                        break;
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn no_source_file_reaches_for_an_engine() {
    let src = crate_root().join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    assert!(
        files.len() >= 2,
        "expected to scan the whole crate, found only {} file(s) — has the layout moved?",
        files.len()
    );

    let mut offenders = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let code = strip_comments(&text);
        for (n, line) in code.lines().enumerate() {
            for marker in FORBIDDEN_DEP_MARKERS {
                if line.contains(marker) {
                    offenders.push(format!(
                        "{}:{} — {}",
                        path.strip_prefix(crate_root()).unwrap_or(path).display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "bevy_speech_bubbles must stay engine-free, but {} line(s) reference an engine crate.\n  {}\n\n\
         This crate is a dialog kit any game can take. If a type genuinely needs to cross the \
         boundary, the answer is a plain-data type here and a conversion on the far side — not a \
         dependency.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

#[test]
fn the_dependency_list_stays_closed() {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml"))
        .expect("bevy_speech_bubbles must have a Cargo.toml");

    // Only the `[dependencies]` table — a dev-dependency on something heavier would be a different
    // (and much less alarming) conversation.
    let deps = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("bevy_speech_bubbles must declare a [dependencies] table");
    let deps = deps.split("\n[").next().unwrap_or(deps);

    for line in deps.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `.` is a separator too, so an inherited dependency written as the dotted key
        // `serde.workspace = true` reads as `serde` rather than as a crate called
        // `serde.workspace`. A crate name cannot contain a dot, so nothing real is truncated —
        // and the check keeps biting on the part that matters, the name before it.
        let name = line.split(['=', ' ', '.']).next().unwrap_or("");
        // A dependency line is `name = ...` or `name.workspace = true`. Anything else inside this
        // table is a CONTINUATION of a multi-line value — a feature array's entries, a closing
        // bracket — and is not a crate name. Reading one as a crate name produced a genuinely
        // baffling failure ("declares `\"bevy_asset\",`"), so continuations are skipped by shape.
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        assert!(
            ALLOWED_DEPS.contains(&name),
            "bevy_speech_bubbles declares `{name}`, which is not in its allowed set {ALLOWED_DEPS:?}.\n\
             Widening this is a design decision, not a convenience. If it is genuinely warranted, add it to ALLOWED_DEPS in \
             this test in the same commit, so the change is visible in review."
        );
        for marker in FORBIDDEN_DEP_MARKERS {
            assert!(
                !name.contains(marker),
                "bevy_speech_bubbles declares `{name}` — the crate exists precisely so it does not depend on \
                 an engine."
            );
        }
    }
}
