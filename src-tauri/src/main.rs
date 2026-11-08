// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "mcp-token") {
        betastart_lib::run_mcp_token();
        return;
    }
    if std::env::args().any(|arg| arg == "mcp" || arg == "--mcp") {
        betastart_lib::run_mcp_stdio();
        return;
    }
    betastart_lib::run()
}
