use crate::model::{ConverterError, DictionaryMetadata, Result};
use std::io::Write;

fn attribute(value: &str) -> Result<String> {
    if value
        .chars()
        .any(|c| !(matches!(c, '\t' | '\n' | '\r') || c >= ' '))
    {
        return Err(ConverterError::new(
            "invalid_metadata",
            "Метаданные содержат недопустимый символ XML",
        ));
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\r', "&#13;")
        .replace('\n', "&#10;"))
}

pub(super) fn write_header(
    out: &mut impl Write,
    metadata: Option<&DictionaryMetadata>,
) -> Result<()> {
    let xml = if let Some(meta) = metadata {
        format!(
            "<Dictionary GeneratedByEngineVersion=\"2.0\" RequiredEngineVersion=\"2.0\" Encoding=\"UTF-8\" Encrypted=\"0\" KeyCaseSensitive=\"No\" StripKey=\"Yes\" Title=\"{}\" Description=\"{}\"/>\r\n",
            attribute(&meta.title)?,
            attribute(&meta.description)?
        )
    } else {
        "<Library_Data GeneratedByEngineVersion=\"2.0\" RequiredEngineVersion=\"2.0\" Encrypted=\"0\" KeyCaseSensitive=\"No\" StripKey=\"No\"/>\r\n".to_owned()
    };
    let bytes: Vec<u8> = xml.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let length = u32::try_from(bytes.len())
        .map_err(|_| ConverterError::new("size_overflow", "Слишком большой заголовок словаря"))?;
    out.write_all(&length.to_be_bytes())?;
    out.write_all(&bytes)?;
    out.write_all(&adler2::adler32_slice(&bytes).to_le_bytes())?;
    Ok(())
}
