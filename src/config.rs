use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use std::fs;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config file at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config {0}")]
    Parse(#[from] toml::de::Error),

    #[error("invalid watch path: {0}")]
    InvalidWatchPath(PathBuf),

    #[error("failed to write config file at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse glob in watch_path: {source}")]
    GlobPattern { source: glob::PatternError },
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    Sha256,
}

impl HashAlgorithm {
    pub fn hash(&self, bytes: &[u8]) -> String {
        match self {
            HashAlgorithm::Sha256 => const_hex::encode(Sha256::digest(bytes)),
        }
    }
}

#[cfg(target_os = "windows")]
fn default_log_dir() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\Verinox\logs")
}
#[cfg(target_os = "linux")]
fn default_log_dir() -> PathBuf {
    PathBuf::from("/var/log/verinox")
}
#[cfg(target_os = "macos")]
fn default_log_dir() -> PathBuf {
    PathBuf::from("/Library/Logs/Verinox")
}

#[cfg(target_os = "windows")]
fn default_baseline_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\Verinox\baseline.json")
}
#[cfg(target_os = "linux")]
fn default_baseline_path() -> PathBuf {
    PathBuf::from("/var/lib/verinox/baseline.json")
}
#[cfg(target_os = "macos")]
fn default_baseline_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/Verinox/baseline.json")
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Config {
    watch_patterns: Vec<String>,
    hash_algorithm: HashAlgorithm,
    #[serde(default = "default_log_dir")]
    log_dir: PathBuf,
    max_log_size: bytesize::ByteSize,
    #[serde(default = "default_baseline_path")]
    baseline_path: PathBuf,
}

impl Config {
    pub fn ensure_exists(path: &Path) -> Result<(), ConfigError> {
        if path.exists() {
            return Ok(());
        }
        // In case the config file doesn't exist, we'll write a base, default config
        fs::write(path, DEFAULT_CONFIG).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load(path: &PathBuf) -> Result<Config, ConfigError> {
        let toml_config = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;

        let config: Config = toml::from_str(&toml_config)?;

        for glob in &config.watch_patterns {
            let _result =
                glob::Pattern::new(glob).map_err(|source| ConfigError::GlobPattern { source })?;
        }

        Ok(config)
    }

    pub fn watch_patterns(&self) -> &[String] {
        &self.watch_patterns
    }

    pub fn hash_algorithm(&self) -> HashAlgorithm {
        self.hash_algorithm
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub fn max_log_size(&self) -> bytesize::ByteSize {
        self.max_log_size
    }

    pub fn baseline_path(&self) -> &PathBuf {
        &self.baseline_path
    }

    // Used by unit tests in writer.rs
    pub fn build_test_config(
        dir: &TempDir,
        watch_pattern_path: &[&str],
    ) -> Result<Config, ConfigError> {
        let log_dir = dir.path().join("logs");
        let log_dir_string = log_dir.to_string_lossy().into_owned();

        let watch_dir = watch_pattern_path.iter().next();
        let watch_dir = match watch_dir {
            Some(watch_dir) => watch_dir,
            None => "",
        };

        let config_path = dir.path().join("test_config.toml");

        let baseline_path = dir.path().join("baseline.json");
        let baseline_path_string = baseline_path.to_string_lossy().into_owned();

        let toml_str = format!(
            r#"hash_algorithm = 'sha256'
log_dir = '{log_dir_string}'
max_log_size = '50B'
watch_patterns = ['{watch_dir}']
baseline_path = '{baseline_path_string}'
"#
        );

        std::fs::write(&config_path, toml_str).unwrap();

        Config::load(&config_path)
    }
}

#[cfg(target_os = "windows")]
const DEFAULT_CONFIG: &str = r#"# Hashing algorithm to use when hashing files.
hash_algorithm = "sha256"
# Log directory
log_dir = 'C:\ProgramData\Verinox\logs'
# Maximum size of a single log file before it breaks off into a new file.
max_log_size = "10MB"
# watch_patterns are the directories to monitor for file changes.
watch_patterns = [
    'C:\Windows\System32\drivers\etc\hosts', 
    'C:\Windows\System32\Tasks\**', 
    'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp\**', 
    'C:\Users\*\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup\**', 
    'C:\Windows\win.ini', 
    'C:\Windows\system.ini'
    ]"#;

#[cfg(target_os = "linux")]
const DEFAULT_CONFIG: &str = r#"# Hashing algorithm to use when hashing files.
hash_algorithm = "sha256"
# Log Directory
log_dir = "/var/log/verinox"
# Maximum size of a single log file before it breaks off into a new file.
max_log_size = "10MB"
# watch_patterns are the directories to monitor for file changes.
watch_patterns = [
    '/etc/passwd', 
    '/etc/shadow', 
    '/etc/sudoers', 
    '/etc/sudoers.d/**', 
    '/etc/hosts', 
    '/etc/ssh/sshd_config', 
    '/etc/crontab', 
    '/etc/cron.d/**', 
    '/etc/systemd/system/**', 
    '/etc/ld.so.preload'
    ]"#;

#[cfg(target_os = "macos")]
const DEFAULT_CONFIG: &str = r#"# Hashing algorithm to use when hashing files.
hash_algorithm = "sha256"
# Log directory
log_dir = "/Library/Logs/Verinox"
# Maximum size of a single log file before it breaks off into a new file.
max_log_size = "10MB"
# watch_patterns are the directories to monitor for file changes.
watch_patterns = [
    '/etc/hosts', 
    '/etc/passwd', 
    '/etc/sudoers', 
    '/etc/ssh/sshd_config', 
    '/Library/LaunchAgents/**', 
    '/Library/LaunchDaemons/**'
    ]"#;

#[cfg(test)]
mod tests {
    use super::*;
    use bytesize::ByteSize;

    use std::str::FromStr;

    #[test]
    fn test_gives_good_sha256_hash() {
        let message: &[u8; 5] = &[0x48, 0x65, 0x6C, 0x6C, 0x6F];
        let sha256_hash = HashAlgorithm::Sha256.hash(message);
        assert_eq!(
            "185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969",
            sha256_hash
        );
    }
    #[test]
    fn test_config_not_exists_write_new_config() {
        let temp_dir = tempfile::Builder::new()
            .prefix("test_assets")
            .tempdir()
            .unwrap();
        let path = temp_dir
            .path()
            .join(Path::new("default_config.windows.toml"));
        let _result = Config::ensure_exists(&path);
        assert!(path.exists());
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn test_load_config_exists_windows() {
        let path = PathBuf::from("assets/default_config.windows.toml");
        let result = Config::load(&path);
        let watch_patterns: Vec<String> = vec![
            "C:\\Windows\\System32\\drivers\\etc\\hosts".to_string(),
            "C:\\Windows\\System32\\Tasks\\**".to_string(),
            "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\StartUp\\**".to_string(),
            "C:\\Users\\*\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\**"
                .to_string(),
            "C:\\Windows\\win.ini".to_string(),
            "C:\\Windows\\system.ini".to_string(),
        ];
        assert_eq!(
            Config {
                hash_algorithm: HashAlgorithm::Sha256,
                watch_patterns,
                log_dir: PathBuf::from(r"C:\ProgramData\Verinox\logs"),
                max_log_size: ByteSize::from_str("10MB").unwrap(),
                baseline_path: PathBuf::from(r"C:\ProgramData\Verinox\baseline.json")
            },
            result.unwrap()
        );
    }
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_load_config_exists_not_windows() {
        let path = PathBuf::from("assets/default_config.linux.toml");
        let result = Config::load(&path);
        let watch_patterns: Vec<String> = vec![
            "/etc/passwd".to_string(),
            "/etc/shadow".to_string(),
            "/etc/sudoers".to_string(),
            "/etc/sudoers.d/**".to_string(),
            "/etc/hosts".to_string(),
            "/etc/ssh/sshd_config".to_string(),
            "/etc/crontab".to_string(),
            "/etc/cron.d/**".to_string(),
            "/etc/systemd/system/**".to_string(),
            "/etc/ld.so.preload".to_string(),
        ];
        assert_eq!(
            Config {
                hash_algorithm: HashAlgorithm::Sha256,
                watch_patterns,
                log_dir: PathBuf::from("/var/log/verinox"),
                max_log_size: ByteSize::from_str("10MB").unwrap(),
                baseline_path: PathBuf::from("/var/lib/verinox/baseline.json")
            },
            result.unwrap()
        );
    }
}
