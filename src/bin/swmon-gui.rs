#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use core::fmt;
use ddc_hi::{Display, DisplayInfo};
use env_logger;
use log::{debug, trace};
use oneshot::{self, TryRecvError};
use std::thread;
use std::{collections::VecDeque, sync::mpsc};
use strum::IntoEnumIterator;

use eframe::egui;
use swmon::{collect_display_info, do_switch, InputSource, TableDisplayInfo};

struct AppState {
    control: ControlFlow,
    switch: Option<SwitchState>,
    bottom_text: String,
    error_text: VecDeque<String>,
}

enum ControlFlow {
    Waiting(WaitReason),
    Idle,
}

struct SwitchState {
    displays: Vec<TableDisplayInfo<'static>>,
    /// Map monitor select to displays vec.
    displays_map: Vec<u8>,
    monitor_select: u8,
    input_select: InputSource,
}

enum WaitReason {
    Detecting {
        just_switched: bool,
        long_detect: bool,
        recv: oneshot::Receiver<BgResult<Vec<TableDisplayInfo<'static>>>>,
    },
    Switching(oneshot::Receiver<BgResult<()>>),
}

struct BackgroundError {
    msg: String,
}

type BgResult<T> = Result<T, BackgroundError>;

enum Cmd {
    DetectMonitors(oneshot::Sender<BgResult<Vec<TableDisplayInfo<'static>>>>),
    SwitchMonitor((u8, InputSource, oneshot::Sender<BgResult<()>>)),
}

impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cmd::DetectMonitors(_) => f.write_str("DetectMonitors"),
            Cmd::SwitchMonitor((i, src, _)) => write!(f, "SwitchMonitor ({}, {})", i, src),
        }
    }
}

fn bg_thread(recv: mpsc::Receiver<Cmd>) {
    let mut displays = None;
    // let mut display_info = None;

    let mut i = 0;
    loop {
        trace!(target: "swmon-gui-bg", "Waiting for event {}", i);
        let res = if let Ok(cmd) = recv.recv() {
            cmd
        } else {
            debug!(target: "swmon-gui-bg", "Sender disconnected");
            break;
        };

        match res {
            Cmd::DetectMonitors(send) => {
                debug!(target: "swmon-gui-bg", "Detecting monitors");
                displays.replace(Display::enumerate());

                match displays.as_mut() {
                    Some(d) => {
                        let display_info: Vec<TableDisplayInfo<'static>> =
                        collect_display_info(d)
                            .into_iter()
                            .map(|d| d.to_static())
                            .collect();
                        let _ = send.send(Ok(display_info));
                    },
                    None => {
                        debug!(target: "swmon-gui-bg", "No monitors detected");
                        return;
                    }
                }
            }
            Cmd::SwitchMonitor((num, input_source, send)) => {
                match displays.as_mut() {
                    Some(d) => {
                        debug!(target: "swmon-gui-bg", "Switching monitor {}", num);
                        match do_switch(d, num, input_source) {
                            Ok(_) => {
                                let _ = send.send(Ok(()));
                            }
                            Err(e) => {
                                debug!(target: "swmon-gui-bg", "Could not perform switch: {}", e);
                                let _ = send.send(Err(BackgroundError { msg: e.to_string() }));
                            }
                        }
                    },
                    None => {
                        debug!(target: "swmon-gui-bg", "Detected displays are gone");
                        return;
                    }
                }

            }
        }

        i += 1;
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let (cmd_send, cmd_recv) = mpsc::channel();
    let (send, recv) = oneshot::channel();
    let detect_cmd = Cmd::DetectMonitors(send);

    let mut error_text = VecDeque::new();

    let error_tmp = format!("Error sending request {}", detect_cmd);
    if cmd_send.clone().send(detect_cmd).is_err() {
        error_text.push_back(error_tmp);
    }

    let mut state = AppState {
        control: ControlFlow::Waiting(WaitReason::Detecting {
            just_switched: false,
            long_detect: false,
            recv,
        }),
        switch: None,
        bottom_text: String::new(),
        error_text,
    };
    thread::spawn(|| bg_thread(cmd_recv));

    eframe::run_simple_native("swmon", options, move |ctx, _frame| {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .min_height(20.0)
            .show(ctx, |ui| {
                ui.centered_and_justified(|ui| ui.label(&state.bottom_text));
                state.bottom_text = String::new();
            });

        egui::CentralPanel::default().show(ctx, |ui| match &mut state.control {
            ControlFlow::Waiting(WaitReason::Detecting {
                just_switched,
                long_detect,
                recv,
            }) => {
                ui.centered_and_justified(|ui| {
                    ui.add(egui::widgets::Spinner::new().size(100.0));
                });

                // Status reports that indicate no problems or probably
                // transient problems.
                state.bottom_text = match (*just_switched, *long_detect) {
                    (false, false) => format!("Detecting attached monitors... please wait"),
                    (false, true) => format!("Detect found nothing; no monitors?"),
                    // Quietly go back to detection if we just switched inputs
                    (true, _) => format!("Switching inputs... please wait"),
                    // This is too common in normal operation to justify right now.
                    // (true, true) => format!("Refresh found nothing; monitors not responding?"),
                };

                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(displays)) if displays.len() > 0 => {
                        if !*just_switched {
                            // If max_id is 255 we need a 256 element reverse-map.
                            let max_id: u16 = displays.iter().map(|d| d.number).max().unwrap() as u16;
                            let mut displays_map = vec![0; (max_id + 1) as usize];
                            for (i, d) in displays.iter().map(|d| d.number).enumerate() {
                                displays_map[d as usize] = i as u8;
                            }

                            state.switch = Some(SwitchState {
                                displays,
                                displays_map,
                                monitor_select: 0,
                                input_select: InputSource::Vga1,
                            });
                        }
                        state.control = ControlFlow::Idle;
                    }
                    Ok(Ok(displays)) if displays.len() == 0 => {
                        let (send, recv_) = oneshot::channel();
                        let detect_cmd = Cmd::DetectMonitors(send);

                        let error_text = format!("Error sending request {}", detect_cmd);
                        if cmd_send.clone().send(detect_cmd).is_err() {
                            state.error_text.push_back(error_text)
                        }

                        *long_detect = true;
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
                    displays_map,
                    ref mut monitor_select,
                    ref mut input_select,
                } = state.switch.as_mut().unwrap();

                // ui.horizontal_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Select display");
                            let combo = egui::ComboBox::from_id_source("display").selected_text(
                                choice_text(&displays[displays_map[*monitor_select as usize]as usize].info),
                            );
                            combo.show_ui(ui, |ui| {
                                for d in displays.iter() {
                                    let text = choice_text(&d.info);
                                    ui.selectable_value(monitor_select, d.number, text);
                                }
                            });

                            state.bottom_text = format!("{}", &displays[displays_map[*monitor_select as usize] as usize].info);
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
                        let switch_cmd = Cmd::SwitchMonitor((*monitor_select, *input_select, send));

                        let error_text = format!("Error sending request {}", switch_cmd);
                        if cmd_send.clone().send(switch_cmd).is_err() {
                            state.error_text.push_back(error_text)
                        }

                        state.control = ControlFlow::Waiting(WaitReason::Switching(recv));
                    }
                });
                // });
            }
            ControlFlow::Waiting(WaitReason::Switching(recv)) => {
                ui.centered_and_justified(|ui| {
                    ui.add(egui::widgets::Spinner::new().size(100.0));
                });

                state.bottom_text = format!("Switching inputs... please wait");
                let recv_res = recv.try_recv();

                match recv_res {
                    Ok(Ok(_)) => {
                        let (send, recv) = oneshot::channel();
                        let detect_cmd = Cmd::DetectMonitors(send);

                        let error_text = format!("Error sending request {}", detect_cmd);
                        if cmd_send.clone().send(detect_cmd).is_err() {
                            state.error_text.push_back(error_text);
                        }

                        state.control = ControlFlow::Waiting(WaitReason::Detecting {
                            just_switched: true,
                            long_detect: false,
                            recv,
                        });
                    }
                    Ok(Err(BackgroundError { msg })) => {
                        let (send, recv) = oneshot::channel();
                        let detect_cmd = Cmd::DetectMonitors(send);

                        state
                            .error_text
                            .push_back(format!("Error switching monitor {}", msg));

                        let error_text = format!("Error sending request {}", detect_cmd);
                        if cmd_send.clone().send(detect_cmd).is_err() {
                            state.error_text.push_back(error_text);
                        }

                        state.control = ControlFlow::Waiting(WaitReason::Detecting {
                            just_switched: true,
                            long_detect: false,
                            recv,
                        });
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        state
                            .error_text
                            .push_back("Monitor switching thread stopped responding! Close and restart to fix.".to_string());
                    }
                }
            }
        });

        let mut show_error = !state.error_text.is_empty();
        if show_error {
            egui::Window::new("Error")
                .open(&mut show_error)
                .show(ctx, |ui| ui.label(state.error_text.get(0).unwrap()));
        }

        if !show_error {
            let _ = state.error_text.pop_front();
        }
    })
}
