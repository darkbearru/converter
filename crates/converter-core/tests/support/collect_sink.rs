use converter_core::{
    formats::EntrySink,
    model::{Diagnostic, DictionaryEntry, Result},
};

#[derive(Default)]
pub struct CollectSink {
    pub entries: Vec<DictionaryEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

impl EntrySink for CollectSink {
    fn push(&mut self, entry: DictionaryEntry) -> Result<()> {
        self.entries.push(entry);
        Ok(())
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()> {
        self.diagnostics.push(diagnostic);
        Ok(())
    }
}
