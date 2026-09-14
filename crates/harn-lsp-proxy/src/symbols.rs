use serde_json::Value;

pub fn find_symbol_location(results: &[Value], word: &str) -> Option<Value> {
    // First pass: look for exact name match + kind struct/enum/class/interface (22/13/5/11)
    for sym in results {
        if let (Some(name), Some(kind)) = (
            sym.get("name").and_then(|n| n.as_str()),
            sym.get("kind").and_then(|k| k.as_u64()),
        ) {
            if name == word && (kind == 22 || kind == 13 || kind == 5 || kind == 11) {
                if let Some(loc) = sym.get("location") {
                    return Some(loc.clone());
                }
            }
        }
    }

    // Second pass: just exact name match
    for sym in results {
        if let Some(name) = sym.get("name").and_then(|n| n.as_str()) {
            if name == word {
                if let Some(loc) = sym.get("location") {
                    return Some(loc.clone());
                }
            }
        }
    }

    // Fallback: first item's location
    if let Some(first) = results.first() {
        if let Some(loc) = first.get("location") {
            return Some(loc.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn test_find_symbol_fallback_first() {
        let results = vec![
            json!({
                "name": "HarnessAlternative",
                "kind": 1,
                "location": {"uri": "file:///src/first.rs"}
            }),
            json!({
                "name": "SomethingElse",
                "kind": 1,
                "location": {"uri": "file:///src/second.rs"}
            }),
        ];

        let loc = find_symbol_location(&results, "UnmatchedWord").unwrap();
        assert_eq!(loc["uri"], "file:///src/first.rs");
    }

    #[test]
    fn test_find_symbol_empty_or_no_location() {
        assert_eq!(find_symbol_location(&[], "Harness"), None);

        let results = vec![json!({"name": "Harness"})];
        assert_eq!(find_symbol_location(&results, "Harness"), None);
    }
}
