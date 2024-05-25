#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use ddc_hi::{Display, DisplayInfo};
use oneshot::{self, TryRecvError};
use std::sync::mpsc;
use std::thread;
use strum::IntoEnumIterator;

use eframe::egui;
use swmon::{collect_display_info, do_switch, InputSource, TableDisplayInfo};

struct AppState {
    control: ControlFlow,
    switch: Option<SwitchState>,
}

enum ControlFlow {
    Waiting(WaitReason),
    Idle,
}

struct SwitchState {
    displays: Vec<TableDisplayInfo<'static>>,
    monitor_select: u8,
    input_select: InputSource,
}

enum WaitReason {
    Detecting {
        just_switched: bool,
        recv: oneshot::Receiver<BgResult<Vec<TableDisplayInfo<'static>>>>,
    },
    Switching(oneshot::Receiver<BgResult<()>>),
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

    let (cmd_send, cmd_recv) = mpsc::channel();
    let (send, recv) = oneshot::channel();
    cmd_send.clone().send(Cmd::DetectMonitors(send));
    let mut state = AppState {
        control: ControlFlow::Waiting(WaitReason::Detecting {
            just_switched: false,
            recv,
        }),
        switch: None,
    };
    thread::spawn(|| bg_thread(cmd_recv));

    let mut bottom_text = String::new();
    eframe::run_simple_native("swmon", options, move |ctx, _frame| {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .min_height(20.0)
            .show(ctx, |ui| {
                ui.centered_and_justified(|ui| ui.label(&bottom_text));
                bottom_text = String::new();
            });

        egui::CentralPanel::default().show(ctx, |ui| match &mut state.control {
            ControlFlow::Waiting(WaitReason::Detecting {
                just_switched,
                recv,
            }) => {
                ui.centered_and_justified(|ui| {
                    ui.add(egui::widgets::Spinner::new().size(100.0));
                });

                if *just_switched {
                    // Quietly go back to detection if we just switched inputs
                    bottom_text = format!("Switching inputs... please wait");
                } else {
                    bottom_text = format!("Detecting attached monitors... please wait");
                }

                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(displays)) if displays.len() > 0 => {
                        if !*just_switched {
                            state.switch = Some(SwitchState {
                                displays,
                                monitor_select: 0,
                                input_select: InputSource::Vga1,
                            });
                        }
                        state.control = ControlFlow::Idle;
                    }
                    Ok(Ok(displays)) if displays.len() == 0 => {
                        let (send, recv_) = oneshot::channel();
                        cmd_send.clone().send(Cmd::DetectMonitors(send));
                        *recv = recv_
                    }
                    Ok(Ok(_)) => unreachable!(),
                    Ok(Err(_)) => todo!(),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => todo!(),
                }
            }
            ControlFlow::Idle => {
                fn choice_text(d: &DisplayInfo) -> String {
                    return format!(
                        "{} {} ({})",
                        d.manufacturer_id.as_ref().cloned().unwrap_or("?".into()),
                        d.model_name.as_ref().cloned().unwrap_or("?".into()),
                        d.backend
                    );
                }

                let SwitchState {
                    displays,
                    ref mut monitor_select,
                    ref mut input_select,
                } = state.switch.as_mut().unwrap();

                // ui.horizontal_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Select display");
                            let combo = egui::ComboBox::from_id_source("display").selected_text(
                                choice_text(&displays[*monitor_select as usize].info),
                            );
                            combo.show_ui(ui, |ui| {
                                for (i, d) in displays.iter().enumerate() {
                                    let text = choice_text(&d.info);
                                    ui.selectable_value(monitor_select, i as u8, text);
                                }
                            });
                        });

                        ui.horizontal(|ui| {
                            let combo = egui::ComboBox::from_id_source("input")
                                .selected_text(input_select.as_ref())
                                .width(0.0);
                            ui.label("Select input");
                            combo.show_ui(ui, |ui| {
                                for inp in InputSource::iter() {
                                    ui.selectable_value(input_select, inp, inp.as_ref());
                                }
                            });
                        });
                    });

                    if ui.button("Switch!").clicked() {
                        let (send, recv) = oneshot::channel();
                        cmd_send.clone().send(Cmd::SwitchMonitor((
                            *monitor_select,
                            *input_select,
                            send,
                        )));
                        state.control = ControlFlow::Waiting(WaitReason::Switching(recv));
                    }
                });
                // });
            }
            ControlFlow::Waiting(WaitReason::Switching(recv)) => {
                ui.centered_and_justified(|ui| {
                    ui.add(egui::widgets::Spinner::new().size(100.0));
                });

                bottom_text = format!("Switching inputs... please wait");
                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(_)) => {
                        let (send, recv) = oneshot::channel();
                        cmd_send.clone().send(Cmd::DetectMonitors(send));
                        state.control = ControlFlow::Waiting(WaitReason::Detecting {
                            just_switched: true,
                            recv,
                        });
                    }
                    Ok(Err(_)) => todo!(),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => todo!(),
                }
            }
        });
    })
}
