// No console window behind the app on Windows (debug builds keep it, for
// the log).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    djsugar::run()
}
