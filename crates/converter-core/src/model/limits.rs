#[derive(Clone, Debug)]
pub struct Limits {
    pub sort_buffer_bytes: usize,
    pub max_article_bytes: usize,
    pub max_html_bytes: usize,
    pub max_headword_variants: usize,
    pub max_archive_entries: usize,
    pub max_unpacked_bytes: u64,
    pub max_compression_ratio: u64,
    pub merge_fan_in: usize,
    pub resource_buffer_bytes: usize,
    pub key_block_bytes: usize,
    pub record_block_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            sort_buffer_bytes: 64 * 1024 * 1024,
            max_article_bytes: 8 * 1024 * 1024,
            max_html_bytes: 8 * 1024 * 1024,
            max_headword_variants: 64,
            max_archive_entries: 100_000,
            max_unpacked_bytes: 32 * 1024 * 1024 * 1024,
            max_compression_ratio: 1_000,
            merge_fan_in: 32,
            resource_buffer_bytes: 1024 * 1024,
            key_block_bytes: 32 * 1024,
            record_block_bytes: 64 * 1024,
        }
    }
}

impl Limits {
    pub fn mobile() -> Self {
        Self {
            sort_buffer_bytes: 16 * 1024 * 1024,
            ..Self::default()
        }
    }
}
