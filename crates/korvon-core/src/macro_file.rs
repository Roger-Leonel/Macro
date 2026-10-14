use crate::action::Action;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum MacroError {
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("arquivo inválido: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("versão {0} do formato não é suportada (máximo {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
}

/// Macro persistida em `.kmacro` (JSON versionado).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub version: u32,
    pub name: String,
    pub actions: Vec<Action>,
}

impl Macro {
    pub fn new(name: impl Into<String>, actions: Vec<Action>) -> Self {
        Self { version: FORMAT_VERSION, name: name.into(), actions }
    }

    pub fn to_json(&self) -> Result<String, MacroError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self, MacroError> {
        let m: Macro = serde_json::from_str(s)?;
        if m.version > FORMAT_VERSION {
            return Err(MacroError::UnsupportedVersion(m.version));
        }
        Ok(m)
    }

    pub fn save(&self, path: &Path) -> Result<(), MacroError> {
        Ok(std::fs::write(path, self.to_json()?)?)
    }

    pub fn load(path: &Path) -> Result<Self, MacroError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{MouseButton, PressState};

    #[test]
    fn roundtrip() {
        let m = Macro::new(
            "t",
            vec![
                Action::MouseMove { x: 1, y: -2, ms: 8 },
                Action::Click { button: MouseButton::Left, x: 3, y: 4, hold_ms: 50 },
                Action::Chars { text: "ab".into(), delay_ms: 50 },
                Action::MouseButton { button: MouseButton::Left, state: PressState::Down },
                Action::Wait { ms: 10 },
                Action::Text { text: "olá".into() },
            ],
        );
        assert_eq!(Macro::from_json(&m.to_json().unwrap()).unwrap(), m);
    }

    #[test]
    fn rejects_future_version() {
        let s = r#"{"version":99,"name":"x","actions":[]}"#;
        assert!(matches!(Macro::from_json(s), Err(MacroError::UnsupportedVersion(99))));
    }
}
