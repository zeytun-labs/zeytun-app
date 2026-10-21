// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "ci-smoke")]
    if std::env::args_os().any(|argument| argument == "--ci-smoke-test") {
        zeytun_lib::ci_smoke::run();
        return;
    }
    zeytun_lib::crash_dump::install();
    zeytun_lib::run()
}
