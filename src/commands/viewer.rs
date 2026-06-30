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
/// Resolution order when `open_with` is `None`:
///   `$VISUAL` → `$EDITOR` → `$PAGER` → platform default → no-op
///
/// The no-op case (no env vars set, no `--open-with`, no platform default)
/// lets `rfc fetch` work on headless systems without forcing the user to
/// invent a viewer.
///
/// On Windows the last-resort platform default is `notepad.exe`, which is
/// always available.  On Unix there is no universal default.
pub fn open(text: &str, open_with: Option<&str>) -> Result<()> {
    let viewer_str = match open_with {
        Some(program) => program.to_string(),
        None => match resolve_viewer() {
            Some(v) => v,
            None => return Ok(()),
        },
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

/// Resolve a viewer from environment variables, following the POSIX convention
/// (`VISUAL` before `EDITOR`) used by git, gh, starship, and most CLI tools.
/// Falls back to a platform-specific default when no env var is set.
fn resolve_viewer() -> Option<String> {
    resolve_viewer_from(
        env::var("VISUAL").ok(),
        env::var("EDITOR").ok(),
        env::var("PAGER").ok(),
    )
}

/// Pure-function core of [`resolve_viewer`] — takes already-read env vars so
/// tests don't need to manipulate the real environment.
pub(crate) fn resolve_viewer_from(
    visual: Option<String>,
    editor: Option<String>,
    pager: Option<String>,
) -> Option<String> {
    visual.or(editor).or(pager).or_else(platform_default_viewer)
}

/// Platform-specific last-resort viewer.
/// On Windows `notepad.exe` is always present; on Unix there is no universal
/// equivalent so we return `None` and let the caller no-op.
#[cfg(windows)]
fn platform_default_viewer() -> Option<String> {
    Some("notepad.exe".to_string())
}

#[cfg(not(windows))]
fn platform_default_viewer() -> Option<String> {
    None
}

/// Split a viewer command string into `(program, args)`.
///
/// Quoting rules differ by platform:
///
/// - **Unix**: backslash escapes, single quotes, and double quotes are all
///   honoured (standard shell tokenisation).
/// - **Windows**: only double quotes group tokens; backslash and single quote
///   are treated as ordinary characters so that Windows paths like
///   `C:\Program Files\editor.exe` are not mangled.
///
/// Returns `None` when the input is empty/whitespace-only.
fn split_command(s: &str) -> Option<(String, Vec<String>)> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut in_double_quote = false;
    let mut had_char = false;

    #[cfg(not(windows))]
    let mut in_single_quote = false;

    while let Some(c) = chars.next() {
        match c {
            // Unix only: backslash escapes the next character (except inside
            // single quotes where everything is literal).
            #[cfg(not(windows))]
            '\\' if !in_single_quote => {
                if let Some(next) = chars.next() {
                    current.push(next);
                    had_char = true;
                }
            }
            // Unix only: single quotes — everything inside is literal.
            #[cfg(not(windows))]
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                had_char = true;
            }
            '"' => {
                #[cfg(not(windows))]
                if in_single_quote {
                    current.push('"');
                    had_char = true;
                    continue;
                }
                in_double_quote = !in_double_quote;
                had_char = true;
            }
            c if c.is_whitespace() && {
                #[cfg(windows)]
                {
                    !in_double_quote
                }
                #[cfg(not(windows))]
                {
                    !in_double_quote && !in_single_quote
                }
            } =>
            {
                if had_char {
                    parts.push(current.clone());
                    current.clear();
                    had_char = false;
                }
            }
            c => {
                current.push(c);
                had_char = true;
            }
        }
    }

    if had_char {
        parts.push(current);
    }

    let mut iter = parts.into_iter();
    let program = iter.next()?;
    Some((program, iter.collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_viewer_visual_takes_precedence() {
        assert_eq!(
            resolve_viewer_from(
                Some("nvim".to_string()),
                Some("vim".to_string()),
                Some("less".to_string()),
            ),
            Some("nvim".to_string())
        );
    }

    #[test]
    fn resolve_viewer_editor_fallback() {
        assert_eq!(
            resolve_viewer_from(None, Some("vim".to_string()), Some("less".to_string())),
            Some("vim".to_string())
        );
    }

    #[test]
    fn resolve_viewer_pager_fallback() {
        assert_eq!(
            resolve_viewer_from(None, None, Some("less".to_string())),
            Some("less".to_string())
        );
    }

    #[test]
    fn resolve_viewer_all_none_on_unix() {
        // On Unix platform_default_viewer returns None.
        #[cfg(not(windows))]
        assert_eq!(resolve_viewer_from(None, None, None), None);
        #[cfg(windows)]
        assert_eq!(
            resolve_viewer_from(None, None, None),
            Some("notepad.exe".to_string())
        );
    }

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

    #[test]
    fn split_command_double_quoted_path() {
        assert_eq!(
            split_command(r#""/path/to/my program""#),
            Some(("/path/to/my program".to_string(), vec![]))
        );
    }

    #[test]
    fn split_command_double_quoted_path_with_args() {
        assert_eq!(
            split_command(r#""/path/to/my editor" -R"#),
            Some(("/path/to/my editor".to_string(), vec!["-R".to_string()]))
        );
    }

    #[test]
    fn split_command_double_quoted_arg() {
        assert_eq!(
            split_command("echo \"hello world\""),
            Some(("echo".to_string(), vec!["hello world".to_string()]))
        );
    }

    #[test]
    fn split_command_multiple_quoted_args() {
        assert_eq!(
            split_command("echo \"a\" \"b c\""),
            Some(("echo".to_string(), vec!["a".to_string(), "b c".to_string()]))
        );
    }

    #[test]
    fn split_command_single_quote_inside_double() {
        assert_eq!(
            split_command("echo \"it's fine\""),
            Some(("echo".to_string(), vec!["it's fine".to_string()]))
        );
    }

    // Unix-only quoting tests
    #[cfg(not(windows))]
    mod unix {
        use super::*;

        #[test]
        fn backslash_escaped_space() {
            assert_eq!(
                split_command(r"/path/to/my\ program"),
                Some(("/path/to/my program".to_string(), vec![]))
            );
        }

        #[test]
        fn single_quoted_path() {
            assert_eq!(
                split_command("'/path/to/my program'"),
                Some(("/path/to/my program".to_string(), vec![]))
            );
        }

        #[test]
        fn escaped_quote_inside_double_quotes() {
            assert_eq!(
                split_command(r#"echo "a\"b""#),
                Some(("echo".to_string(), vec!["a\"b".to_string()]))
            );
        }

        #[test]
        fn double_quote_inside_single_quotes() {
            assert_eq!(
                split_command(r#"echo '"hello" world'"#),
                Some(("echo".to_string(), vec!["\"hello\" world".to_string()]))
            );
        }

        #[test]
        fn unclosed_single_quote_rest_is_literal() {
            assert_eq!(
                split_command("echo 'hello world"),
                Some(("echo".to_string(), vec!["hello world".to_string()]))
            );
        }

        #[test]
        fn mixed_quoting() {
            assert_eq!(
                split_command("echo 'a b' \"c d\""),
                Some((
                    "echo".to_string(),
                    vec!["a b".to_string(), "c d".to_string()]
                ))
            );
        }
    }

    // Windows-only quoting tests
    #[cfg(windows)]
    mod windows {
        use super::*;

        #[test]
        fn backslash_in_path_is_literal() {
            // Backslashes in Windows paths must not be treated as escapes.
            assert_eq!(
                split_command(r"C:\Program Files\editor.exe"),
                Some((
                    r"C:\Program".to_string(),
                    vec!["Files\\editor.exe".to_string()]
                ))
            );
        }

        #[test]
        fn double_quoted_windows_path() {
            assert_eq!(
                split_command(r#""C:\Program Files\editor.exe""#),
                Some((r"C:\Program Files\editor.exe".to_string(), vec![]))
            );
        }

        #[test]
        fn double_quoted_windows_path_with_args() {
            assert_eq!(
                split_command(r#""C:\Program Files\editor.exe" -R"#),
                Some((
                    r"C:\Program Files\editor.exe".to_string(),
                    vec!["-R".to_string()]
                ))
            );
        }
    }

    // viewer::open subprocess tests — these actually spawn processes.

    #[test]
    fn open_with_true_succeeds() {
        match open("hello", Some("/usr/bin/true")) {
            Ok(()) => {}
            Err(e) => panic!("open with /usr/bin/true failed: {:#}", e),
        }
    }

    #[test]
    fn open_with_cat_passes_content() {
        match open("line1\nline2", Some("/bin/cat")) {
            Ok(()) => {}
            Err(e) => panic!("open with /bin/cat failed: {:#}", e),
        }
    }

    #[test]
    fn open_empty_text_succeeds() {
        match open("", Some("/usr/bin/true")) {
            Ok(()) => {}
            Err(e) => panic!("open with /usr/bin/true empty text failed: {:#}", e),
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn open_nonexistent_viewer_fails() {
        let result = open("content", Some("/nonexistent/editor"));
        assert!(result.is_err());
    }

    #[test]
    #[cfg(not(windows))]
    fn open_with_empty_string_fails() {
        let result = open("content", Some(""));
        assert!(result.is_err());
    }
}
