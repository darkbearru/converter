use converter_app::{AppError, AppResult, JobSnapshot};
use converter_core::model::{Diagnostic, DiagnosticSeverity};
use url::Url;

// Ограничиваем именно закодированный URL, чтобы большой журнал не приводил к HTTP 414.
const MAX_URL_BYTES: usize = 7000;

pub fn issue_url(
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
    let mut title = "Проблема в Converter".to_owned();
    let mut body = format!(
        "### Описание проблемы\n\n\n### Шаги воспроизведения\n\n1. \n\n### Ожидаемый результат\n\n\n### Сведения приложения\n\n- Converter: {version}\n- ОС: {os} ({arch})\n"
    );
    if let Some(job) = job {
        // До подготовки display_name удалённой задачи может содержать hostname.
        let name = if job.download.kind == converter_app::DownloadKind::Remote {
            "Словарь по ссылке".into()
        } else {
            filename(&job.display_name)
        };
        title = format!("Проблема конвертации: {name}");
        body.push_str(&format!(
            "- Файл: `{name}`\n- Состояние: {}\n- Код ошибки: {}\n- Групп предупреждений: {}\n",
            format!("{:?}", job.state).to_lowercase(),
            job.error
                .as_ref()
                .or(job.export.error.as_ref())
                .map(|error| code(&error.code))
                .unwrap_or_else(|| "нет".into()),
            job.diagnostic_count,
        ));
        if let Some(result) = &job.result {
            body.push_str(&format!(
                "- Статей: {}; ключей: {}; ресурсов: {}\n",
                result.summary.source_articles,
                result.summary.output_keys,
                result.summary.resources,
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
                diagnostic_note(job, shown + 1),
                lines.clone() + &line
            );
            if encode_url(&title, &candidate)?.len() > MAX_URL_BYTES {
                break;
            }
            shown += 1;
            lines.push_str(&line);
        }
        body.push_str(&diagnostic_note(job, shown));
        body.push_str(&lines);
    }
    encode_url(&title, &body)
}

fn diagnostic_note(job: &JobSnapshot, shown: usize) -> String {
    let mut note = String::new();
    if job.diagnostic_count > 0 {
        note.push_str(&format!("\n### Краткая диагностика\n\nПоказано {shown} из {} групп (коды и число употреблений).\n", job.diagnostic_count));
        if shown == 0 {
            note.push_str("Подробности доступны в окне предупреждений приложения.\n");
        }
    }
    if job.result.as_ref().is_some_and(|result| {
        result
            .files
            .iter()
            .any(|file| file.name == "conversion-report.json")
    }) {
        note.push_str("\nПолный журнал: приложите conversion-report.json из папки результата вручную. Перед публикацией проверьте его содержимое.\n");
    }
    note
}

fn filename(value: &str) -> String {
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
        "Словарь".into()
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
        let link = issue_url("0.2.0", "macos", "aarch64", None, &[]).expect("Общий отчёт");
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
            code: "missing_reference".into(),
            message: "Секретное содержимое словаря".into(),
            source_location: None,
            severity: DiagnosticSeverity::Warning,
            occurrences: 42,
        };
        let link = issue_url("0.2.0", "windows", "x86_64", Some(&job), &[diagnostic])
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
                code: format!("missing_reference_{index}"),
                message: "о".repeat(10_000),
                source_location: None,
                severity: DiagnosticSeverity::Warning,
                occurrences: 1,
            })
            .collect();
        let link = issue_url("0.2.0", "macos", "aarch64", Some(&job), &diagnostics)
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
            let link = issue_url("0.2.0", "macos", "aarch64", Some(&job), &[])
                .expect("Отчёт об отменённой загрузке");
            assert!(!link.contains("private.company.local"));
            assert!(body(&link).contains("Словарь по ссылке"));
        }
    }
}
