#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use ddc_hi::{Display, DisplayInfo};
use oneshot::{self, TryRecvError};
use std::sync::mpsc;
use std::thread;
use strum::IntoEnumIterator;

use eframe::{egui, App};
use swmon::{collect_display_info, do_switch, InputSource, TableDisplayInfo};

enum AppState {
    SendDetect,
    Detect(oneshot::Receiver<BgResult<Vec<TableDisplayInfo<'static>>>>),
    Idle {
        displays: Vec<TableDisplayInfo<'static>>,
        monitor_select: u8,
        input_select: InputSource,
    },
    Switch(oneshot::Receiver<BgResult<()>>),
}

struct BackgroundError {}

type BgResult<T> = Result<T, BackgroundError>;

enum Cmd {
    DetectMonitors(oneshot::Sender<BgResult<Vec<TableDisplayInfo<'static>>>>),
    SwitchMonitor((u8, InputSource, oneshot::Sender<BgResult<()>>)),
}

fn bg_thread(recv: mpsc::Receiver<Cmd>) {
    let mut displays = None;
    // let mut display_info = None;

    loop {
        let res = if let Ok(cmd) = recv.recv() {
            cmd
        } else {
            break;
        };

        match res {
            Cmd::DetectMonitors(send) => {
                displays.replace(Display::enumerate());

                let display_info: Vec<TableDisplayInfo<'static>> =
                    collect_display_info(displays.as_mut().unwrap())
                        .into_iter()
                        .map(|d| d.to_static())
                        .collect();
                send.send(Ok(display_info));
            }
            Cmd::SwitchMonitor((num, input_source, send)) => {
                do_switch(displays.as_mut().unwrap(), num, input_source);
                send.send(Ok(()));
            }
            _ => unimplemented!(),
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    // env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let mut state = AppState::SendDetect;

    // Our application state:
    let (cmd_send, cmd_recv) = mpsc::channel();
    thread::spawn(|| bg_thread(cmd_recv));

    eframe::run_simple_native("swmon", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| match &mut state {
            AppState::SendDetect => {
                let (send, recv) = oneshot::channel();
                cmd_send.clone().send(Cmd::DetectMonitors(send));
                state = AppState::Detect(recv);
            }
            AppState::Detect(recv) => {
                ui.label(format!("Detecting attached monitors... please wait"));
                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(displays)) if displays.len() > 0 => {
                        state = AppState::Idle {
                            displays,
                            monitor_select: 0,
                            input_select: InputSource::Vga1,
                        };
                    }
                    Ok(Ok(displays)) if displays.len() == 0 => state = AppState::SendDetect,
                    Ok(Ok(_)) => unreachable!(),
                    Ok(Err(_)) => todo!(),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => todo!(),
                }
            }
            AppState::Idle {
                displays,
                monitor_select,
                input_select,
            } => {
                fn choice_text(d: &DisplayInfo) -> String {
                    return format!(
                        "{} {} ({})",
                        d.manufacturer_id.as_ref().cloned().unwrap_or("?".into()),
                        d.model_name.as_ref().cloned().unwrap_or("?".into()),
                        d.backend
                    );
                }

                let combo = egui::ComboBox::from_label("Select display")
                    .selected_text(choice_text(&displays[*monitor_select as usize].info));
                combo.show_ui(ui, |ui| {
                    for (i, d) in displays.iter().enumerate() {
                        let text = choice_text(&d.info);
                        ui.selectable_value(monitor_select, i as u8, text);
                    }
                });

                let combo =
                    egui::ComboBox::from_label("Select input").selected_text(input_select.as_ref());
                combo.show_ui(ui, |ui| {
                    for inp in InputSource::iter() {
                        ui.selectable_value(input_select, inp, inp.as_ref());
                    }
                });

                if ui.button("Switch!").clicked() {
                    let (send, recv) = oneshot::channel();
                    cmd_send.clone().send(Cmd::SwitchMonitor((
                        *monitor_select,
                        *input_select,
                        send,
                    )));
                    state = AppState::Switch(recv);
                }
            }
            AppState::Switch(recv) => {
                ui.label(format!("Switching inputs... please wait"));
                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(_)) => {
                        state = AppState::SendDetect;
                    }
                    Ok(Err(_)) => todo!(),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => todo!(),
                }
            }
        });
    })
}
