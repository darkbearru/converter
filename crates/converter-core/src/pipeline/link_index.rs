use crate::{mdict::collation::mdx_sort_key, model::*};
use rusqlite::{Connection, params};
use std::path::Path;

pub(super) fn sql_error(error: rusqlite::Error) -> ConverterError {
    ConverterError::new(
        "spool_database",
        format!("Ошибка промежуточного индекса: {error}"),
    )
}

pub(super) struct LinkIndex {
    connection: Connection,
}
impl LinkIndex {
    pub fn new(path: &Path) -> Result<Self> {
        let connection = Connection::open(path).map_err(sql_error)?;
        connection.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=FILE; PRAGMA cache_size=-1024; CREATE TABLE keys (normalized TEXT PRIMARY KEY) WITHOUT ROWID; CREATE TABLE refs (target TEXT NOT NULL, normalized TEXT NOT NULL, line BLOB NOT NULL); BEGIN;").map_err(sql_error)?;
        Ok(Self { connection })
    }
    pub fn key(&self, key: &str) -> Result<()> {
        self.connection
            .prepare_cached("INSERT OR IGNORE INTO keys VALUES (?1)")
            .map_err(sql_error)?
            .execute([mdx_sort_key(key)])
            .map_err(sql_error)?;
        Ok(())
    }
    pub fn references(&self, html: &str, line: u64) -> Result<()> {
        for part in html.split("href=\"entry://").skip(1) {
            let Some((encoded, _)) = part.split_once('"') else {
                continue;
            };
            let target = decode_uri(encoded)?;
            self.connection
                .prepare_cached("INSERT INTO refs VALUES (?1, ?2, ?3)")
                .map_err(sql_error)?
                .execute(params![
                    target,
                    mdx_sort_key(&target),
                    &line.to_be_bytes()[..]
                ])
                .map_err(sql_error)?;
        }
        Ok(())
    }
    pub fn missing(
        &self,
        ctx: &RunContext,
        source: &str,
        mut emit: impl FnMut(Diagnostic) -> Result<()>,
    ) -> Result<()> {
        ctx.control.check()?;
        let control = ctx.control.clone();
        self.connection
            .progress_handler(1000, Some(move || control.check().is_err()))
            .map_err(sql_error)?;
        self.connection.execute_batch("COMMIT;").map_err(|error| {
            ctx.control
                .check()
                .err()
                .unwrap_or_else(|| sql_error(error))
        })?;
        let mut query = self.connection.prepare("SELECT target, line FROM refs WHERE NOT EXISTS (SELECT 1 FROM keys WHERE keys.normalized=refs.normalized)").map_err(sql_error)?;
        let mut rows = query.query([]).map_err(sql_error)?;
        while let Some(row) = rows.next().map_err(|error| {
            ctx.control
                .check()
                .err()
                .unwrap_or_else(|| sql_error(error))
        })? {
            ctx.control.check()?;
            let target: String = row.get(0).map_err(sql_error)?;
            let bytes: Vec<u8> = row.get(1).map_err(sql_error)?;
            let line = u64::from_be_bytes(bytes.try_into().map_err(|_| {
                ConverterError::new("corrupt_spool", "Повреждён номер строки в индексе ссылок")
            })?);
            emit(Diagnostic {
                code: "missing_reference".into(),
                message: format!("Не найдена статья для ссылки: {target}"),
                severity: DiagnosticSeverity::Warning,
                occurrences: 1,
                source_location: Some(SourceLocation {
                    source: source.into(),
                    line,
                }),
            })?;
        }
        Ok(())
    }
}

fn decode_uri(encoded: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut input = encoded.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = input.next().and_then(|n| (n as char).to_digit(16));
            let low = input.next().and_then(|n| (n as char).to_digit(16));
            match (high, low) {
                (Some(high), Some(low)) => bytes.push((high * 16 + low) as u8),
                _ => {
                    return Err(ConverterError::new(
                        "invalid_reference",
                        "Неправильная кодировка внутренней ссылки",
                    ));
                }
            }
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes)
        .map_err(|_| ConverterError::new("invalid_reference", "Цель ссылки не является UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn отменяет_длительную_проверку_существующих_ссылок()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let index = LinkIndex::new(&temp.path().join("links.sqlite"))?;
        index.key("exists")?;
        for _ in 0..500_000 {
            index.references("<a href=\"entry://exists\">ссылка</a>", 1)?;
        }
        let ctx = RunContext::new(temp.path().to_owned());
        let control = ctx.control.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            control.cancel();
        });
        let result = index.missing(&ctx, "source.dsl", |_| Ok(()));
        worker.join().expect("Сигнал отмены");
        assert_eq!(
            result
                .expect_err("SQL-проверка должна реагировать на отмену")
                .code,
            "cancelled"
        );
        Ok(())
    }
}
