//! The interface's types (`app/src/lib/bindings.ts`) come from the structs the commands send, through ts-rs
//! (`#[derive(TS)]`, which reads their serde attributes): a field renamed, removed or made optional here changes
//! them, and `svelte-check` then finds every use. A test fails when the file is stale; `make desktop-bindings`
//! writes it again.

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::BTreeMap;
    use ts_rs::{Config, TypeVisitor, TS};

    const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts");

    /// Each type the roots use, once, by its TypeScript name.
    struct Declarations {
        config: Config,
        found: BTreeMap<String, (TypeId, String)>,
    }

    impl TypeVisitor for Declarations {
        fn visit<T: TS + 'static + ?Sized>(&mut self) {
            if T::output_path().is_none() {
                return; // A string, a list, a map: written inline.
            }
            let name = T::ident(&self.config);
            if let Some((id, _)) = self.found.get(&name) {
                assert_eq!(*id, TypeId::of::<T>(), "two Rust types are named {name} in TypeScript: rename one");
                return;
            }
            let declaration = format!("{}export {}", T::docs().unwrap_or_default(), T::decl(&self.config));
            self.found.insert(name, (TypeId::of::<T>(), declaration));
            T::visit_dependencies(self);
        }
    }

    fn bindings() -> String {
        // Counts and minutes are small: numbers, not bigints.
        let mut types = Declarations { config: Config::new().with_large_int("number"), found: BTreeMap::new() };
        // What the commands return (commands.rs), and everything in them.
        types.visit::<remora_app::view::InboxView>();
        types.visit::<remora_app::SourceInfo>();
        types.visit::<remora_app::AccountInfo>();
        types.visit::<crate::commands::SettingsView>();
        types.visit::<crate::commands::SnoozeAdvice>();
        types.visit::<crate::updates::UpdateStatus>();
        types.visit::<remora_core::Preset>();
        let declarations: Vec<String> = types.found.into_values().map(|(_, declaration)| declaration).collect();
        format!(
            "// Generated from the Rust structs by src-tauri/src/bindings.rs: run `make desktop-bindings`, don't edit.\n\n{}\n",
            declarations.join("\n\n")
        )
    }

    #[test]
    fn the_interface_types_match_the_rust_structs() {
        let expected = bindings();
        if std::env::var_os("REMORA_WRITE_BINDINGS").is_some() {
            std::fs::write(PATH, &expected).expect("bindings.ts written");
            return;
        }
        let found = std::fs::read_to_string(PATH).unwrap_or_default().replace("\r\n", "\n");
        assert!(found == expected, "app/src/lib/bindings.ts is stale: run `make desktop-bindings`");
    }
}
