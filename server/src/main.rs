use data_fabrication_server::DataFabricationServer;
use platform_challenge_sdk::server::ChallengeServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let challenge = DataFabricationServer::default();

    ChallengeServer::builder(challenge)
        .from_env()
        .build()
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))
}
