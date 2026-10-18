mod convert;

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
        source: PathBuf,
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

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Convert {
            source,
            resources,
            output,
            encoding,
            profile,
        } => convert::run(source, resources, output, encoding, profile),
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
