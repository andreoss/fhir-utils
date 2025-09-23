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
    #[command(about = "Validate a data contract file")]
    Validate {
        #[arg(short, long, help = "Path to the contract JSON file")]
        file: String,
    },
    #[command(about = "Convert delimited or fixed-width records to FHIR resources")]
    Convert {
        #[arg(
            short = 'd',
            long,
            conflicts_with = "file",
            help = "Base directory holding input/ and config/"
        )]
        directory: Option<PathBuf>,
        #[arg(
            short = 'f',
            long,
            requires = "config_dir",
            help = "Single input file; requires --config-dir"
        )]
        file: Option<PathBuf>,
        #[arg(
            short = 'c',
            long,
            help = "Configuration directory holding the data contract"
        )]
        config_dir: Option<PathBuf>,
        #[arg(short, long, help = "Output directory for FHIR resources")]
        output: PathBuf,
        #[arg(long, help = "Fail the run on a task error or an empty group key")]
        strict: bool,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Validate { file } => run_validate(&file),
        Commands::Convert {
            directory,
            file,
            config_dir,
            output,
            strict,
        } => run_conversion(directory, file, config_dir, output, strict),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn skipped_detail(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    let rest = names.len().saturating_sub(shown.len());
    if rest > 0 {
        format!(": {} and {rest} more", shown.join(", "))
    } else {
        format!(": {}", shown.join(", "))
    }
}

fn run_validate(file: &str) -> Result<(), Error> {
    let path = PathBuf::from(file);
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(directory) = directory {
        fhir_utils::opener::set_opener(std::sync::Arc::new(fhir_utils::opener::LocalOpener::new(
            directory,
        )));
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| Error::Config(format!("cannot open {file}: {error}")))?;
    Contract::load(&content)?;
    println!("Contract is valid");
    Ok(())
}

fn run_conversion(
    directory: Option<PathBuf>,
    file: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    output: PathBuf,
    strict: bool,
) -> Result<(), Error> {
    if directory.is_none() && file.is_none() {
        return Err(Error::Config("convert needs -d or -f".into()));
    }

    let summary = run_convert(&ConvertRequest {
        base: directory,
        file,
        config_dir,
        output,
        opener: None,
        strict,
    })?;
    println!(
        "Converted {} file(s), wrote {} resource(s), skipped {} file(s){}",
        summary.files,
        summary.resources,
        summary.skipped(),
        skipped_detail(&summary.skipped_files)
    );
    Ok(())
}
