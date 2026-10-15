//! The preview's demo data: `demo_view` in English, `demo_view fr` in French (from the interface's dictionary).
fn main() {
    let french = std::env::args().nth(1).is_some_and(|lang| lang == "fr");
    let translator =
        remora_app::tray::Translator::new(french.then_some("fr"), include_str!("../../../app/src/i18n/fr.json"));
    let fixture = remora_app::demo::fixture(chrono::Utc::now(), &|text| translator.t(text));
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
