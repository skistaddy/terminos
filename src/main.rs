#![allow(unused_imports)]
#![allow(unused)]

mod app;
mod args;
mod canvas;
mod input;
mod session;
mod test;

use crate::args::Args;
use clap::Parser;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    app::App::run(Args::parse())
}
