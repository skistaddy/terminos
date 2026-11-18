use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// Specifies a config file to boot with
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Starts debugging session
    #[arg(short, long)]
    pub debug: bool,

    /// Starts input test
    #[arg(short, long)]
    pub input_test: bool,
}
