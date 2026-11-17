use std::{collections::BTreeSet, sync::OnceLock};

pub fn list() -> Vec<String> {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = fontdb::Database::new();
            fonts.load_system_fonts();
            fonts
                .faces()
                .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
                .filter(|name| !name.starts_with('@'))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        })
        .clone()
}
