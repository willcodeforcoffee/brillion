//! Post body rendering: Markdown → HTML, then sanitized (SPEC.md §12
//! #4 — `ammonia`, decided). Applied uniformly to locally-authored
//! content here; federated (remote) HTML will go through the same
//! sanitizer once inbound `Create` lands (phase 3/4).

use pulldown_cmark::{html, Options, Parser};

pub fn render(markdown: &str) -> String {
    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);
    ammonia::clean(&unsafe_html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let html = render("# Hello\n\nSome **bold** text.");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn strips_script_tags() {
        let html = render("<script>alert('xss')</script>\n\nHi");
        assert!(!html.contains("<script>"));
        assert!(html.contains("Hi"));
    }
}
