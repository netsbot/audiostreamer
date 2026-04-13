use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::{CliArgs, CliOutputMode};
use crate::error::{Result, StreamerError};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub source_path: PathBuf,
    pub output: OutputSelection,
    pub chunk_size: usize,
}

impl AppConfig {
    pub fn from_cli(cli: CliArgs) -> Result<Self> {
        let file_config = if let Some(path) = &cli.config {
            FileConfig::load_from_path(path)?
        } else {
            FileConfig::default()
        };

        let output = cli.output.map(OutputSelection::from).unwrap_or_else(|| {
            OutputSelection::parse(&file_config.output.mode).unwrap_or(OutputSelection::Noop)
        });

        let input = cli
            .input
            .unwrap_or_else(|| file_config.source.input.clone())
            .trim()
            .to_string();

        if input.is_empty() {
            return Err(StreamerError::InvalidConfig(
                "input must be provided via --input or config source.input".to_string(),
            ));
        }

        let chunk_size = cli.chunk_size.unwrap_or(file_config.stream.chunk_size);
        if chunk_size == 0 {
            return Err(StreamerError::InvalidConfig(
                "chunk_size must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            source_path: PathBuf::from(input),
            output,
            chunk_size,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OutputSelection {
    Playback,
    Noop,
}

impl OutputSelection {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "playback" => Some(Self::Playback),
            "noop" => Some(Self::Noop),
            _ => None,
        }
    }
}

impl From<CliOutputMode> for OutputSelection {
    fn from(value: CliOutputMode) -> Self {
        match value {
            CliOutputMode::Playback => Self::Playback,
            CliOutputMode::Noop => Self::Noop,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileConfig {
    #[serde(default)]
    source: FileSourceConfig,
    #[serde(default)]
    output: FileOutputConfig,
    #[serde(default)]
    stream: FileStreamConfig,
}

impl FileConfig {
    fn load_from_path(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str::<Self>(&content)?)
    }
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            source: FileSourceConfig::default(),
            output: FileOutputConfig::default(),
            stream: FileStreamConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileSourceConfig {
    #[serde(default)]
    input: String,
}

impl Default for FileSourceConfig {
    fn default() -> Self {
        Self {
            input: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileOutputConfig {
    #[serde(default = "default_output_mode")]
    mode: String,
}

impl Default for FileOutputConfig {
    fn default() -> Self {
        Self {
            mode: default_output_mode(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileStreamConfig {
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
}

impl Default for FileStreamConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
        }
    }
}

fn default_output_mode() -> String {
    "noop".to_string()
}

fn default_chunk_size() -> usize {
    2048
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn merges_cli_over_file_config() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "[source]\ninput = \"song.wav\"\n[output]\nmode = \"noop\"\n[stream]\nchunk_size = 1024"
        )
        .expect("write config");

        let cli = CliArgs {
            config: Some(file.path().to_path_buf()),
            input: Some("other.wav".to_string()),
            output: Some(CliOutputMode::Playback),
            chunk_size: Some(4096),
        };

        let cfg = AppConfig::from_cli(cli).expect("config from cli");
        assert_eq!(cfg.source_path, PathBuf::from("other.wav"));
        assert!(matches!(cfg.output, OutputSelection::Playback));
        assert_eq!(cfg.chunk_size, 4096);
    }

    #[test]
    fn rejects_empty_input() {
        let cli = CliArgs {
            config: None,
            input: None,
            output: Some(CliOutputMode::Noop),
            chunk_size: Some(1024),
        };

        let err = AppConfig::from_cli(cli).expect_err("must fail without input");
        assert!(matches!(err, StreamerError::InvalidConfig(_)));
    }
}
