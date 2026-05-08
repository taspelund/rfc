//! Spawn a user-supplied program against a document.
//!
//! The document is always written to a tempfile and passed as the final
//! argument to the program — this works uniformly for editors and pagers
//! and avoids second-guessing what kind of viewer the user picked.

use std::env;
use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result};

/// Open `text` in a viewer.
///
/// Resolution order when `open_with` is `None`: `$EDITOR` → `$PAGER` →
/// no-op. The no-op case (neither var set, no `--open-with`) lets `rfc
/// fetch` work on headless systems without forcing the user to invent a
/// viewer.
pub fn open(text: &str, open_with: Option<&str>) -> Result<()> {
    let viewer_str = match open_with {
        Some(program) => program.to_string(),
        None => {
            if let Ok(editor) = env::var("EDITOR") {
                editor
            } else if let Ok(pager) = env::var("PAGER") {
                pager
            } else {
                return Ok(());
            }
        }
    };

    let (program, extra_args) = split_command(&viewer_str)
        .with_context(|| format!("Empty viewer command: {:?}", viewer_str))?;

    let mut temp_file = tempfile::NamedTempFile::new()?;
    temp_file.write_all(text.as_bytes())?;
    temp_file.flush()?;

    let status = Command::new(&program)
        .args(&extra_args)
        .arg(temp_file.path())
        .status()
        .with_context(|| format!("Failed to start viewer: {}", program))?;

    if !status.success() {
        anyhow::bail!("Viewer exited with non-zero status");
    }

    Ok(())
}

/// Split a viewer command string into `(program, args)` on whitespace.
/// Returns `None` when the input is empty/whitespace-only.
fn split_command(s: &str) -> Option<(String, Vec<String>)> {
    let mut parts = s.split_whitespace().map(String::from);
    let program = parts.next()?;
    Some((program, parts.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_command_basic() {
        assert_eq!(split_command("vim"), Some(("vim".to_string(), vec![])));
    }

    #[test]
    fn split_command_with_args() {
        assert_eq!(
            split_command("code -"),
            Some(("code".to_string(), vec!["-".to_string()]))
        );
        assert_eq!(
            split_command("nvim -R"),
            Some(("nvim".to_string(), vec!["-R".to_string()]))
        );
    }

    #[test]
    fn split_command_empty() {
        assert_eq!(split_command(""), None);
        assert_eq!(split_command("   "), None);
    }
}
