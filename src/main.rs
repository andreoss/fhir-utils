use clap::{Parser, Subcommand};
use fhir_utils::cli::{run_convert, ConvertRequest};
use fhir_utils::{Contract, Error};
use std::fs;
use std::path::PathBuf;
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
    /// Convert delimited or fixed-width records to FHIR resources
    Convert {
        /// Base directory holding input/ and config/
        #[arg(short = 'd', long, conflicts_with = "file")]
        directory: Option<PathBuf>,
        /// Single input file; requires --config-dir
        #[arg(short = 'f', long, requires = "config_dir")]
        file: Option<PathBuf>,
        /// Configuration directory holding the data contract
        #[arg(short = 'c', long)]
        config_dir: Option<PathBuf>,
        /// Output directory for FHIR resources
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Validate { file } => run_validate(&file),
        Commands::Convert {
            directory,
            file,
            config_dir,
            output,
        } => run_conversion(directory, file, config_dir, output),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run_validate(file: &str) -> Result<(), Error> {
    let content = fs::read_to_string(file)?;
    Contract::load(&content)?;
    println!("Contract is valid");
    Ok(())
}

fn run_conversion(
    directory: Option<PathBuf>,
    file: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    output: PathBuf,
) -> Result<(), Error> {
    if directory.is_none() && file.is_none() {
        return Err(Error::Config("convert needs -d or -f".into()));
    }

    let summary = run_convert(&ConvertRequest {
        base: directory,
        file,
        config_dir,
        output,
    })?;
    println!(
        "Converted {} file(s), wrote {} resource(s), skipped {} file(s)",
        summary.files, summary.resources, summary.skipped
    );
    Ok(())
}
