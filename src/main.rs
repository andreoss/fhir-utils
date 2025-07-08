use clap::{Parser, Subcommand};
use fhir_utils::{Contract, Error};
use std::fs;
use std::process;

#[derive(Parser)]
#[command(name = "fhir-utils", version, about = "FHIR conversion utility")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a data contract file
    Validate {
        /// Path to the contract JSON file
        #[arg(short, long)]
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file } => {
            if let Err(e) = run_validate(&file) {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }
}

fn run_validate(file: &str) -> Result<(), Error> {
    let content = fs::read_to_string(file)?;
    Contract::load(&content)?;
    println!("Contract is valid");
    Ok(())
}
