use clap::{Parser, Subcommand, ValueEnum};
use ddc_hi::{Ddc, Display};
use anyhow::Result;

// use ddc::Ddc;
// use ddc_winapi::Monitor;

#[derive(clap::Parser)]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Cmd
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List displays and exit
    List,
    /// Input source to switch to
    Switch {
        #[clap(short = 'b')]
        backend: Option<String>,
        #[clap(short = 'm')]
        model: Option<String>,
        #[clap(value_enum)]
        input: InputSource
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum InputSource {
    Vga1 = 1,
    Vga2,
    Dvi1,
    Dvi2,
    Composite1,
    Composite2,
    SVideo1,
    SVideo2,
    Tuner1,
    Tuner2,
    Tuner3,
    Component1,
    Component2,
    Component3,
    DisplayPort1,
    DisplayPort2,
    Hdmi1,
    Hdmi2
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.cmd {
        Cmd::List => {
            for mut display in Display::enumerate() {
                if display.update_capabilities().is_ok() {
                    println!(
                        "{:?} {}: {:?} {:?}",
                        display.info.backend,
                        display.info.id,
                        display.info.manufacturer_id,
                        display.info.model_name
                    );
                }
            }
        },

        Cmd::Switch { backend, model, input } => {
            for mut display in Display::enumerate() {
                if model.is_some() || backend.is_some() {
                    unimplemented!("choosing the monitor/backend is unimplemented.")
                }

                display.handle.set_vcp_feature(0x60, input as u16)?;
                break;
            }
        }
    }

    Ok(())
}

