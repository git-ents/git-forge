use ammonia::clean;
use arborium::{Config, Highlighter, HtmlFormat, detect_language};
use asciidoc_html5::{Options as AsciiDocOptions, SafeMode, convert_with};
use comrak::{Options, markdown_to_html};

pub fn highlight_file(path: &str, source: &str) -> String {
    let Some(language) = detect_language(path) else {
        return ammonia::clean_text(source);
    };

    let config = Config {
        html_format: HtmlFormat::ClassNamesWithPrefix("arb".to_owned()),
        ..Config::default()
    };
    let mut highlighter = Highlighter::with_config(config);

    highlighter
        .highlight(language, source)
        .unwrap_or_else(|_| ammonia::clean_text(source))
}

pub fn render_markdown(source: &str) -> String {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.tagfilter = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.alerts = true;
    options.render.gfm_quirks = true;
    options.render.r#unsafe = false;

    clean(&markdown_to_html(source, &options))
}

pub fn render_asciidoc(source: &str) -> String {
    let options = AsciiDocOptions::new()
        .embedded(true)
        .safe_mode(SafeMode::Secure);

    clean(&convert_with(source, &options))
}

#[cfg(test)]
mod tests {
    use super::{highlight_file, render_asciidoc, render_markdown};

    #[test]
    fn highlights_known_files_and_escapes_unknown_files() {
        let highlighted = highlight_file("main.rs", "fn main() {}");
        assert!(highlighted.contains("arb-"));

        let plain = highlight_file("README.unknown", "<script>");
        assert_eq!(plain, "&lt;script&gt;");
    }

    #[test]
    fn renders_sanitized_markdown() {
        let html = render_markdown("# Hello\n\n<script>alert(1)</script>");
        assert!(html.contains("<h1"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn renders_sanitized_embedded_asciidoc() {
        let html = render_asciidoc("= Hello\n\nWorld.");
        assert!(html.contains("<p>World.</p>"));
        assert!(!html.contains("<!DOCTYPE"));
    }
}
