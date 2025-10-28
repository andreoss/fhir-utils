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
        #[arg(long, help = "Print the resolved contract with definitions inlined")]
        print: bool,
    },
    #[command(about = "Emit the normalized internal records instead of FHIR resources")]
    Transform {
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
            help = "Single input file; needs --config-dir or MAPPING_CONFIG_DIRECTORY"
        )]
        file: Option<PathBuf>,
        #[arg(
            short = 'c',
            long,
            help = "Configuration directory holding the data contract"
        )]
        config_dir: Option<PathBuf>,
        #[arg(long, help = "Fail the run on a task error or an empty group key")]
        strict: bool,
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
            help = "Single input file; needs --config-dir or MAPPING_CONFIG_DIRECTORY"
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
        #[arg(
            long,
            value_name = "TYPE",
            num_args = 0..=1,
            default_missing_value = "collection",
            help = "Write one bundle per group key: collection, batch or transaction"
        )]
        bundle: Option<String>,
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
        Commands::Validate { file, print } => run_validate(&file, print),
        Commands::Transform {
            directory,
            file,
            config_dir,
            strict,
        } => run_transformation(directory, file, config_dir, strict),
        Commands::Convert {
            directory,
            file,
            config_dir,
            output,
            strict,
            bundle,
        } => run_conversion(directory, file, config_dir, output, strict, bundle),
    };

    if let Err(error) = result {
        eprintln!("error E{}: {error}", error.code());
        process::exit(1);
    }
}

fn empty_detail(empty_rows: usize) -> String {
    if empty_rows == 0 {
        String::new()
    } else {
        format!(", {empty_rows} row(s) produced no resource")
    }
}

fn deduplicated_detail(deduplicated: usize) -> String {
    if deduplicated == 0 {
        String::new()
    } else {
        format!(", {deduplicated} duplicate resource(s) collapsed")
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

fn run_validate(file: &str, print: bool) -> Result<(), Error> {
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
    let contract = Contract::load(&content)?;
    for warning in contract.lint() {
        eprintln!("warning: {warning}");
    }
    if print {
        println!("{}", serde_json::to_string_pretty(&contract)?);
    } else {
        println!("Contract is valid");
    }
    Ok(())
}

fn run_transformation(
    directory: Option<PathBuf>,
    file: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    strict: bool,
) -> Result<(), Error> {
    if directory.is_none() && file.is_none() {
        return Err(Error::Config("transform needs -d or -f".into()));
    }

    let summary = fhir_utils::cli::run_transform(
        &ConvertRequest {
            base: directory,
            file,
            config_dir,
            output: PathBuf::new(),
            opener: None,
            strict,
            bundle: None,
        },
        &mut std::io::stdout(),
    )?;
    eprintln!(
        "Transformed {} file(s), read {} row(s), skipped {} file(s){}",
        summary.files,
        summary.rows,
        summary.skipped(),
        skipped_detail(&summary.skipped_files)
    );
    Ok(())
}

fn run_conversion(
    directory: Option<PathBuf>,
    file: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    output: PathBuf,
    strict: bool,
    bundle: Option<String>,
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
        bundle,
    })?;
    println!(
        "Converted {} file(s), read {} row(s), wrote {} resource(s){}{}, skipped {} file(s){}",
        summary.files,
        summary.rows,
        summary.resources,
        empty_detail(summary.empty_rows),
        deduplicated_detail(summary.deduplicated),
        summary.skipped(),
        skipped_detail(&summary.skipped_files)
    );
    Ok(())
}
