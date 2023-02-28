use std::{
    sync::{Arc, Mutex},
    thread,
};

use crate::scheduler::{
    FirstComeFirstServeScheduler, IODeviceState, ProcessState, Scheduler,
    SchedulerTickOutcome, SchedulerWrapper,
};

use egui_extras::{Column, TableBuilder};
use rand::Rng;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct SchedulerApp {
    // Mutable reference to the scheduler wrapper.
    #[serde(skip)]
    scheduler_wrapper: Arc<Mutex<SchedulerWrapper<FirstComeFirstServeScheduler>>>,

    #[serde(skip)]
    last_tick_outcome: Option<SchedulerTickOutcome>,

    // Speed control
    #[serde(skip)]
    speed: Arc<Mutex<f32>>,
}

impl Default for SchedulerApp {
    fn default() -> Self {
        let scheduler_wrapper = Arc::new(Mutex::new(SchedulerWrapper::new(
            FirstComeFirstServeScheduler::new(),
        )));

        // Add some processes to the scheduler.
        let mut scheduler_wrapper_locked = scheduler_wrapper.lock().unwrap();
        // Create an rng for setting random params for the jobs
        let mut rng = rand::thread_rng();
        for _ in 0..15 {
            let job_duration = rng.gen_range(1..=360);
            let io_prob = rng.gen_range(0.0..=0.1);
            scheduler_wrapper_locked
                .scheduler
                .register_process(job_duration, io_prob);
        }
        drop(scheduler_wrapper_locked);

        let scheduler_wrapper_clone = scheduler_wrapper.clone();

        let speed = Arc::new(Mutex::new(0.0));

        let speed_clone = speed.clone();

        let _ = thread::spawn(move || loop {
            // Compute sleep duration based on speed
            // 0 speed, skip iteration
            // 0.5 speed, sleep 1000 / 30
            // 1 speed, sleep 1000 / 60

            let curr_speed = *speed_clone.lock().unwrap();

            let sleep_duration = if curr_speed == 0.0 {
                thread::sleep(std::time::Duration::from_millis(100));
                continue;
            } else {
                (1000.0 / (60.0 * curr_speed)) as u64
            };

            thread::sleep(std::time::Duration::from_millis(sleep_duration));
            let mut scheduler_wrapper_clone = scheduler_wrapper_clone.lock().unwrap();
            scheduler_wrapper_clone.tick();
            drop(scheduler_wrapper_clone);
        });

        Self {
            scheduler_wrapper,
            last_tick_outcome: None,
            speed: speed,
        }
    }
}

impl SchedulerApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for SchedulerApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self {
            scheduler_wrapper, ..
        } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        /*
        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });*/

        /*
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(label);
            });

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });
        */

        egui::CentralPanel::default().show(ctx, |_| {});

        // Window that shows the state of processes.
        egui::Window::new("Process states").show(ctx, |ui| {
            let scheduler_wrapper = scheduler_wrapper.lock().unwrap();

            let scheduler = &scheduler_wrapper.scheduler;
            let processes = &scheduler.processes;

            let table = TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder())
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("PID");
                    });
                    header.col(|ui| {
                        ui.strong("CPU Time");
                    });
                    header.col(|ui| {
                        ui.strong("State");
                    });
                });

            table.body(|mut body| {
                for (i, process) in processes.iter().enumerate() {
                    // The color will be based on the state of the process.
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("{:?}", i));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:?}", process.time));
                        });
                        row.col(|ui| {
                            let color = match process.state {
                                ProcessState::Ready => egui::Color32::YELLOW,
                                ProcessState::Running => egui::Color32::GREEN,
                                ProcessState::IOBlocked => egui::Color32::RED,
                                ProcessState::IORequest => egui::Color32::LIGHT_BLUE,
                                ProcessState::Finished => egui::Color32::BROWN,
                            };
                            ui.colored_label(color, format!("{:?}", process.state));
                        });
                    });
                }
            });
            drop(scheduler_wrapper);
            ctx.request_repaint_after(std::time::Duration::from_millis(1000 / 60));
        });

        // Window that shows the state of the IO device.
        egui::Window::new("IO device").show(ctx, |ui| {
            let scheduler_wrapper = scheduler_wrapper.lock().unwrap();
            let scheduler = &scheduler_wrapper.scheduler;
            let io_device = &scheduler.io_device;
            // The color will be based on the state of the IO device.
            let color = match io_device.state {
                IODeviceState::Idle => egui::Color32::YELLOW,
                IODeviceState::Busy => egui::Color32::GREEN,
            };
            ui.colored_label(color, format!("{:?}", io_device.state));

            let progres_bar =
                egui::ProgressBar::new(io_device.remaining_progress as f32 / 10.0);

            ui.add(progres_bar);

            drop(scheduler_wrapper);
            ctx.request_repaint_after(std::time::Duration::from_millis(1000 / 60));
        });

        // Window that shows the scheduler state.
        egui::Window::new("Scheduler").show(ctx, |ui| {
            let mut scheduler_wrapper = scheduler_wrapper.lock().unwrap();
            // Get the last event from the event queue.
            let event = scheduler_wrapper.event_queue.pop();

            if let Some(event) = event {
                self.last_tick_outcome = Some(event);
            }

            if let Some(event) = self.last_tick_outcome {
                let selected_row: usize = match event {
                    SchedulerTickOutcome::Idle => 0,
                    SchedulerTickOutcome::ProcessResumed(_) => 1,
                    SchedulerTickOutcome::ProcessContinued(_) => 2,
                    SchedulerTickOutcome::ProcessFinished(_) => 3,
                    SchedulerTickOutcome::IORequestHandled(_) => 4,
                    SchedulerTickOutcome::IOFinished(_) => 5,
                };

                let names = [
                    "Idle",
                    "Process resumed",
                    "Process continued",
                    "Process finished",
                    "IO request handled",
                    "IO finished",
                ];

                TableBuilder::new(ui)
                    .striped(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Action");
                        });
                        header.col(|ui| {
                            ui.strong("Active?");
                        });
                    })
                    .body(|mut body| {
                        for i in 0..6 {
                            let color = if i == selected_row {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::WHITE
                            };
                            body.row(30.0, |mut row| {
                                row.col(|ui| {
                                    ui.colored_label(color, names[i]);
                                });
                                row.col(|ui| {
                                    ui.colored_label(
                                        color,
                                        format!("{:?}", i == selected_row),
                                    );
                                });
                            });
                        }
                    });

                let used_process: Option<usize> = match event {
                    SchedulerTickOutcome::IOFinished(process) => Some(process),
                    SchedulerTickOutcome::ProcessFinished(process) => Some(process),
                    SchedulerTickOutcome::IORequestHandled(process) => Some(process),
                    SchedulerTickOutcome::ProcessResumed(process) => Some(process),
                    SchedulerTickOutcome::ProcessContinued(process) => Some(process),
                    SchedulerTickOutcome::Idle => None,
                };
                // If there is an event, show it.
                if let Some(process) = used_process {
                    ui.label(format!("Process: {:?}", process));
                } else {
                    ui.label("Idle");
                }
            }
            // Clear event queue for now
            scheduler_wrapper.event_queue.clear();
            ctx.request_repaint_after(std::time::Duration::from_millis(1000 / 60));
        });

        // Window for the simulation speed.
        egui::Window::new("Simulation speed").show(ctx, |ui| {
            let mut simulation_speed = *self.speed.lock().unwrap();
            ui.add(egui::Slider::new(&mut simulation_speed, 0.0..=1.0).text("Speed"));
            *self.speed.lock().unwrap() = simulation_speed;
        });
    }
}
