pub(super) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn uri(value: &str, keep_slashes: bool) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || (keep_slashes && byte == b'/')
        {
            output.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

pub(super) fn argument<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("{key}=");
    for (position, _) in raw.match_indices(&pattern) {
        if position > 0 && !raw[..position].ends_with(char::is_whitespace) {
            continue;
        }
        let value = raw[position + pattern.len()..].trim_start();
        if let Some(quote) = value.chars().next().filter(|ch| matches!(ch, '\'' | '"')) {
            let mut escaped = false;
            for (end, ch) in value.char_indices().skip(1) {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    return Some(&value[1..end]);
                }
            }
            return None;
        }
        return Some(value.split_whitespace().next().unwrap_or_default());
    }
    None
}

pub(super) fn substitute_tilde(raw: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            result.push(ch);
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else if ch == '~' {
            result.push_str(replacement);
        } else {
            result.push(ch);
        }
    }
    result
}

pub(super) fn inline_text(raw: &str, display: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                result.push(next);
            } else {
                result.push(ch);
            }
        } else if ch == '~' {
            result.push_str(display);
        } else {
            result.push(ch);
        }
    }
    result
}

pub(super) fn subentry_reference(raw: &str, limit: usize) -> crate::model::Result<String> {
    fn escape(value: &str) -> String {
        let mut result = String::new();
        for ch in value.chars() {
            if matches!(ch, '\\' | '[' | ']' | '<' | '>' | '~' | '"') {
                result.push('\\');
            }
            result.push(ch);
        }
        result
    }
    let keys = super::headwords::expand_headwords(raw, limit)?;
    let key = keys.first().ok_or_else(|| {
        crate::model::ConverterError::new("invalid_headword", "Подстатья не содержит ключа")
            .with_message("messages.invalid_headword", crate::model::message_args([]))
    })?;
    let display = super::headwords::display_headword(raw)?;
    Ok(format!(
        "[ref target=\"{}\"]{}[/ref]\n",
        escape(key),
        escape(&display)
    ))
}
