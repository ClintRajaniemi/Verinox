use blake3;
use const_hex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
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
    Blake3,
}

impl HashAlgorithm {
    pub fn hash(&self, bytes: &[u8]) -> String {
        match self {
            // match arms for hashing both with sha256 and blake3
            HashAlgorithm::Sha256 => const_hex::encode(Sha256::digest(bytes)),
            HashAlgorithm::Blake3 => blake3::hash(bytes).to_string(),
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

#[derive(Deserialize, Debug, PartialEq)]
pub struct Config {
    watch_patterns: Vec<String>,
    hash_algorithm: HashAlgorithm,
    #[serde(default = "default_log_dir")]
    log_dir: PathBuf,
    max_log_size: bytesize::ByteSize,
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
        let toml_config = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;

        let config: Config = toml::from_str(&toml_config)?;

        for glob in &config.watch_patterns {
            let _result =
                glob::Pattern::new(&glob).map_err(|source| ConfigError::GlobPattern { source })?;
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
mod test {
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
    fn test_gives_good_blake3_hash() {
        let message: &[u8; 5] = &[0x48, 0x65, 0x6C, 0x6C, 0x6F];
        let blake3_hash = HashAlgorithm::Blake3.hash(message);
        assert_eq!(
            "fbc2b0516ee8744d293b980779178a3508850fdcfe965985782c39601b65794f",
            blake3_hash
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
    #[test]
    fn test_load_config_exists() {
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
                log_dir: PathBuf::from("C:\\ProgramData\\Verinox\\logs".to_string()),
                max_log_size: ByteSize::from_str("10MB").unwrap()
            },
            result.unwrap()
        );
    }
}
