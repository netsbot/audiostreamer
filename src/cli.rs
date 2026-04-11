use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Parser)]
#[command(name = "audiostreamer", about = "Scaffold audio streaming app")]
pub struct CliArgs {
    #[arg(long, help = "Path to a TOML config file")]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Input audio file path")]
    pub input: Option<String>,

    #[arg(long, value_enum, help = "Output mode (playback/noop)")]
    pub output: Option<CliOutputMode>,

    #[arg(long, help = "Chunk size used in stream loop")]
    pub chunk_size: Option<usize>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliOutputMode {
    Playback,
    Noop,
}