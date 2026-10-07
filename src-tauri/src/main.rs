// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    zeytun_lib::crash_dump::install();
    zeytun_lib::run()
}
