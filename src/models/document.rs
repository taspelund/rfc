use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::DATATRACKER_BASE_URL;

/// The type of document - either an RFC or an Internet-Draft
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentType {
    /// An RFC document with its number
    Rfc(u32),
    /// An Internet-Draft with its name
    Draft(String),
}

impl DocumentType {
    /// Parse a document type from a string
    /// Handles formats like "rfc9000", "RFC 9000", "9000", or draft names
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();

        // Try to parse as RFC number
        if let Some(num_str) = s.strip_prefix("rfc") {
            let num_str = num_str.trim();
            if let Ok(num) = num_str.parse::<u32>() {
                return Some(DocumentType::Rfc(num));
            }
        }

        // Try to parse as plain number (assumed RFC)
        if let Ok(num) = s.parse::<u32>() {
            return Some(DocumentType::Rfc(num));
        }

        // Check if it looks like a draft name
        if s.starts_with("draft-") || s.contains("draft") {
            return Some(DocumentType::Draft(s));
        }

        None
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

    /// Get the IETF Datatracker URL for this document
    pub fn datatracker_url(&self) -> String {
        match self {
            DocumentType::Rfc(num) => format!("{}/doc/rfc{}/", DATATRACKER_BASE_URL, num),
            DocumentType::Draft(name) => format!("{}/doc/{}/", DATATRACKER_BASE_URL, name),
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

/// An IETF document (RFC or Internet-Draft)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Internal name (e.g., "rfc9000" or "draft-ietf-quic-transport-34")
    pub name: String,
    /// Human-readable title
    pub title: String,
    /// Document type
    pub doc_type: DocumentType,
    /// Abstract text
    pub abstract_text: Option<String>,
    /// Number of pages
    pub pages: Option<u32>,
    /// Publication date
    pub published: Option<DateTime<Utc>>,
    /// Document status (e.g., "Standards Track", "Informational")
    pub status: Option<String>,
    /// List of authors
    pub authors: Vec<String>,
    /// Stream (e.g., "IETF", "IAB", "IRTF")
    pub stream: Option<String>,
    /// Working group
    pub wg: Option<String>,
}

impl Document {
    /// Create a new document with minimal information
    pub fn new(name: String, title: String, doc_type: DocumentType) -> Self {
        Self {
            name,
            title,
            doc_type,
            abstract_text: None,
            pages: None,
            published: None,
            status: None,
            authors: Vec::new(),
            stream: None,
            wg: None,
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
    fn test_parse_rfc_number() {
        assert_eq!(DocumentType::parse("9000"), Some(DocumentType::Rfc(9000)));
        assert_eq!(
            DocumentType::parse("rfc9000"),
            Some(DocumentType::Rfc(9000))
        );
        assert_eq!(
            DocumentType::parse("RFC9000"),
            Some(DocumentType::Rfc(9000))
        );
        assert_eq!(
            DocumentType::parse("RFC 9000"),
            Some(DocumentType::Rfc(9000))
        );
    }

    #[test]
    fn test_parse_draft() {
        assert_eq!(
            DocumentType::parse("draft-ietf-quic-transport-34"),
            Some(DocumentType::Draft(
                "draft-ietf-quic-transport-34".to_string()
            ))
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
    fn test_parse_edge_cases() {
        // Empty/whitespace
        assert_eq!(DocumentType::parse(""), None);
        assert_eq!(DocumentType::parse("   "), None);

        // RFC without number
        assert_eq!(DocumentType::parse("rfc"), None);
        assert_eq!(DocumentType::parse("RFC"), None);

        // Whitespace handling
        assert_eq!(
            DocumentType::parse("  rfc9000  "),
            Some(DocumentType::Rfc(9000))
        );
        assert_eq!(
            DocumentType::parse("  9000  "),
            Some(DocumentType::Rfc(9000))
        );

        // RFC zero (technically parses, semantically invalid but not our problem)
        assert_eq!(DocumentType::parse("0"), Some(DocumentType::Rfc(0)));

        // Overflow protection - very large number should fail to parse
        assert_eq!(DocumentType::parse("99999999999999999999"), None);

        // Draft edge cases
        assert_eq!(
            DocumentType::parse("draft-"),
            Some(DocumentType::Draft("draft-".to_string()))
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
