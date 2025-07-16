mod config;
mod contract;
mod default_tasks;
mod error;
mod integration_tests;
mod lookup;
mod reader;
mod tasks;

pub use config::Config;
pub use contract::{Contract, FileDefinition, General};
pub use default_tasks::{build_default_task_chain, execute_default_and_user_tasks};
pub use error::Error;
pub use lookup::lookup_file_definition;
pub use reader::{read_file, ReaderParams, RecordBatch};
pub use tasks::{execute_task_chain, TaskRegistry};
