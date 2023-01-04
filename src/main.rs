use std::borrow::Cow;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ddc_hi::{Ddc, Display, DisplayInfo};
use tabled::{Style, Table, Tabled};

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

/// MCCS input sources- names follow the spec for Feature Code 0x60.
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
    Hdmi2,
}

pub struct TableDisplayInfo<'a> {
    number: u8,
    info: &'a DisplayInfo,
}

impl<'a> Tabled for TableDisplayInfo<'a> {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'static, str>> {
        vec![
            format!("{}", self.number),
            format!("{}", self.info.backend),
            self.info.id.clone(),
            format!(
                "{}",
                &self
                    .info
                    .manufacturer_id
                    .as_ref()
                    .cloned()
                    .unwrap_or("?".into())
            ),
            format!(
                "{}",
                &self.info.model_name.as_ref().cloned().unwrap_or("?".into())
            ),
        ]
        .into_iter()
        .map(|s| Cow::<'static, _>::Owned(s))
        .collect()
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec![
            "No.",
            "Backend",
            "Display ID",
            "Manufacturer ID",
            "Model Name",
        ]
        .into_iter()
        .map(|s| Cow::Borrowed(s))
        .collect()
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.cmd {
        Cmd::List => {
            let mut display = Display::enumerate();
            let display_info = display
                .iter_mut()
                .enumerate()
                .filter_map(|(i, display)| {
                    if display.update_capabilities().is_ok() {
                        Some(TableDisplayInfo {
                            number: i as u8,
                            info: &display.info,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<TableDisplayInfo>>();

            let mut table = Table::new(&display_info);
            println!("{}", table.with(Style::blank()));
        }

        Cmd::Switch { monitor, input } => {
            let mut display = Display::enumerate();
            let chosen = display
                .get_mut(monitor as usize)
                .ok_or(anyhow!("monitor number {} out of range", monitor))?;
            chosen.handle.set_vcp_feature(0x60, input as u16)?;
        }
    }

    Ok(())
}
