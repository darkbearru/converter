use converter_test_support::http::{FixtureServer, Scenario};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Укажите локальный DSL или ZIP")?;
    let payload = std::fs::read(path)?;
    let server = FixtureServer::spawn(Scenario::DisconnectAt(1_048_576), payload).await;
    println!("{}", server.url());
    tokio::signal::ctrl_c().await?;
    Ok(())
}
