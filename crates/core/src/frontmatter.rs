//! Lossless reader/writer for the `---` header at the top of a note.
//!
//! This is deliberately not a YAML parser. Notes are also edited by hand and
//! by other tools, so every line we do not understand is kept exactly as it
//! was. We only interpret flat `key: value` lines.

/// One `key: value` entry plus any lines that followed it before the next key
/// (YAML lists, indented continuations, comments). Those are kept verbatim.
#[derive(Debug, Clone, PartialEq)]
struct Field {
    key: String,
    value: String,
    trailing: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    /// Lines that appear before the first key (comments, blank lines).
    leading: Vec<String>,
    fields: Vec<Field>,
    pub body: String,
}

impl Document {
    pub fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut lines = text.split('\n');
        let first = lines.next().unwrap_or("");
        if first.trim_end_matches('\r') != "---" {
            return Self { body: text.to_string(), ..Self::default() };
        }

        let mut doc = Self::default();
        let mut consumed = first.len() + 1;
        let mut closed = false;
        for raw in lines {
            consumed += raw.len() + 1;
            let line = raw.trim_end_matches('\r');
            if line == "---" {
                closed = true;
                break;
            }
            match split_key(line) {
                Some((key, value)) => doc.fields.push(Field {
                    key: key.to_string(),
                    value: value.to_string(),
                    trailing: Vec::new(),
                }),
                None => match doc.fields.last_mut() {
                    Some(field) => field.trailing.push(line.to_string()),
                    None => doc.leading.push(line.to_string()),
                },
            }
        }
        if !closed {
            // An opening `---` with no closing one is just a horizontal rule.
            return Self { body: text.to_string(), ..Self::default() };
        }

        let rest = text.get(consumed.min(text.len())..).unwrap_or("");
        let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n')).unwrap_or(rest);
        doc.body = rest.to_string();
        doc
    }

    pub fn has(&self, key: &str) -> bool {
        self.fields.iter().any(|f| f.key == key)
    }

    /// The value for `key` with surrounding quotes removed.
    pub fn get(&self, key: &str) -> Option<String> {
        self.fields.iter().find(|f| f.key == key).map(|f| unquote(&f.value))
    }

    pub fn get_bool(&self, key: &str) -> bool {
        matches!(self.get(key).as_deref(), Some("true" | "yes"))
    }

    /// Set `key`, keeping its position if it already exists.
    pub fn set(&mut self, key: &str, value: &str) {
        let value = quote_if_needed(value);
        match self.fields.iter_mut().find(|f| f.key == key) {
            Some(field) => {
                field.value = value;
                // A scalar replaces any multi-line value the key had before.
                field.trailing.retain(|l| l.trim().is_empty() || l.trim_start().starts_with('#'));
            }
            None => self.fields.push(Field { key: key.to_string(), value, trailing: Vec::new() }),
        }
    }

    pub fn render(&self) -> String {
        if self.fields.is_empty() && self.leading.is_empty() {
            return self.body.clone();
        }
        let mut out = String::from("---\n");
        for line in &self.leading {
            out.push_str(line);
            out.push('\n');
        }
        for field in &self.fields {
            out.push_str(&field.key);
            out.push(':');
            if !field.value.is_empty() {
                out.push(' ');
                out.push_str(&field.value);
            }
            out.push('\n');
            for line in &field.trailing {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str("---\n\n");
        out.push_str(&self.body);
        out
    }
}

fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    if !(value.is_empty() || value.starts_with(' ')) {
        return None;
    }
    Some((key, value.trim()))
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let mut out = String::new();
        let mut chars = v[1..v.len() - 1].chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else {
        v.to_string()
    }
}

/// Plain values stay unquoted so the files read naturally; values YAML would
/// misread are double-quoted so other tools can still parse the header.
fn quote_if_needed(value: &str) -> String {
    let value = value.replace(['\n', '\r'], " ");
    let risky_start = value
        .chars()
        .next()
        .is_some_and(|c| "[]{}&*!|>'\"%@`#-?:, ".contains(c));
    let risky = risky_start
        || value.ends_with(' ')
        || value.ends_with(':')
        || value.contains(": ")
        || value.contains(" #");
    if risky {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_blog_format() {
        let doc = Document::parse("---\ntitle: Hello World\ndate: 2024-04-02\n---\n\nBody here.\n");
        assert_eq!(doc.get("title").as_deref(), Some("Hello World"));
        assert_eq!(doc.get("date").as_deref(), Some("2024-04-02"));
        assert_eq!(doc.body, "Body here.\n");
    }

    #[test]
    fn file_without_header_is_all_body() {
        let text = "# Just a note\n\ntext";
        let doc = Document::parse(text);
        assert!(!doc.has("title"));
        assert_eq!(doc.render(), text);
    }

    #[test]
    fn unclosed_header_is_treated_as_body() {
        let text = "---\nnot: closed\n\nmore";
        assert_eq!(Document::parse(text).body, text);
    }

    #[test]
    fn unknown_and_multiline_fields_survive_a_round_trip() {
        let text = "---\n# comment\ntitle: A\ntags:\n  - one\n  - two\naliases: [x, y]\n---\n\nBody\n";
        let mut doc = Document::parse(text);
        assert_eq!(doc.render(), text);
        doc.set("title", "B");
        assert_eq!(
            doc.render(),
            "---\n# comment\ntitle: B\ntags:\n  - one\n  - two\naliases: [x, y]\n---\n\nBody\n"
        );
    }

    #[test]
    fn titles_yaml_would_misread_are_quoted_and_read_back() {
        let mut doc = Document::parse("body");
        doc.set("title", "Plan: \"phase\" one");
        let text = doc.render();
        assert!(text.contains("title: \"Plan: \\\"phase\\\" one\"\n"), "{text}");
        assert_eq!(Document::parse(&text).get("title").as_deref(), Some("Plan: \"phase\" one"));
    }

    #[test]
    fn crlf_files_parse() {
        let doc = Document::parse("---\r\ntitle: Win\r\n---\r\n\r\nBody");
        assert_eq!(doc.get("title").as_deref(), Some("Win"));
        assert_eq!(doc.body, "Body");
    }

    #[test]
    fn horizontal_rule_in_body_is_not_a_header_end() {
        let doc = Document::parse("---\ntitle: A\n---\n\none\n\n---\n\ntwo");
        assert_eq!(doc.body, "one\n\n---\n\ntwo");
    }
}
