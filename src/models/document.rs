use serde::{Deserialize, Serialize};

/// The type of document - either an RFC or an Internet-Draft
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentType {
    /// An RFC document with its number
    Rfc(u32),
    /// An Internet-Draft with its name
    Draft(String),
}

impl DocumentType {
    /// Parse a user-supplied identifier into a `DocumentType`.
    ///
    /// Recognized RFC forms (case-insensitive, leading/trailing whitespace
    /// trimmed): bare numbers like `9000`, prefixed forms like `rfc9000`
    /// or `RFC 9000`. Anything else is treated as a draft name; a missing
    /// `draft-` prefix is added automatically so users can write either
    /// `rfc 4271`-style shorthand or full draft names.
    pub fn from_user_input(s: &str) -> Self {
        let s = s.trim().to_lowercase();

        if let Some(num_str) = s.strip_prefix("rfc") {
            if let Ok(num) = num_str.trim().parse::<u32>() {
                return DocumentType::Rfc(num);
            }
        }

        if let Ok(num) = s.parse::<u32>() {
            return DocumentType::Rfc(num);
        }

        if s.starts_with("draft-") {
            DocumentType::Draft(s)
        } else {
            DocumentType::Draft(format!("draft-{}", s))
        }
    }

    /// Parse a canonical, server-supplied document name (e.g. `rfc9000`,
    /// `draft-ietf-quic-transport-34`). Unlike `from_user_input`, this
    /// makes no attempt to repair malformed input — the only ambiguity is
    /// the RFC-vs-draft prefix.
    pub fn from_canonical_name(name: &str) -> Self {
        if let Some(num_str) = name.strip_prefix("rfc") {
            if let Ok(num) = num_str.parse::<u32>() {
                return DocumentType::Rfc(num);
            }
        }
        DocumentType::Draft(name.to_string())
    }

    /// Get the canonical name for this document
    pub fn name(&self) -> String {
        match self {
            DocumentType::Rfc(num) => format!("rfc{}", num),
            DocumentType::Draft(name) => name.clone(),
        }
    }

    /// Get a display-friendly name
    pub fn display_name(&self) -> String {
        match self {
            DocumentType::Rfc(num) => format!("RFC {}", num),
            DocumentType::Draft(name) => name.clone(),
        }
    }
}

impl std::fmt::Display for DocumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Document content format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Html,
    Text,
}

impl Format {
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Html => "html",
            Format::Text => "txt",
        }
    }
}

/// An IETF document (RFC or Internet-Draft).
///
/// Only the fields the CLI actually displays are kept; richer metadata
/// (pages, authors, publication date, etc.) lives on the wire type and
/// is dropped at the API boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Canonical name (e.g. `rfc9000` or `draft-ietf-quic-transport-34`).
    pub name: String,
    /// Human-readable title.
    pub title: String,
    pub doc_type: DocumentType,
}

impl Document {
    pub fn new(name: String, title: String, doc_type: DocumentType) -> Self {
        Self {
            name,
            title,
            doc_type,
        }
    }

    /// Get a short display title (truncated if necessary)
    pub fn short_title(&self, max_len: usize) -> String {
        if self.title.chars().count() <= max_len {
            self.title.clone()
        } else {
            let truncated: String = self.title.chars().take(max_len.saturating_sub(3)).collect();
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_user_input_rfc() {
        assert_eq!(
            DocumentType::from_user_input("9000"),
            DocumentType::Rfc(9000)
        );
        assert_eq!(
            DocumentType::from_user_input("rfc9000"),
            DocumentType::Rfc(9000)
        );
        assert_eq!(
            DocumentType::from_user_input("RFC9000"),
            DocumentType::Rfc(9000)
        );
        assert_eq!(
            DocumentType::from_user_input("RFC 9000"),
            DocumentType::Rfc(9000)
        );
        assert_eq!(
            DocumentType::from_user_input("  rfc9000  "),
            DocumentType::Rfc(9000)
        );
    }

    #[test]
    fn test_from_user_input_draft() {
        assert_eq!(
            DocumentType::from_user_input("draft-ietf-quic-transport-34"),
            DocumentType::Draft("draft-ietf-quic-transport-34".to_string())
        );
        // Bare names get a draft- prefix added.
        assert_eq!(
            DocumentType::from_user_input("ietf-quic-transport"),
            DocumentType::Draft("draft-ietf-quic-transport".to_string())
        );
    }

    #[test]
    fn test_from_canonical_name() {
        assert_eq!(
            DocumentType::from_canonical_name("rfc9000"),
            DocumentType::Rfc(9000)
        );
        assert_eq!(
            DocumentType::from_canonical_name("draft-ietf-quic-transport-34"),
            DocumentType::Draft("draft-ietf-quic-transport-34".to_string())
        );
        // "rfc" with no number is a draft name on the wire (rare but legal).
        assert_eq!(
            DocumentType::from_canonical_name("rfcfoo"),
            DocumentType::Draft("rfcfoo".to_string())
        );
    }

    #[test]
    fn test_document_type_display() {
        assert_eq!(DocumentType::Rfc(9000).to_string(), "RFC 9000");
        assert_eq!(
            DocumentType::Draft("draft-ietf-quic-transport".to_string()).to_string(),
            "draft-ietf-quic-transport"
        );
    }

    #[test]
    fn test_short_title() {
        let doc = Document::new(
            "rfc9000".to_string(),
            "A Very Long Title That Needs Truncation".to_string(),
            DocumentType::Rfc(9000),
        );

        // No truncation needed
        assert_eq!(
            doc.short_title(100),
            "A Very Long Title That Needs Truncation"
        );

        // Truncation with ellipsis
        assert_eq!(doc.short_title(20), "A Very Long Title...");

        // Edge cases
        assert_eq!(doc.short_title(3), "...");
        assert_eq!(doc.short_title(0), "...");
    }

    #[test]
    fn test_short_title_utf8() {
        // Test with multibyte UTF-8 characters to ensure no panic
        let doc = Document::new(
            "rfc1234".to_string(),
            "Café résumé naïve".to_string(),
            DocumentType::Rfc(1234),
        );

        // Should not panic on multibyte characters
        let result = doc.short_title(10);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 10);
    }
}
