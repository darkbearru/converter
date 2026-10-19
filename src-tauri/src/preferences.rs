use atomicwrites::{AtomicFile, OverwriteBehavior};
use converter_app::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    #[default]
    System,
    Ru,
    En,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Ru,
    En,
}
impl Locale {
    pub fn key(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }
}
pub fn resolve_locale(language: LanguagePreference, system_locale: Option<&str>) -> Locale {
    match language {
        LanguagePreference::Ru => Locale::Ru,
        LanguagePreference::En => Locale::En,
        LanguagePreference::System => {
            if system_locale
                .and_then(|v| v.split(['-', '_']).next())
                .is_some_and(|v| v.eq_ignore_ascii_case("ru"))
            {
                Locale::Ru
            } else {
                Locale::En
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct AppPreferences {
    pub language: LanguagePreference,
    pub locale: Locale,
    pub support_url: Option<String>,
}
#[derive(Default, Deserialize, Serialize)]
struct SavedPreferences {
    #[serde(default)]
    language: LanguagePreference,
}
pub struct PreferencesStore {
    path: PathBuf,
    language: LanguagePreference,
    locale: Locale,
    invalid: bool,
}
impl PreferencesStore {
    pub fn open(path: PathBuf) -> Self {
        let (saved, invalid) = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<SavedPreferences>(&bytes) {
                Ok(value) => (value, false),
                Err(_) => (SavedPreferences::default(), true),
            },
            Err(_) => (SavedPreferences::default(), false),
        };
        let locale = resolve_locale(saved.language, sys_locale::get_locale().as_deref());
        Self {
            path,
            language: saved.language,
            locale,
            invalid,
        }
    }
    pub fn get(&self) -> AppPreferences {
        AppPreferences {
            language: self.language,
            locale: self.locale,
            support_url: crate::localization::support_url(self.locale),
        }
    }
    pub fn set(&mut self, language: LanguagePreference) -> AppResult<AppPreferences> {
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            let parent = self.path.parent().ok_or("invalid preferences path")?;
            fs::create_dir_all(parent)?;
            if self.invalid {
                let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
                fs::copy(
                    &self.path,
                    parent.join(format!("preferences.invalid-{stamp}.json")),
                )?;
            }
            let bytes = serde_json::to_vec_pretty(&SavedPreferences { language })?;
            AtomicFile::new(&self.path, OverwriteBehavior::AllowOverwrite).write(|file| {
                use std::io::Write;
                file.write_all(&bytes)?;
                file.sync_all()
            })?;
            Ok(())
        };
        save().map_err(|_| {
            AppError::new(
                "preferences_write_failed",
                "Не удалось сохранить язык приложения",
            )
        })?;
        self.language = language;
        self.locale = resolve_locale(language, sys_locale::get_locale().as_deref());
        self.invalid = false;
        Ok(self.get())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn основной_язык_и_ручной_выбор() {
        for (tag, expected) in [
            (Some("ru-RU"), Locale::Ru),
            (Some("RU_ru"), Locale::Ru),
            (Some("en-US"), Locale::En),
            (Some("de-DE"), Locale::En),
            (None, Locale::En),
        ] {
            assert_eq!(resolve_locale(LanguagePreference::System, tag), expected);
        }
        assert_eq!(
            resolve_locale(LanguagePreference::Ru, Some("en-US")),
            Locale::Ru
        );
    }
    #[test]
    fn сохранение_повреждённой_копии_и_восстановление_выбора() {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("preferences.json");
        fs::write(&path, b"{broken").expect("write");
        let mut store = PreferencesStore::open(path.clone());
        assert_eq!(store.get().language, LanguagePreference::System);
        store.set(LanguagePreference::En).expect("save");
        assert_eq!(
            PreferencesStore::open(path).get().language,
            LanguagePreference::En
        );
        assert_eq!(fs::read_dir(temp.path()).expect("list").count(), 2);
    }
    #[test]
    fn ошибка_записи_не_меняет_выбор() {
        let temp = tempfile::tempdir().expect("temp");
        let parent = temp.path().join("file");
        fs::write(&parent, "blocked").expect("write");
        let mut store = PreferencesStore::open(parent.join("preferences.json"));
        assert!(store.set(LanguagePreference::Ru).is_err());
        assert_eq!(store.get().language, LanguagePreference::System);
    }
}
