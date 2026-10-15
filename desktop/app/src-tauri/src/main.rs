fn main() {
    // `remora count`, `remora list`, `remora remind`: the same binary asks the running app, then exits (IDEA-22).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if remora_app::local_api::is_command(&args) {
        std::process::exit(remora_app::local_api::run_command(&args));
    }
    remora_tray::run()
}
