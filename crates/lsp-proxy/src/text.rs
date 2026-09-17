use serde_json::Value;
use std::fs::read_to_string;

/// Ranks and deduplicates candidate symbol locations for a queried word.
///
/// Priority ranking:
/// 1. Exact match on Types (Struct, Enum, Class, Interface: 22, 13, 5, 11) -> 100
/// 2. Exact match on Functions / Methods (12, 6) -> 90
/// 3. Other exact match -> 80
/// 4. Prefix match on Types -> 50
/// 5. Other prefix match -> 40
/// 6. Fuzzy / Substring match -> 10
pub fn rank_symbol_locations(results: &[Value], word: &str) -> Vec<Value> {
    let mut scored: Vec<(u32, Value)> = Vec::new();

    for sym in results {
        let name = sym.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let kind = sym.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
        let location = match sym.get("location") {
            Some(loc) => loc.clone(),
            None => continue,
        };

        let score = if name == word {
            match kind {
                22 | 13 | 5 | 11 => 100,
                12 | 6 => 90,
                _ => 80,
            }
        } else if name.starts_with(word) {
            match kind {
                22 | 13 | 5 | 11 => 50,
                _ => 40,
            }
        } else {
            10
        };

        scored.push((score, location));
    }

    // Sort descending by score (stable sort preserves original ordering for equal scores)
    scored.sort_by_key(|a| std::cmp::Reverse(a.0));

    // Deduplicate identical locations while preserving score order
    let mut locations = Vec::new();
    for (_, loc) in scored {
        if !locations.contains(&loc) {
            locations.push(loc);
        }
    }

    locations
}

pub fn find_symbol_location(results: &[Value], word: &str) -> Option<Value> {
    rank_symbol_locations(results, word).into_iter().next()
}

pub fn word_at_cursor(params: &Value) -> Option<String> {
    if let (Some(uri), Some(line), Some(col)) = (
        params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
        params.pointer("/position/line").and_then(|v| v.as_u64()),
        params
            .pointer("/position/character")
            .and_then(|v| v.as_u64()),
    ) {
        let fpath = uri.strip_prefix("file://").unwrap_or(uri);
        let text = read_to_string(fpath).ok()?;
        get_word_at_position(&text, line as usize, col as usize)
    } else {
        None
    }
}

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
    use serde_json::json;

    #[test]
    fn test_get_word_at_start() {
        let text = "Harness::init()";
        assert_eq!(
            get_word_at_position(text, 0, 0),
            Some("Harness".to_string())
        );
        assert_eq!(
            get_word_at_position(text, 0, 3),
            Some("Harness".to_string())
        );
        assert_eq!(
            get_word_at_position(text, 0, 7),
            Some("Harness".to_string())
        );
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
        assert_eq!(get_word_at_position(text, 0, 4), Some("привет".to_string()));
        let harness_idx = text.find("Harness").unwrap();
        assert_eq!(
            get_word_at_position(text, 0, harness_idx + 2),
            Some("Harness".to_string())
        );
    }

    #[test]
    fn test_word_at_cursor_with_temp_file() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_lsp_cursor.harn");
        std::fs::write(&file_path, "fn test_cursor() { Harness::init(); }").unwrap();

        let params = json!({
            "textDocument": {
                "uri": format!("file://{}", file_path.to_str().unwrap())
            },
            "position": {
                "line": 0,
                "character": 21
            }
        });

        let word = word_at_cursor(&params);
        assert_eq!(word, Some("Harness".to_string()));

        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn test_rank_symbol_locations_ordering_and_dedup() {
        let results = vec![
            json!({
                "name": "HarnessHelper",
                "kind": 12,
                "location": {"uri": "file:///src/helper.rs"}
            }),
            json!({
                "name": "Harness",
                "kind": 12, // Function (score 90)
                "location": {"uri": "file:///src/func.rs"}
            }),
            json!({
                "name": "Harness",
                "kind": 22, // Struct (score 100)
                "location": {"uri": "file:///src/struct.rs"}
            }),
            // Duplicate location
            json!({
                "name": "Harness",
                "kind": 22,
                "location": {"uri": "file:///src/struct.rs"}
            }),
        ];

        let ranked = rank_symbol_locations(&results, "Harness");
        assert_eq!(ranked.len(), 3);
        // Struct first
        assert_eq!(ranked[0]["uri"], "file:///src/struct.rs");
        // Function second
        assert_eq!(ranked[1]["uri"], "file:///src/func.rs");
        // Prefix helper third
        assert_eq!(ranked[2]["uri"], "file:///src/helper.rs");
    }

    #[test]
    fn test_find_symbol_exact_priority_kind() {
        let results = vec![
            json!({
                "name": "Harness",
                "kind": 12, // Function
                "location": {"uri": "file:///src/func.rs"}
            }),
            json!({
                "name": "Harness",
                "kind": 22, // Struct
                "location": {"uri": "file:///src/struct.rs"}
            }),
        ];

        let loc = find_symbol_location(&results, "Harness").unwrap();
        assert_eq!(loc["uri"], "file:///src/struct.rs");
    }

    #[test]
    fn test_find_symbol_exact_name_secondary() {
        let results = vec![
            json!({
                "name": "Other",
                "kind": 22,
                "location": {"uri": "file:///src/other.rs"}
            }),
            json!({
                "name": "HarnessTarget",
                "kind": 1,
                "location": {"uri": "file:///src/target.rs"}
            }),
        ];

        let loc = find_symbol_location(&results, "HarnessTarget").unwrap();
        assert_eq!(loc["uri"], "file:///src/target.rs");
    }

    #[test]
    fn test_find_symbol_empty_or_no_location() {
        assert_eq!(find_symbol_location(&[], "Harness"), None);

        let results = vec![json!({"name": "Harness"})];
        assert_eq!(find_symbol_location(&results, "Harness"), None);
    }
}
