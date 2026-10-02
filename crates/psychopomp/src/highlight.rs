//! A small TypeScript/JavaScript highlighter that compiles one source line into
//! styled spans for the editor recipe. Roles follow the Presentation Theme syntax
//! mapping: keywords, function calls as the accent, capitalized names as types,
//! strings, muted comments, and numbers. It is a line-local approximation for
//! explainers, not a parser: template literals and block comments that span lines
//! are not tracked.
use crate::code::{StyledSpan, SyntaxStyle};

const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "of",
    "return",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

/// Muted gray: neutral, so every theme maps it to its muted text color.
pub const COMMENT: SyntaxStyle = SyntaxStyle::Rgb(128, 128, 128);
/// Numbers keep one warm color in every theme.
pub const NUMBER: SyntaxStyle = SyntaxStyle::Rgb(245, 167, 66);

pub fn typescript(line: &str) -> Vec<StyledSpan> {
    let chars = line.char_indices().collect::<Vec<_>>();
    let mut spans: Vec<StyledSpan> = Vec::new();
    let push = |spans: &mut Vec<StyledSpan>, text: &str, style: SyntaxStyle| {
        if text.is_empty() {
            return;
        }
        match spans.last_mut() {
            Some(last) if same(last.style, style) => last.text.push_str(text),
            _ => spans.push(StyledSpan::new(text, style)),
        }
    };
    let slice = |from: usize, to: usize| {
        let start = chars.get(from).map_or(line.len(), |(i, _)| *i);
        let end = chars.get(to).map_or(line.len(), |(i, _)| *i);
        &line[start..end]
    };
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index].1;
        if c == '/' && chars.get(index + 1).is_some_and(|(_, n)| *n == '/') {
            push(&mut spans, slice(index, chars.len()), COMMENT);
            break;
        }
        if matches!(c, '"' | '\'' | '`') {
            let mut end = index + 1;
            while end < chars.len() && chars[end].1 != c {
                end += if chars[end].1 == '\\' { 2 } else { 1 };
            }
            let end = (end + 1).min(chars.len());
            push(&mut spans, slice(index, end), SyntaxStyle::String);
            index = end;
            continue;
        }
        if c.is_ascii_digit() {
            let mut end = index + 1;
            while end < chars.len()
                && (chars[end].1.is_ascii_alphanumeric() || matches!(chars[end].1, '_' | '.'))
            {
                end += 1;
            }
            push(&mut spans, slice(index, end), NUMBER);
            index = end;
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let mut end = index + 1;
            while end < chars.len()
                && (chars[end].1.is_alphanumeric() || matches!(chars[end].1, '_' | '$'))
            {
                end += 1;
            }
            let word = slice(index, end);
            let called = chars[end..]
                .iter()
                .map(|(_, c)| *c)
                .find(|c| !c.is_whitespace())
                == Some('(');
            let member = index > 0 && chars[index - 1].1 == '.';
            let style = if KEYWORDS.contains(&word) && !member {
                SyntaxStyle::Keyword
            } else if called {
                SyntaxStyle::Accent
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                SyntaxStyle::Type
            } else {
                SyntaxStyle::Plain
            };
            push(&mut spans, word, style);
            index = end;
            continue;
        }
        push(&mut spans, slice(index, index + 1), SyntaxStyle::Plain);
        index += 1;
    }
    spans
}

fn same(a: SyntaxStyle, b: SyntaxStyle) -> bool {
    use SyntaxStyle::*;
    match (a, b) {
        (Plain, Plain)
        | (Keyword, Keyword)
        | (Type, Type)
        | (String, String)
        | (Accent, Accent) => true,
        (Rgb(r1, g1, b1), Rgb(r2, g2, b2)) => (r1, g1, b1) == (r2, g2, b2),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles(line: &str) -> Vec<(String, &'static str)> {
        typescript(line)
            .into_iter()
            .map(|span| {
                let name = match span.style {
                    SyntaxStyle::Plain => "plain",
                    SyntaxStyle::Keyword => "keyword",
                    SyntaxStyle::Type => "type",
                    SyntaxStyle::String => "string",
                    SyntaxStyle::Accent => "accent",
                    SyntaxStyle::Rgb(128, 128, 128) => "comment",
                    SyntaxStyle::Rgb(..) => "number",
                };
                (span.text, name)
            })
            .collect()
    }

    #[test]
    fn roles_follow_the_theme_syntax_mapping() {
        assert_eq!(
            styles(r#"const failure = new Error("port in use") // why"#),
            vec![
                ("const".into(), "keyword"),
                (" failure = ".into(), "plain"),
                ("new".into(), "keyword"),
                (" ".into(), "plain"),
                ("Error".into(), "accent"),
                ("(".into(), "plain"),
                ("\"port in use\"".into(), "string"),
                (") ".into(), "plain"),
                ("// why".into(), "comment"),
            ]
        );
        assert_eq!(
            styles("yield* Effect.sleep(100)"),
            vec![
                ("yield".into(), "keyword"),
                ("* ".into(), "plain"),
                ("Effect".into(), "type"),
                (".".into(), "plain"),
                ("sleep".into(), "accent"),
                ("(".into(), "plain"),
                ("100".into(), "number"),
                (")".into(), "plain"),
            ]
        );
    }

    #[test]
    fn member_names_are_not_keywords_and_text_is_preserved() {
        let line = r#"  if (error.type === "a\"b") return x.default"#;
        let spans = typescript(line);
        assert_eq!(
            spans.iter().map(|s| s.text.as_str()).collect::<String>(),
            line
        );
        let last = spans.last().unwrap();
        assert!(last.text.ends_with("x.default") && matches!(last.style, SyntaxStyle::Plain));
        assert!(styles(line).contains(&("\"a\\\"b\"".into(), "string")));
    }
}
