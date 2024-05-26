use anyhow::Result;
use clap::{Parser, Subcommand};
use ddc_hi::Display;
use swmon::*;

#[derive(clap::Parser)]
#[clap(author, version)]
/// Switch monitor input source from command-line
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List DDC-capable displays and exit
    List,
    /// Switch input source using given display
    Switch {
        /// Display number to use (No. column in list)
        #[clap(short = 'm')]
        monitor: u8,
        /// Input source to switch to
        #[clap(value_enum)]
        input: InputSource,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.cmd {
        Cmd::List => {
            let mut display = Display::enumerate();
            let display_info = collect_display_info(&mut display);
            cli::print_table(&display_info);
        }

        Cmd::Switch { monitor, input } => {
            let mut display = Display::enumerate();
            do_switch(&mut display, monitor, input)?;
        }
    }

    Ok(())
}
