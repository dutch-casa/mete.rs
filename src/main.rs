mod cli;
mod commands;

#[cfg(feature = "mcp")]
mod mcp;

#[cfg(feature = "mcp")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use cli::Commands;

    let runner = cli::CliRunner::new();

    // Handle MCP command specially (async)
    if let Some(Commands::Mcp {}) = &runner.cli.command {
        mcp::run_server().await?;
        return Ok(());
    }

    // All other commands run synchronously
    runner.run()
}

#[cfg(not(feature = "mcp"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    cli::CliRunner::new().run()
}
