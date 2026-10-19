use crate::{
    localization::{native_format, native_text},
    preferences::Locale,
};
use converter_app::{AppError, AppResult, JobSnapshot};
use converter_core::model::{Diagnostic, DiagnosticSeverity};
use url::Url;

// Ограничиваем именно закодированный URL, чтобы большой журнал не приводил к HTTP 414.
const MAX_URL_BYTES: usize = 7000;

pub fn issue_url(
    locale: Locale,
    version: &str,
    os: &str,
    arch: &str,
    job: Option<&JobSnapshot>,
    diagnostics: &[Diagnostic],
) -> AppResult<String> {
    let os = match os {
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    };
    let mut title = native_text(locale, "issues.title");
    let mut body = native_format(
        locale,
        "issues.body",
        &[
            ("version", version.into()),
            ("os", os.into()),
            ("arch", arch.into()),
        ],
    );
    if let Some(job) = job {
        // До подготовки display_name удалённой задачи может содержать hostname.
        let name = if job.download.kind == converter_app::DownloadKind::Remote {
            native_text(locale, "issues.remote")
        } else {
            filename(&job.display_name, locale)
        };
        title = native_format(locale, "issues.conversion_title", &[("name", name.clone())]);
        body.push_str(&native_format(
            locale,
            "issues.job",
            &[
                ("name", name),
                ("state", format!("{:?}", job.state).to_lowercase()),
                (
                    "code",
                    job.error
                        .as_ref()
                        .or(job.export.error.as_ref())
                        .map(|error| code(&error.code))
                        .unwrap_or_else(|| native_text(locale, "issues.none")),
                ),
                ("warnings", job.diagnostic_count.to_string()),
            ],
        ));
        if let Some(result) = &job.result {
            body.push_str(&native_format(
                locale,
                "issues.summary",
                &[
                    ("articles", result.summary.source_articles.to_string()),
                    ("keys", result.summary.output_keys.to_string()),
                    ("resources", result.summary.resources.to_string()),
                ],
            ));
        }
        let mut lines = String::new();
        let mut shown = 0;
        for diagnostic in diagnostics
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Warning)
        {
            let line = format!(
                "- `{}` × {}\n",
                code(&diagnostic.code),
                diagnostic.occurrences
            );
            let candidate = format!(
                "{body}{}{}",
                diagnostic_note(job, shown + 1, locale),
                lines.clone() + &line
            );
            if encode_url(&title, &candidate)?.len() > MAX_URL_BYTES {
                break;
            }
            shown += 1;
            lines.push_str(&line);
        }
        body.push_str(&diagnostic_note(job, shown, locale));
        body.push_str(&lines);
    }
    encode_url(&title, &body)
}

fn diagnostic_note(job: &JobSnapshot, shown: usize, locale: Locale) -> String {
    let mut note = String::new();
    if job.diagnostic_count > 0 {
        note.push_str(&native_format(
            locale,
            "issues.diagnostics",
            &[
                ("shown", shown.to_string()),
                ("total", job.diagnostic_count.to_string()),
            ],
        ));
        if shown == 0 {
            note.push_str(&native_text(locale, "issues.details"));
        }
    }
    if job.result.as_ref().is_some_and(|result| {
        result
            .files
            .iter()
            .any(|file| file.name == "conversion-report.json")
    }) {
        note.push_str(&native_text(locale, "issues.report"));
    }
    note
}

fn filename(value: &str, locale: Locale) -> String {
    // Имя сохраняем, но не передаём путь, параметры URL или содержимое статей.
    let name = value
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let name: String = name
        .chars()
        .filter(|character| !character.is_control() && *character != '`')
        .take(60)
        .collect();
    if name.is_empty() {
        native_text(locale, "issues.dictionary")
    } else {
        name
    }
}

fn code(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
        .take(64)
        .collect()
}

fn encode_url(title: &str, body: &str) -> AppResult<String> {
    let mut url =
        Url::parse("https://github.com/darkbearru/converter/issues/new").map_err(|_| {
            AppError::new(
                "issue_url_failed",
                "Не удалось подготовить сообщение о проблеме",
            )
            .with_message(
                "messages.issue_url_failed",
                converter_core::model::message_args([]),
            )
        })?;
    url.query_pairs_mut()
        .append_pair("title", title)
        .append_pair("body", body);
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use converter_app::{AppError, JobSnapshot, JobState};
    use converter_core::model::{Diagnostic, DiagnosticSeverity};
    use url::Url;

    fn job() -> JobSnapshot {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/ui/fixtures/contracts.json"))
                .expect("Корректный JSON тестовых контрактов");
        serde_json::from_value(fixtures["succeeded"].clone()).expect("Снимок успешного задания")
    }
    fn body(link: &str) -> String {
        Url::parse(link)
            .expect("Корректная ссылка на форму GitHub")
            .query_pairs()
            .find(|(key, _)| key == "body")
            .expect("Параметр body заполнен")
            .1
            .into_owned()
    }
    #[test]
    fn общий_отчёт_открывает_форму_с_версией_и_ос() {
        let link =
            issue_url(Locale::Ru, "0.2.0", "macos", "aarch64", None, &[]).expect("Общий отчёт");
        let parsed = Url::parse(&link).expect("Корректная ссылка на форму");
        assert_eq!(parsed.host_str(), Some("github.com"));
        assert_eq!(parsed.path(), "/darkbearru/converter/issues/new");
        assert_eq!(parsed.query_pairs().count(), 2);
        let text = body(&link);
        for expected in [
            "0.2.0",
            "macOS",
            "aarch64",
            "Описание проблемы",
            "Шаги воспроизведения",
        ] {
            assert!(text.contains(expected), "Нет {expected}");
        }
    }
    #[test]
    fn отчёт_задачи_содержит_код_ошибки_и_диагностику_без_личных_путей() {
        let mut job = job();
        job.state = JobState::Failed;
        job.download.kind = converter_app::DownloadKind::Local;
        job.result = None;
        job.display_name = "C:\\Users\\Alexey\\secret\\main.dsl?token=private#fragment".into();
        job.error = Some(AppError::new(
            "invalid_markup",
            "Ошибка /Users/Alexey/private https://site.test/?token=secret",
        ));
        job.diagnostic_count = 50;
        let diagnostic = Diagnostic {
            message_key: None,
            message_args: Default::default(),
            code: "missing_reference".into(),
            message: "Секретное содержимое словаря".into(),
            source_location: None,
            severity: DiagnosticSeverity::Warning,
            occurrences: 42,
        };
        let link = issue_url(
            Locale::Ru,
            "0.2.0",
            "windows",
            "x86_64",
            Some(&job),
            &[diagnostic],
        )
        .expect("Отчёт об ошибке задания");
        let text = body(&link);
        for expected in [
            "Windows",
            "main.dsl",
            "failed",
            "invalid_markup",
            "missing_reference",
            "42",
            "50",
        ] {
            assert!(text.contains(expected), "Нет {expected}");
        }
        for private in [
            "Alexey",
            "secret",
            "private",
            "token",
            "fragment",
            "Секретное",
            "site.test",
        ] {
            assert!(
                !link.contains(private) && !text.contains(private),
                "Раскрыто {private}"
            );
        }
        assert!(
            !text.contains("conversion-report.json"),
            "Отчёта при ошибке нет"
        );
    }
    #[test]
    fn большой_журнал_не_переполняет_ссылку_и_не_теряется_молча() {
        let mut job = job();
        job.display_name = "非常長的словарь".repeat(1000);
        job.diagnostic_count = 200_000;
        let diagnostics: Vec<_> = (0..200)
            .map(|index| Diagnostic {
                message_key: None,
                message_args: Default::default(),
                code: format!("missing_reference_{index}"),
                message: "о".repeat(10_000),
                source_location: None,
                severity: DiagnosticSeverity::Warning,
                occurrences: 1,
            })
            .collect();
        let link = issue_url(
            Locale::Ru,
            "0.2.0",
            "macos",
            "aarch64",
            Some(&job),
            &diagnostics,
        )
        .expect("Сводка большого журнала");
        assert!(link.len() <= 7000, "Ссылка слишком длинная: {}", link.len());
        let text = body(&link);
        assert!(text.contains("200000"));
        assert!(text.contains("conversion-report.json"));
        assert!(text.contains("Показано"));
        assert!(text.contains("Описание проблемы"));
        assert!(text.contains("articles") || text.contains("Статей"));
    }
    #[test]
    fn отменённая_загрузка_не_передаёт_домен_источника() {
        let mut job = job();
        job.state = JobState::Cancelled;
        job.download.kind = converter_app::DownloadKind::Remote;
        job.source_bytes = None;
        job.display_name = "private.company.local".into();
        job.result = None;
        for source_bytes in [None, Some(1234)] {
            job.source_bytes = source_bytes;
            let link = issue_url(Locale::Ru, "0.2.0", "macos", "aarch64", Some(&job), &[])
                .expect("Отчёт об отменённой загрузке");
            assert!(!link.contains("private.company.local"));
            assert!(body(&link).contains("Словарь по ссылке"));
        }
    }
}

#[cfg(test)]
mod locale_tests {
    use super::*;
    #[test]
    fn английский_отчёт_содержит_описание_и_версию() {
        let link = issue_url(Locale::En, "0.2.7", "windows", "x86_64", None, &[]).expect("issue");
        let url = Url::parse(&link).expect("URL");
        let body = url
            .query_pairs()
            .find(|(key, _)| key == "body")
            .expect("body")
            .1
            .into_owned();
        assert!(
            body.contains("Problem description")
                && body.contains("Windows")
                && body.contains("0.2.7")
        );
        assert!(!body.contains("Описание"));
    }
}
