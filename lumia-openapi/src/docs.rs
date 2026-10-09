//! The HTML page that renders the OpenAPI document with Scalar.

/// Builds the HTML page that renders the OpenAPI document at `spec_url` using
/// the [Scalar](https://github.com/scalar/scalar) API reference.
///
/// The page loads Scalar from a CDN and points it at `spec_url`; the script is
/// fetched by the browser, not by the server.
pub fn scalar_html(spec_url: &str) -> String {
    let spec_url = escape_attribute(spec_url);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>API Reference</title>
  </head>
  <body>
    <script id="api-reference" data-url="{spec_url}"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>
"#
    )
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_scalar_at_the_spec() {
        let html = scalar_html("/openapi.json");
        assert!(html.contains(r#"data-url="/openapi.json""#), "{html}");
        assert!(
            html.contains("cdn.jsdelivr.net/npm/@scalar/api-reference"),
            "{html}"
        );
    }

    #[test]
    fn escapes_the_spec_url() {
        let html = scalar_html(r#"/a"b&c.json"#);
        assert!(
            html.contains(r#"data-url="/a&quot;b&amp;c.json""#),
            "{html}"
        );
    }
}
