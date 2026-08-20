use blake3;
use const_hex;
use sha2::{Digest, Sha256};

use std::path::PathBuf;

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
}

#[derive(serde::Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
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

#[cfg(test)]
mod test {
    use super::*;

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
    fn test_gives_good_blak3_hash() {
        let message: &[u8; 5] = &[0x48, 0x65, 0x6C, 0x6C, 0x6F];
        let blake3_hash = HashAlgorithm::Blake3.hash(message);
        assert_eq!(
            "fbc2b0516ee8744d293b980779178a3508850fdcfe965985782c39601b65794f",
            blake3_hash
        );
    }
}
