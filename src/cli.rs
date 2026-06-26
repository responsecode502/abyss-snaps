use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "abyss-snaps", version = "0.1.2")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn parse_args() -> Self {
        <Self as clap::Parser>::parse()
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Run,
    Rollback {
        #[arg(short, long)]
        hash: String,
    },
}
