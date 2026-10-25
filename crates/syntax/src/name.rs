/// Normalises an entity or property name for comparison.
///
/// Names compare case-insensitively, ignoring leading, trailing and repeated
/// whitespace, so `[[Jane  Doe]]` and `[[jane doe]]` refer to the same names.
pub fn normalise_name(name: &str) -> String {
    name.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace_and_case() {
        assert_eq!(normalise_name("  Jane \t Doe "), "jane doe");
        assert_eq!(normalise_name("ÉCOLE"), "école");
        assert_eq!(normalise_name(""), "");
    }
}
