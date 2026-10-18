mod convert;
mod fetch;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Локальный конвертер словарей DSL в MDX + MDD")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Конвертировать DSL, DSL.DZ или ZIP")]
    Convert {
        #[arg(long, help = "Основной словарь или ZIP-пакет")]
        #[arg(required_unless_present = "url", conflicts_with = "url")]
        source: Option<PathBuf>,
        #[arg(long, help = "Прямая HTTP/HTTPS-ссылка", conflicts_with = "source")]
        url: Option<String>,
        #[arg(
            long,
            help = "Ссылка на ZIP ресурсов; можно повторять",
            requires = "url"
        )]
        resources_url: Vec<String>,
        #[arg(long, help = "Постоянный кэш скачивания", requires = "url")]
        cache: Option<PathBuf>,
        #[arg(long, help = "Ожидаемый SHA-256 основного источника", requires = "url")]
        sha256: Option<String>,
        #[arg(
            long,
            help = "Папка, ZIP ресурсов или словарь сокращений; можно повторять"
        )]
        resources: Vec<PathBuf>,
        #[arg(long, help = "Папка для готовых результатов")]
        output: PathBuf,
        #[arg(
            long,
            value_enum,
            default_value = "auto",
            help = "Кодировка входного словаря"
        )]
        encoding: Encoding,
        #[arg(
            long,
            value_enum,
            default_value = "desktop",
            help = "Профиль ограничений памяти"
        )]
        profile: Profile,
    },
    #[command(about = "Скачать источник или продолжить сохранённую загрузку")]
    Fetch {
        #[arg(
            long,
            required_unless_present = "source_id",
            conflicts_with = "source_id"
        )]
        url: Option<String>,
        #[arg(long, conflicts_with = "url")]
        source_id: Option<String>,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long, requires = "url")]
        sha256: Option<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum Encoding {
    Auto,
    Utf8,
    Utf16le,
    Utf16be,
    Windows1251,
}
#[derive(Clone, ValueEnum)]
enum Profile {
    Desktop,
    Mobile,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let conversion_control = converter_core::model::RunControl::default();
    let download_control = converter_download::TransferControl::default();
    let (conversion_signal, download_signal) =
        (conversion_control.clone(), download_control.clone());
    if ctrlc::set_handler(move || {
        conversion_signal.cancel();
        download_signal.cancel();
    })
    .is_err()
    {
        eprintln!("Не удалось включить обработчик Ctrl-C");
        return std::process::ExitCode::FAILURE;
    }
    let result = match cli.command {
        Command::Convert {
            source,
            url,
            resources_url,
            cache,
            sha256,
            resources,
            output,
            encoding,
            profile,
        } => {
            let prepared = if let Some(url) = url {
                match cache {
                    Some(cache) => {
                        fetch::sources(url, resources_url, sha256, cache, download_control).await
                    }
                    None => Err(converter_core::model::ConverterError::new(
                        "cache_required",
                        "Для URL укажите постоянный кэш --cache",
                    )),
                }
            } else {
                source
                    .map(|path| convert::prepared(path, "main".into()))
                    .transpose()
                    .and_then(|file| {
                        file.ok_or_else(|| {
                            converter_core::model::ConverterError::new(
                                "source_required",
                                "Выберите источник",
                            )
                        })
                    })
                    .map(|file| vec![file])
            };
            match prepared {
                Ok(mut files) => {
                    let source = files.remove(0);
                    match resources
                        .into_iter()
                        .enumerate()
                        .map(|(i, path)| convert::prepared(path, format!("local-resource-{i}")))
                        .collect::<converter_core::model::Result<Vec<_>>>()
                    {
                        Ok(local) => {
                            files.extend(local);
                            convert::run(
                                source,
                                files,
                                output,
                                encoding,
                                profile,
                                conversion_control,
                            )
                        }
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            }
        }
        Command::Fetch {
            url,
            source_id,
            cache,
            sha256,
        } => fetch::run(url, source_id, sha256, cache, download_control).await,
    };
    match result {
        Ok(value) => {
            println!("{value}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Ошибка: {}", error.message.escape_debug());
            println!("{}", serde_json::json!({"error": error}));
            std::process::ExitCode::from(if error.code == "cancelled" { 130 } else { 1 })
        }
    }
}
