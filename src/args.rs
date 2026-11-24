use crate::imports::*;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// Specifies a config file to boot with
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Shows debug messages
    #[arg(short, long)]
    pub debug: bool,

    /// Forces text mode
    #[arg(short, long)]
    pub text: bool,
}
