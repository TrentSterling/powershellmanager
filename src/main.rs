#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod action_tests;
mod activity;
mod app;
mod arrange;
mod auto;
mod branding;
mod cli;
mod config;
mod desktop;
mod gui;
mod history;
mod hotkey;
mod launch;
mod layout;
mod monitor;
#[cfg(test)]
mod native_audit;
mod order;
mod persistence;
mod sticky;
mod theme;
mod theme_studio;
mod tray;
#[cfg(test)]
mod ui_tests;
mod updates;
mod window_chrome;
mod windows;

use clap::Parser;

fn main() -> std::process::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let cli = cli::Cli::parse();

    let output = launch::dispatch(
        cli,
        config::load(),
        &mut desktop::NativeDesktop,
        &mut launch::NativeGuiRunner,
    );
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    std::process::ExitCode::from(output.exit_code as u8)
}
