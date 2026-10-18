#[path = "../tests/support/large_fixture.rs"]
mod fixture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments.next().ok_or("Укажите путь нового DSL-файла")?;
    let count = arguments
        .next()
        .map(|value| value.to_string_lossy().parse::<u64>())
        .transpose()?
        .unwrap_or(1_000_000);
    let size = fixture::generate(std::path::Path::new(&path), count)?;
    println!(
        "Создано статей: {}, размер: {} байт",
        size.articles, size.bytes
    );
    Ok(())
}
