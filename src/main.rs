#![allow(warnings)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod input;
use clap::Parser;
use input::*;
use std::error::Error;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Specifies a config file to boot with
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Starts debugging session
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let presses = init()?;

    for event in presses {
        match event {
            Event::Key { code, state } => {
                let status = if state { "down" } else { "up" };
                println!("key: {code} {status}");
            }
            Event::Mouse { x, y } => {
                println!("mouse changed: {x}, {y}");
            }
            Event::Scroll { x, y } => {
                println!("scrolled: {x}, {y}");
            }
            Event::Unknown => println!("unknown"),
        }
    }

    Ok(())
}
