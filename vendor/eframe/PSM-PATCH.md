eframe 0.31.1, MIT OR Apache-2.0, Emil Ernerfeldt.
Local change to src/native/run.rs: process due hidden Windows viewports directly, preventing ControlFlow::Poll from spinning after SW_HIDE. Adapted from the same owner-maintained TrontSnap patch; upstream issue references #5229/#7776. All other source is unmodified.
