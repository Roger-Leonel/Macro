//! Biblioteca de macros: os `.kmacro` guardados em `%APPDATA%\Korvon Macro\macros`.
//! É a lista exibida na aba Agendamentos; "Salvar" e "Abrir" começam nesta pasta.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub path: PathBuf,
}

pub fn dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Korvon Macro").join("macros");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Macros da biblioteca em ordem alfabética.
pub fn list() -> Vec<Item> {
    let Some(dir) = dir() else { return vec![] };
    let mut items: Vec<Item> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("kmacro")))
        .filter_map(|p| Some(Item { name: p.file_stem()?.to_str()?.to_string(), path: p }))
        .collect();
    items.sort_by_key(|i| i.name.to_lowercase());
    items
}
