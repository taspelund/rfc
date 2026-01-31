use serde::Serialize;

use super::Document;

/// Filter for search results
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum SearchFilter {
    /// Only return RFCs
    RfcsOnly,
    /// Only return Internet-Drafts
    DraftsOnly,
    /// Return both RFCs and drafts
    #[default]
    Both,
}

impl SearchFilter {
    /// Get the API parameter value for this filter
    pub fn api_param(&self) -> Option<&'static str> {
        match self {
            SearchFilter::RfcsOnly => Some("rfc"),
            SearchFilter::DraftsOnly => Some("draft"),
            SearchFilter::Both => None,
        }
    }
}

/// Search results from the API
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchResult {
    /// List of matching documents
    pub documents: Vec<Document>,
    /// Whether there are more results available
    pub has_more: bool,
    /// Total number of matching documents available (from API)
    pub total_count: Option<u32>,
    /// The query that produced these results
    pub query: String,
    /// The filter that was applied
    pub filter: SearchFilter,
}

impl SearchResult {
    /// Create an empty search result
    pub fn empty(query: String, filter: SearchFilter) -> Self {
        Self {
            documents: Vec::new(),
            has_more: false,
            total_count: None,
            query,
            filter,
        }
    }

    /// Check if this result set is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Get the number of documents in this result set
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_filter_api_param() {
        assert_eq!(SearchFilter::RfcsOnly.api_param(), Some("rfc"));
        assert_eq!(SearchFilter::DraftsOnly.api_param(), Some("draft"));
        assert_eq!(SearchFilter::Both.api_param(), None);
    }

    #[test]
    fn test_search_filter_default() {
        assert_eq!(SearchFilter::default(), SearchFilter::Both);
    }

    #[test]
    fn test_search_result_empty() {
        let result = SearchResult::empty("test query".to_string(), SearchFilter::RfcsOnly);

        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
        assert!(!result.has_more);
        assert_eq!(result.query, "test query");
        assert_eq!(result.filter, SearchFilter::RfcsOnly);
    }

    #[test]
    fn test_search_result_default() {
        let result = SearchResult::default();

        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
        assert!(!result.has_more);
        assert!(result.query.is_empty());
        assert_eq!(result.filter, SearchFilter::Both);
    }
}
