use once_cell::sync::Lazy;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub csv_buffer_size: usize,
    pub mapping_config_directory: String,
    pub mapping_config_file_name: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            csv_buffer_size: env::var("CSV_BUFFER_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            mapping_config_directory: env::var("MAPPING_CONFIG_DIRECTORY")
                .unwrap_or_else(|_| "/var/app/config".into()),
            mapping_config_file_name: env::var("MAPPING_CONFIG_FILE_NAME")
                .unwrap_or_else(|_| "data-contract.json".into()),
        }
    }
}

static CONFIG: Lazy<Config> = Lazy::new(Config::from_env);

pub fn config() -> &'static Config {
    &CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_default_config() {
        env::remove_var("CSV_BUFFER_SIZE");
        env::remove_var("MAPPING_CONFIG_DIRECTORY");
        env::remove_var("MAPPING_CONFIG_FILE_NAME");

        let config = Config::from_env();
        assert_eq!(config.csv_buffer_size, 1000);
        assert_eq!(config.mapping_config_directory, "/var/app/config");
        assert_eq!(config.mapping_config_file_name, "data-contract.json");
    }

    #[test]
    #[serial]
    fn test_env_override() {
        env::set_var("CSV_BUFFER_SIZE", "500");
        env::set_var("MAPPING_CONFIG_DIRECTORY", "/custom/config");
        env::set_var("MAPPING_CONFIG_FILE_NAME", "custom-contract.json");

        let config = Config::from_env();
        assert_eq!(config.csv_buffer_size, 500);
        assert_eq!(config.mapping_config_directory, "/custom/config");
        assert_eq!(config.mapping_config_file_name, "custom-contract.json");

        env::remove_var("CSV_BUFFER_SIZE");
        env::remove_var("MAPPING_CONFIG_DIRECTORY");
        env::remove_var("MAPPING_CONFIG_FILE_NAME");
    }
}
