use std::collections::HashMap;

use percent_encoding::percent_decode_str;

/// Decodes a percent-encoded path segment into a UTF-8 string.
pub(crate) fn decode_path_segment(segment: &str) -> String {
    percent_decode_str(segment).decode_utf8_lossy().into_owned()
}

/// Parses an `application/x-www-form-urlencoded` query string into a map.
///
/// Keys that appear more than once keep their last value.
pub(crate) fn parse_query(query: &str) -> HashMap<String, String> {
    form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(decode_path_segment("hello%20world"), "hello world");
        assert_eq!(decode_path_segment("100%25"), "100%");
        assert_eq!(decode_path_segment("plain"), "plain");
    }

    #[test]
    fn keeps_malformed_escapes() {
        assert_eq!(decode_path_segment("50%"), "50%");
        assert_eq!(decode_path_segment("%zz"), "%zz");
    }

    #[test]
    fn parses_query_strings() {
        let query = parse_query("q=rust+lang&page=2&tag=a&tag=b");
        assert_eq!(query.get("q").map(String::as_str), Some("rust lang"));
        assert_eq!(query.get("page").map(String::as_str), Some("2"));
        assert_eq!(query.get("tag").map(String::as_str), Some("b"));
        assert!(parse_query("").is_empty());
    }

    #[test]
    fn decodes_query_escapes() {
        let query = parse_query("name=Jane%20Doe");
        assert_eq!(query.get("name").map(String::as_str), Some("Jane Doe"));
    }
}
