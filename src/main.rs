mod cli;
mod commands;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    cli::CliRunner::new().run()
}
