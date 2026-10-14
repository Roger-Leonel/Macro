//! Tema claro e colorido do Korvon Macro: janela cinza clara, tabela branca,
//! seleção azul e ícones coloridos.

use iced::widget::{button, container, text_input};
use iced::{border, Background, Border, Color, Shadow, Theme};

const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

pub const WINDOW: Color = rgb(0xf0f0f0);
pub const RIBBON: Color = rgb(0xfafafa);
pub const WHITE: Color = rgb(0xffffff);
pub const BORDER: Color = rgb(0xd4d4d4);
pub const HEADER: Color = rgb(0xf4f4f4);
pub const TEXT: Color = rgb(0x1f1f1f);
pub const MUTED: Color = rgb(0x707070);
pub const ACCENT: Color = rgb(0x2b79d6);
pub const SELECT: Color = rgb(0xa4cdfa);
pub const HOVER: Color = rgb(0xe5f0fc);
pub const PLAYING: Color = rgb(0xfff0b3);
pub const REC_RED: Color = rgb(0xd32f2f);
pub const PLAY_GREEN: Color = rgb(0x2e7d32);

pub const RADIUS: f32 = 4.0;

pub fn theme() -> Theme {
    Theme::custom(
        "Korvon".to_string(),
        iced::theme::Palette {
            background: WINDOW,
            text: TEXT,
            primary: ACCENT,
            success: PLAY_GREEN,
            danger: REC_RED,
        },
    )
}

fn outline(color: Color, radius: f32) -> Border {
    Border { color, width: 1.0, radius: border::rounded(radius).radius }
}

fn faded(c: Color) -> Color {
    Color { a: c.a * 0.45, ..c }
}

pub fn surface(color: Color, radius: f32, line: Option<Color>) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        text_color: None,
        background: Some(Background::Color(color)),
        border: Border {
            color: line.unwrap_or(Color::TRANSPARENT),
            width: if line.is_some() { 1.0 } else { 0.0 },
            radius: border::rounded(radius).radius,
        },
        shadow: Shadow::default(),
    }
}

/// Botão grande da faixa de opções: sem preenchimento até o hover.
pub fn ribbon_button(_: &Theme, status: button::Status) -> button::Style {
    let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: hot.then_some(Background::Color(HOVER)),
        text_color: if status == button::Status::Disabled { faded(MUTED) } else { TEXT },
        border: outline(if hot { rgb(0x9cc4f0) } else { Color::TRANSPARENT }, RADIUS),
        shadow: Shadow::default(),
    }
}

/// Aba do topo. `file` = aba "Arquivo", destacada em azul.
pub fn tab_style(active: bool, file: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        if file {
            return button::Style {
                background: Some(Background::Color(if hot || active { rgb(0x1f66bd) } else { ACCENT })),
                text_color: WHITE,
                border: Border { radius: border::top(RADIUS), ..Border::default() },
                shadow: Shadow::default(),
            };
        }
        button::Style {
            background: if active { Some(Background::Color(RIBBON)) } else if hot { Some(Background::Color(HOVER)) } else { None },
            text_color: if active { TEXT } else { rgb(0x444444) },
            border: Border {
                color: if active { BORDER } else { Color::TRANSPARENT },
                width: 1.0,
                radius: border::top(RADIUS),
            },
            shadow: Shadow::default(),
        }
    }
}

/// Item do menu Arquivo.
pub fn menu_item(_: &Theme, status: button::Status) -> button::Style {
    let hot = status == button::Status::Hovered || status == button::Status::Pressed;
    button::Style {
        background: hot.then_some(Background::Color(SELECT)),
        text_color: if status == button::Status::Disabled { faded(MUTED) } else { TEXT },
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

/// Linha da tabela de ações.
pub fn row_style(selected: bool, playing: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if selected {
            SELECT
        } else if playing {
            PLAYING
        } else if status == button::Status::Hovered {
            HOVER
        } else {
            WHITE
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: TEXT,
            border: Border::default(),
            shadow: Shadow::default(),
        }
    }
}

/// Botão de dia da semana (liga/desliga) do formulário de agendamento.
pub fn day_button(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hot = status == button::Status::Hovered;
        button::Style {
            background: Some(Background::Color(if on { ACCENT } else if hot { HOVER } else { WHITE })),
            text_color: if on { WHITE } else { TEXT },
            border: outline(if on { ACCENT } else { rgb(0xbdbdbd) }, RADIUS),
            shadow: Shadow::default(),
        }
    }
}

pub fn input(_: &Theme, status: text_input::Status) -> text_input::Style {
    let line = match status {
        text_input::Status::Focused => ACCENT,
        text_input::Status::Hovered => rgb(0x9a9a9a),
        _ => rgb(0xbdbdbd),
    };
    text_input::Style {
        background: Background::Color(WHITE),
        border: outline(line, RADIUS),
        icon: MUTED,
        placeholder: rgb(0x9a9a9a),
        value: TEXT,
        selection: Color { a: 0.35, ..ACCENT },
    }
}
