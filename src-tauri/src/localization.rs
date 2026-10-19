use crate::preferences::Locale;
use serde_json::Value;
use std::sync::OnceLock;
fn config() -> &'static Value {
    static CONFIG: OnceLock<Value> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!("../../config/locales.json")).expect("language config")
    })
}
pub fn support_url(locale: Locale) -> Option<String> {
    config()["locales"][locale.key()]["support_url"]
        .as_str()
        .map(str::to_owned)
}
fn catalog(locale: Locale) -> &'static Value {
    static RU: OnceLock<Value> = OnceLock::new();
    static EN: OnceLock<Value> = OnceLock::new();
    match locale {
        Locale::Ru => RU.get_or_init(|| {
            serde_json::from_str(include_str!("../../locales/ru.json")).expect("ru catalog")
        }),
        Locale::En => EN.get_or_init(|| {
            serde_json::from_str(include_str!("../../locales/en.json")).expect("en catalog")
        }),
    }
}
pub fn native_text(locale: Locale, key: &str) -> String {
    key.split('.')
        .fold(catalog(locale), |value, part| &value[part])
        .as_str()
        .unwrap_or(key)
        .to_owned()
}

pub fn native_format(locale: Locale, key: &str, args: &[(&str, String)]) -> String {
    let template = native_text(locale, key);
    let mut rest = template.as_str();
    let mut out = String::new();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &rest[start + 1..start + end];
        if let Some((_, value)) = args.iter().find(|(key, _)| *key == name) {
            out.push_str(value);
        } else {
            out.push_str(&rest[start..start + end + 1]);
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}
