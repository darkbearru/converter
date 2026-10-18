use converter_core::{
    model::{PreparedFile, RunContext},
    prepare::{prepare_source, resolve_resource},
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write, path::Path};
use zip::{ZipWriter, write::SimpleFileOptions};

fn prepared(path: &Path) -> PreparedFile {
    PreparedFile {
        id: "source".into(),
        display_name: path
            .file_name()
            .expect("Имя источника")
            .to_string_lossy()
            .into_owned(),
        path: path.into(),
    }
}

fn archive(path: &Path, files: &[(&str, &[u8])]) -> Result<(), Box<dyn std::error::Error>> {
    let mut zip = ZipWriter::new(File::create(path)?);
    for (name, bytes) in files {
        zip.start_file(*name, SimpleFileOptions::default())?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    Ok(())
}

#[test]
fn архив_сохраняет_логические_имена() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("source.zip");
    archive(
        &path,
        &[
            ("first.dsl", b"#NAME \"First\"\none\n body"),
            ("first_abrv.dsl", b"x\n abbreviation"),
            ("other/second.dsl", b"two\n body"),
            ("media/image.png", b"PNG"),
            ("AUX.png", b"reserved"),
            ("a\\b.wav", b"sound"),
        ],
    )?;
    let before = Sha256::digest(std::fs::read(&path)?);
    let ctx = RunContext::new(temp.path().join("work"));
    let bundles = prepare_source(&prepared(&path), &[], &ctx)?;
    assert_eq!(bundles.len(), 2);
    let first = bundles
        .iter()
        .find(|b| b.main.display_name == "first.dsl")
        .ok_or("Первый словарь")?;
    assert!(first.abbreviations.is_some());
    let resource = resolve_resource(first, "AUX.png")?.ok_or("Ресурс AUX")?;
    assert_eq!(std::fs::read(&resource.path)?, b"reserved");
    assert_ne!(
        resource.path.file_name().and_then(|s| s.to_str()),
        Some("AUX.png")
    );
    assert!(resolve_resource(first, "a/b.wav")?.is_some());
    assert!(resolve_resource(first, "missing.png")?.is_none());
    assert_eq!(before, Sha256::digest(std::fs::read(&path)?));
    Ok(())
}

#[test]
fn не_теряет_коллизии_и_не_выходит_из_архива() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    for (index, name) in ["../x", "C:\\x", "\\\\host\\x", "/tmp/x", "a/../../x"]
        .into_iter()
        .enumerate()
    {
        let path = temp.path().join(format!("unsafe-{index}.zip"));
        archive(&path, &[("main.dsl", b"one\n body"), (name, b"unsafe")])?;
        let error = prepare_source(
            &prepared(&path),
            &[],
            &RunContext::new(temp.path().join(format!("work-{index}"))),
        )
        .expect_err("Нужна ошибка пути");
        assert_eq!(error.code, "unsafe_archive_path", "{name}");
    }
    let path = temp.path().join("ambiguous.zip");
    archive(
        &path,
        &[
            ("main.dsl", b"one\n body"),
            ("image.png", b"one"),
            ("IMAGE.png", b"two"),
        ],
    )?;
    assert_eq!(
        prepare_source(
            &prepared(&path),
            &[],
            &RunContext::new(temp.path().join("ambiguous"))
        )
        .expect_err("Неоднозначные имена")
        .code,
        "ambiguous_resource"
    );
    let path = temp.path().join("unicode.zip");
    archive(
        &path,
        &[
            ("main.dsl", b"one\n body"),
            ("café.png", b"one"),
            ("cafe\u{301}.png", b"two"),
        ],
    )?;
    assert_eq!(
        prepare_source(
            &prepared(&path),
            &[],
            &RunContext::new(temp.path().join("unicode"))
        )
        .expect_err("NFC-коллизия")
        .code,
        "ambiguous_resource"
    );
    Ok(())
}

#[test]
fn соблюдает_лимиты_и_отмену() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("source.zip");
    archive(
        &path,
        &[("main.dsl", b"one\n body"), ("image.png", b"large")],
    )?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.max_unpacked_bytes = 3;
    assert_eq!(
        prepare_source(&prepared(&path), &[], &ctx)
            .expect_err("Лимит распаковки")
            .code,
        "archive_limit"
    );
    ctx.control.cancel();
    assert_eq!(
        prepare_source(&prepared(&path), &[], &ctx)
            .expect_err("Отмена")
            .code,
        "cancelled"
    );
    Ok(())
}

#[test]
fn отклоняет_ссылки_вложенные_и_зашифрованные_zip() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let link = temp.path().join("link.zip");
    let mut zip = ZipWriter::new(File::create(&link)?);
    zip.start_file("main.dsl", SimpleFileOptions::default())?;
    zip.write_all(b"one\n body")?;
    zip.add_symlink("media/link", "../../outside", SimpleFileOptions::default())?;
    zip.finish()?;
    assert_eq!(
        prepare_source(
            &prepared(&link),
            &[],
            &RunContext::new(temp.path().join("links"))
        )
        .expect_err("Ссылка в ZIP")
        .code,
        "unsafe_archive_path"
    );

    let nested = temp.path().join("nested.zip");
    archive(
        &nested,
        &[("main.dsl", b"one\n body"), ("archive.zip", b"PK\x05\x06")],
    )?;
    assert_eq!(
        prepare_source(
            &prepared(&nested),
            &[],
            &RunContext::new(temp.path().join("nested"))
        )
        .expect_err("Вложенный ZIP")
        .code,
        "nested_archive"
    );

    let encrypted = temp.path().join("encrypted.zip");
    archive(&encrypted, &[("main.dsl", b"one\n body")])?;
    let mut bytes = std::fs::read(&encrypted)?;
    for index in 0..bytes.len().saturating_sub(10) {
        if &bytes[index..index + 4] == b"PK\x03\x04" {
            bytes[index + 6] |= 1;
        }
        if &bytes[index..index + 4] == b"PK\x01\x02" {
            bytes[index + 8] |= 1;
        }
    }
    std::fs::write(&encrypted, bytes)?;
    assert_eq!(
        prepare_source(
            &prepared(&encrypted),
            &[],
            &RunContext::new(temp.path().join("encrypted"))
        )
        .expect_err("Зашифрованный ZIP")
        .code,
        "encrypted_archive"
    );
    Ok(())
}

#[test]
fn одинаковые_коллизии_не_удваивают_ресурс() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("equal.zip");
    archive(
        &path,
        &[
            ("main.dsl", b"one\n body"),
            ("IMAGE.png", b"same"),
            ("image.png", b"same"),
        ],
    )?;
    let bundles = prepare_source(
        &prepared(&path),
        &[],
        &RunContext::new(temp.path().join("work")),
    )?;
    assert_eq!(bundles[0].resources.len(), 1);
    assert!(resolve_resource(&bundles[0], "image.png")?.is_some());
    Ok(())
}

#[test]
fn резервное_имя_сообщается_и_папка_не_меняется() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("main.dsl.dz");
    let mut gzip =
        flate2::write::GzEncoder::new(File::create(&source)?, flate2::Compression::default());
    gzip.write_all(b"one\n body")?;
    gzip.finish()?;
    let assets = temp.path().join("assets");
    std::fs::create_dir(&assets)?;
    std::fs::write(assets.join("Image.png"), b"image")?;
    let bundles = prepare_source(
        &prepared(&source),
        &[prepared(&assets)],
        &RunContext::new(temp.path().join("work")),
    )?;
    let resource = resolve_resource(&bundles[0], "image.png")?.ok_or("Резервное сопоставление")?;
    assert_eq!(resource.key, "\\Image.png");
    assert_eq!(std::fs::read(assets.join("Image.png"))?, b"image");
    Ok(())
}

#[cfg(unix)]
#[test]
fn отклоняет_символические_ссылки_в_папке() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("main.dsl");
    std::fs::write(&source, "one\n body")?;
    let assets = temp.path().join("assets");
    std::fs::create_dir(&assets)?;
    std::os::unix::fs::symlink(&source, assets.join("link"))?;
    assert_eq!(
        prepare_source(
            &prepared(&source),
            &[prepared(&assets)],
            &RunContext::new(temp.path().join("work"))
        )
        .expect_err("Ссылка запрещена")
        .code,
        "unsafe_archive_path"
    );
    Ok(())
}
