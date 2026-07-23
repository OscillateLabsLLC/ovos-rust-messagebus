pub fn remove_comments(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::remove_comments;

    #[test]
    fn strips_full_line_comments() {
        let input = "// a comment\n{\"key\": 1}\n// another";
        assert_eq!(remove_comments(input), "{\"key\": 1}");
    }

    #[test]
    fn strips_indented_comments() {
        let input = "{\n    // indented comment\n    \"key\": 1\n}";
        assert_eq!(remove_comments(input), "{\n    \"key\": 1\n}");
    }

    #[test]
    fn preserves_inline_trailing_comments() {
        // Only whole-line comments are stripped; trailing comments stay put.
        let input = "\"key\": 1 // trailing";
        assert_eq!(remove_comments(input), input);
    }

    #[test]
    fn preserves_urls_inside_strings() {
        let input = "\"url\": \"https://example.com\"";
        assert_eq!(remove_comments(input), input);
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(remove_comments(""), "");
    }

    #[test]
    fn handles_comment_only_input() {
        assert_eq!(remove_comments("// one\n// two"), "");
    }
}
