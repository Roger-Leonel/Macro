//! Ícones próprios do Korvon Macro, desenhados como SVG coloridos (24×24).
//! Os ícones mantêm as cores originais; quando desabilitados viram uma silhueta cinza.

use iced::widget::{svg, Svg};
use iced::{Color, Length, Theme};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    // Arquivo e barra de ferramentas
    New,
    Open,
    Explorer,
    Save,
    Import,
    Play,
    Record,
    Stop,
    Check,
    Delete,
    Up,
    Down,
    Lines,
    Speed,
    Info,
    // Tipos de ação
    Cursor,
    MouseLeft,
    MouseRight,
    Wheel,
    Keyboard,
    Clock,
    Window,
    Tag,
    Goto,
    Repeat,
    Variable,
    Condition,
}

const GREY: &str = "#6b7785";
const BLUE: &str = "#3b82f6";

fn mouse_body(extra: &str) -> String {
    format!(
        r##"<rect x="6" y="3" width="12" height="18" rx="6" fill="#f4f6f8" stroke="{GREY}" stroke-width="1.4"/>{extra}<path d="M12 3v7M6.5 10h11" stroke="{GREY}" stroke-width="1.2" fill="none"/>"##
    )
}

fn body(i: Icon) -> String {
    match i {
        Icon::New => format!(r##"<path d="M6 2.5h8l5 5V21.5H6z" fill="#fff" stroke="{GREY}" stroke-width="1.4" stroke-linejoin="round"/><path d="M14 2.5v5h5" fill="#dbe7fb" stroke="{GREY}" stroke-width="1.4" stroke-linejoin="round"/>"##),
        Icon::Open => r##"<path d="M2.5 7a2 2 0 0 1 2-2h5l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2z" fill="#f6c244" stroke="#c28b0a" stroke-width="1.2" stroke-linejoin="round"/>"##.into(),
        Icon::Explorer => r##"<path d="M2.5 6a2 2 0 0 1 2-2h4.5l2 2.2H19.5a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2z" fill="#f6c244" stroke="#c28b0a" stroke-width="1.2" stroke-linejoin="round"/><path d="M2.5 10.5h19V18a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2z" fill="#ffe08a"/><path d="M2.5 15.5h19V18a2 2 0 0 1-2 2h-15a2 2 0 0 1-2-2z" fill="#3b82f6"/>"##.into(),
        Icon::Save => r##"<path d="M4 3h13l3 3v15H4z" fill="#3b82f6" stroke="#1e56b8" stroke-width="1.2" stroke-linejoin="round"/><rect x="7" y="3" width="9" height="6" fill="#fff"/><rect x="7" y="13" width="10" height="8" rx="1" fill="#dbe7fb"/>"##.into(),
        Icon::Import => format!(r##"<path d="M12 3v11M7.5 10l4.5 4.5 4.5-4.5" stroke="#22a05b" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/><path d="M4 15v5h16v-5" stroke="{GREY}" stroke-width="1.8" fill="none" stroke-linejoin="round"/>"##),
        Icon::Play => r##"<path d="M7 4l13 8-13 8z" fill="#4caf50" stroke="#2e7d32" stroke-width="1.2" stroke-linejoin="round"/>"##.into(),
        Icon::Record => r##"<circle cx="12" cy="12" r="9" fill="#ef5350" stroke="#c62828" stroke-width="1.2"/>"##.into(),
        Icon::Stop => r##"<rect x="4" y="4" width="16" height="16" rx="2" fill="#7fb0ea" stroke="#3b7dd6" stroke-width="1.2"/>"##.into(),
        Icon::Check => r##"<circle cx="12" cy="12" r="9" fill="#4caf50" stroke="#2e7d32" stroke-width="1.2"/><path d="M7.5 12.5l3 3 6-6.5" stroke="#fff" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##.into(),
        Icon::Delete => r##"<path d="M5 5l14 14M19 5L5 19" stroke="#e53935" stroke-width="3" stroke-linecap="round"/>"##.into(),
        Icon::Up => format!(r##"<path d="M12 19V6M6 11l6-6 6 6" stroke="{BLUE}" stroke-width="2.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##),
        Icon::Down => format!(r##"<path d="M12 5v13M6 13l6 6 6-6" stroke="{BLUE}" stroke-width="2.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##),
        Icon::Lines => format!(r##"<path d="M5 6h14M5 12h14M5 18h14" stroke="{GREY}" stroke-width="2" stroke-linecap="round"/><path d="M3 6h.01M3 12h.01M3 18h.01" stroke="{BLUE}" stroke-width="2.4" stroke-linecap="round"/>"##),
        Icon::Speed => format!(r##"<path d="M4 17a8 8 0 1 1 16 0" stroke="{GREY}" stroke-width="2" fill="none" stroke-linecap="round"/><path d="M12 17l4-5" stroke="#ef5350" stroke-width="2.2" stroke-linecap="round"/>"##),
        Icon::Info => format!(r##"<circle cx="12" cy="12" r="9" fill="{BLUE}"/><path d="M12 11v6M12 7.5v.01" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/>"##),
        Icon::Cursor => r##"<path d="M6 3l12 7-5 1.5L11 17z" fill="#3b82f6" stroke="#1e56b8" stroke-width="1" stroke-linejoin="round"/>"##.into(),
        Icon::MouseLeft => mouse_body(r##"<path d="M12 3a6 6 0 0 0-6 6v1h6z" fill="#3b82f6"/>"##),
        Icon::MouseRight => mouse_body(r##"<path d="M12 3a6 6 0 0 1 6 6v1h-6z" fill="#3b82f6"/>"##),
        Icon::Wheel => mouse_body(r##"<rect x="10.8" y="5.5" width="2.4" height="5" rx="1.2" fill="#f59e0b"/>"##),
        Icon::Keyboard => format!(r##"<rect x="2.5" y="6.5" width="19" height="11" rx="2" fill="#f4f6f8" stroke="{GREY}" stroke-width="1.4"/><path d="M6 10h.01M9 10h.01M12 10h.01M15 10h.01M18 10h.01M7 14h10" stroke="{BLUE}" stroke-width="1.9" stroke-linecap="round"/>"##),
        Icon::Clock => format!(r##"<circle cx="12" cy="12" r="9" fill="#fff" stroke="{GREY}" stroke-width="1.6"/><path d="M12 7v5l3.5 2" stroke="{BLUE}" stroke-width="1.9" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##),
        Icon::Window => format!(r##"<rect x="3" y="4" width="18" height="16" rx="2" fill="#fff" stroke="{GREY}" stroke-width="1.4"/><path d="M3 6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v2.5H3z" fill="{BLUE}"/>"##),
        Icon::Tag => r##"<path d="M3 12V4h8l9 9-8 8z" fill="#f59e0b" stroke="#b9770a" stroke-width="1.2" stroke-linejoin="round"/><circle cx="7.5" cy="8.5" r="1.4" fill="#fff"/>"##.into(),
        Icon::Goto => r##"<path d="M4 12h14M13 6l6 6-6 6" stroke="#8b5cf6" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##.into(),
        Icon::Repeat => r##"<path d="M4 11V9a3 3 0 0 1 3-3h11M15 3l3 3-3 3M20 13v2a3 3 0 0 1-3 3H6M9 21l-3-3 3-3" stroke="#22a05b" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"##.into(),
        Icon::Variable => r##"<rect x="3" y="4" width="18" height="16" rx="3" fill="#14b8a6"/><path d="M8 9l8 6M16 9l-8 6" stroke="#fff" stroke-width="2" stroke-linecap="round"/>"##.into(),
        Icon::Condition => r##"<path d="M12 3l9 9-9 9-9-9z" fill="#f59e0b" stroke="#b9770a" stroke-width="1.2" stroke-linejoin="round"/><path d="M12 8v5M12 16v.01" stroke="#fff" stroke-width="2" stroke-linecap="round"/>"##.into(),
    }
}

fn markup(i: Icon) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none">{}</svg>"#, body(i)).into_bytes()
}

const ALL: [Icon; 27] = [
    Icon::New,
    Icon::Open,
    Icon::Explorer,
    Icon::Save,
    Icon::Import,
    Icon::Play,
    Icon::Record,
    Icon::Stop,
    Icon::Check,
    Icon::Delete,
    Icon::Up,
    Icon::Down,
    Icon::Lines,
    Icon::Speed,
    Icon::Info,
    Icon::Cursor,
    Icon::MouseLeft,
    Icon::MouseRight,
    Icon::Wheel,
    Icon::Keyboard,
    Icon::Clock,
    Icon::Window,
    Icon::Tag,
    Icon::Goto,
    Icon::Repeat,
    Icon::Variable,
    Icon::Condition,
];

/// Conjunto de ícones já carregados (criados uma vez, reaproveitados a cada quadro).
pub struct Icons(HashMap<Icon, svg::Handle>);

impl Icons {
    pub fn new() -> Self {
        Self(ALL.iter().map(|&i| (i, svg::Handle::from_memory(markup(i)))).collect())
    }

    /// Ícone quadrado de `size` px. Desabilitado = silhueta cinza clara.
    pub fn get(&self, i: Icon, size: f32, enabled: bool) -> Svg<'static, Theme> {
        let w = svg(self.0[&i].clone()).width(Length::Fixed(size)).height(Length::Fixed(size));
        if enabled {
            w
        } else {
            w.style(|_, _| svg::Style { color: Some(Color::from_rgb8(0xbb, 0xbb, 0xbb)) })
        }
    }
}
