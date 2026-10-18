//! Команды stdin: release — продолжить передачу; requests — напечатать Range/If-Range.
use converter_test_support::http::{FixtureServer, Scenario};
use std::io::BufRead;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("Укажите ZIP fixture")?;
    let server = FixtureServer::spawn(
        Scenario::DisconnectThenHoldAt(1_048_576),
        std::fs::read(path)?,
    )
    .await;
    println!("{}", server.url());
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            line = receiver.recv() => { match line.as_deref() {
                Some("release") => { server.release(); println!("released"); },
                Some("requests") => { for request in server.requests() { println!("{} range={:?} if-range={:?}", request.method, request.range_start, request.headers.get("if-range")); } },
                None => break,
                _ => (),
            } }
        }
    }
    Ok(())
}
