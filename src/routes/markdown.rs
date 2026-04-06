use pulldown_cmark::{html, Options, Parser};

/// Convert markdown to sanitized HTML, preventing XSS.
/// Uses pulldown-cmark to parse markdown, then ammonia to sanitize the HTML output.
pub fn render_markdown(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Sanitize with ammonia — removes all dangerous tags/attributes
    ammonia::clean(&html_output)
}
