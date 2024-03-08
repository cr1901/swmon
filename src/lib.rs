use std::borrow::Cow;

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use ddc_hi::{Ddc, Display, DisplayInfo};
use tabled::Tabled;

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

pub fn collect_display_info<'a>(display: &'a mut Vec<Display>) -> Vec<TableDisplayInfo<'a>> {
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

    return display_info;
}

pub fn do_switch(display: &mut Vec<Display>, monitor: u8, input: InputSource) -> Result<()> {
    let chosen = display
        .get_mut(monitor as usize)
        .ok_or(anyhow!("monitor number {} out of range", monitor))?;
    chosen.handle.set_vcp_feature(0x60, input as u16)?;

    Ok(())
}


pub mod cli {
    use super::TableDisplayInfo;
    use tabled::{Style, Table};

    pub fn print_table(display_info: &Vec<TableDisplayInfo>) {
        let mut table = Table::new(display_info);
        println!("{}", table.with(Style::blank()));
    }
}
