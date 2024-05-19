use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    #[arg(long, value_name = "CLIENT_JAR")]
    pub client_jar: PathBuf,
    #[arg(long, value_name = "SCHEMATIC_FILE")]
    pub schem_file: PathBuf,
}

pub fn parse() -> Cli {
    Cli::parse()
}
