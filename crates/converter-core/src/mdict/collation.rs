/// Порядок MDict не зависит от локали операционной системы.
pub fn mdx_sort_key(key: &str) -> String {
    key.chars()
        .filter(|c| !c.is_ascii_punctuation() && *c != ' ')
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn mdd_sort_key(key: &str) -> String {
    key.to_lowercase()
}
