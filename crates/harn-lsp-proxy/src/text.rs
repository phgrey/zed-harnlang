pub fn get_word_at_position(text: &str, line: usize, col: usize) -> Option<String> {
    let line_str = text.lines().nth(line)?;

    if col > line_str.len() {
        return None;
    }

    // Find the word boundaries around `col`
    let mut start = col;
    while start > 0 && line_str.is_char_boundary(start - 1) {
        let c = line_str[..start].chars().last().unwrap();
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        start -= c.len_utf8();
    }

    let mut end = col;
    while end < line_str.len() {
        let c = line_str[end..].chars().next().unwrap();
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        end += c.len_utf8();
    }

    if start < end {
        Some(line_str[start..end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_word_at_start() {
        let text = "Harness::init()";
        assert_eq!(get_word_at_position(text, 0, 0), Some("Harness".to_string()));
        assert_eq!(get_word_at_position(text, 0, 3), Some("Harness".to_string()));
        assert_eq!(get_word_at_position(text, 0, 7), Some("Harness".to_string()));
    }

    #[test]
    fn test_get_word_in_middle() {
        let text = "let h = HarnessContext_v2::new();";
        assert_eq!(
            get_word_at_position(text, 0, 10),
            Some("HarnessContext_v2".to_string())
        );
    }

    #[test]
    fn test_get_word_punctuation_or_spaces() {
        let text = "fn   main() {\n  // comment\n}";
        // At cursor on middle space between multiple spaces (col 3: "fn [ ] main")
        // char before is space, char after is space
        assert_eq!(get_word_at_position(text, 0, 3), None);

        // At punctuation surrounded by non-alphanumeric, like open paren '(' preceded by ')'
        let text2 = "a + ( b )";
        assert_eq!(get_word_at_position(text2, 0, 4), None);
    }

    #[test]
    fn test_get_word_multiline() {
        let text = "line0\nline1_word\nline2";
        assert_eq!(
            get_word_at_position(text, 1, 6),
            Some("line1_word".to_string())
        );
    }

    #[test]
    fn test_get_word_out_of_bounds() {
        let text = "hello";
        assert_eq!(get_word_at_position(text, 5, 0), None);
        assert_eq!(get_word_at_position(text, 0, 100), None);
        assert_eq!(get_word_at_position("", 0, 0), None);
    }

    #[test]
    fn test_get_word_unicode() {
        let text = "let привет = Harness::new();";
        // "привет" is cyrillic alphanumeric
        assert_eq!(get_word_at_position(text, 0, 4), Some("привет".to_string()));
        // On "Harness"
        let harness_idx = text.find("Harness").unwrap();
        assert_eq!(
            get_word_at_position(text, 0, harness_idx + 2),
            Some("Harness".to_string())
        );
    }
}
