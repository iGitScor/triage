fn main() {
    let fixture = remora_app::demo::fixture(chrono::Utc::now());
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
