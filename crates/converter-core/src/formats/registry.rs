use super::{DictionaryReader, FormatDescriptor};
use crate::{
    dsl::DslReader,
    model::{ConverterError, Result},
};

#[derive(Default)]
pub struct FormatRegistry {
    readers: Vec<Box<dyn DictionaryReader>>,
}

impl FormatRegistry {
    pub fn builtins() -> Self {
        Self {
            readers: vec![Box::new(DslReader)],
        }
    }
    pub fn register(&mut self, reader: impl DictionaryReader + 'static) -> Result<()> {
        let id = reader.descriptor().id;
        if id.is_empty() || self.readers.iter().any(|r| r.descriptor().id == id) {
            return Err(ConverterError::new(
                "duplicate_format",
                format!("Формат уже зарегистрирован или не имеет ID: {id}"),
            )
            .with_message(
                "messages.duplicate_format",
                crate::model::message_args([("id", id.to_string())]),
            ));
        }
        self.readers.push(Box::new(reader));
        Ok(())
    }
    pub fn descriptors(&self) -> Vec<FormatDescriptor> {
        self.readers.iter().map(|r| r.descriptor()).collect()
    }
    pub fn get(&self, id: &str) -> Result<&dyn DictionaryReader> {
        self.readers
            .iter()
            .find(|r| r.descriptor().id == id)
            .map(|r| r.as_ref())
            .ok_or_else(|| {
                ConverterError::new("unsupported_format", format!("Неизвестный формат: {id}"))
                    .with_message(
                        "messages.unsupported_format",
                        crate::model::message_args([("id", id.to_string())]),
                    )
            })
    }
    pub fn detect(&self, prefix: &[u8], name: &str) -> Result<String> {
        let prefix = &prefix[..prefix.len().min(64 * 1024)];
        let mut best = 0;
        let mut selected = None;
        let mut ambiguous = false;
        for reader in &self.readers {
            let confidence = reader.probe(prefix, name).confidence.min(100);
            if confidence > best {
                best = confidence;
                selected = Some(reader.descriptor().id);
                ambiguous = false;
            } else if confidence != 0 && confidence == best {
                ambiguous = true;
            }
        }
        if ambiguous {
            return Err(ConverterError::new(
                "ambiguous_format",
                "Несколько читателей определили формат с одинаковой уверенностью",
            )
            .with_message("messages.ambiguous_format", crate::model::message_args([])));
        }
        selected.ok_or_else(|| {
            ConverterError::new("unsupported_format", "Формат источника не поддерживается")
                .with_message(
                    "messages.unsupported_format_2",
                    crate::model::message_args([]),
                )
        })
    }
}
