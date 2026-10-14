//! Linhas da tabela de ações: a lista de ações é agrupada para exibição
//! (todos os movimentos seguidos do mouse viram uma única linha "x, y » x, y").

use crate::icons::Icon;
use korvon_core::{Action, MouseButton, PressState};

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Faixa de ações cobertas por esta linha (índices inclusivos).
    pub start: usize,
    pub end: usize,
    pub icon: Icon,
    pub title: String,
    pub value: String,
    pub label: String,
}

impl Row {
    pub fn contains(&self, action_index: usize) -> bool {
        (self.start..=self.end).contains(&action_index)
    }
    pub fn len(&self) -> usize {
        self.end - self.start + 1
    }
}

fn button_name(b: MouseButton) -> &'static str {
    match b {
        MouseButton::Left => "esquerdo",
        MouseButton::Right => "direito",
        MouseButton::Middle => "do meio",
        MouseButton::X1 => "X1",
        MouseButton::X2 => "X2",
    }
}

fn button_icon(b: MouseButton) -> Icon {
    match b {
        MouseButton::Right => Icon::MouseRight,
        MouseButton::Middle => Icon::Wheel,
        _ => Icon::MouseLeft,
    }
}

/// Nome legível de um código de tecla virtual.
pub fn vk_name(vk: u16) -> String {
    let fixed = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x13 => "Pause",
        0x1B => "Esc",
        0x20 => "Espaço",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Seta ←",
        0x26 => "Seta ↑",
        0x27 => "Seta →",
        0x28 => "Seta ↓",
        0x2C => "Print Screen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x5B => "Win esquerda",
        0x5C => "Win direita",
        0x5D => "Menu",
        0xA0 => "Shift esquerdo",
        0xA1 => "Shift direito",
        0xA2 => "Ctrl esquerdo",
        0xA3 => "Ctrl direito",
        0xA4 => "Alt esquerdo",
        0xA5 => "Alt direito",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => format!("0x{vk:02X}"),
    }
}

/// Monta as linhas de exibição.
pub fn build(actions: &[Action]) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut cursor: Option<(i32, i32)> = None;
    let mut i = 0;
    while i < actions.len() {
        let row = |icon, title: &str, value: String, label: String| Row {
            start: i,
            end: i,
            icon,
            title: title.to_string(),
            value,
            label,
        };
        let r = match &actions[i] {
            Action::MouseMove { x, y, .. } => {
                let from = cursor.unwrap_or((*x, *y));
                let mut to = (*x, *y);
                let mut j = i;
                while let Some(Action::MouseMove { x, y, .. }) = actions.get(j + 1) {
                    to = (*x, *y);
                    j += 1;
                }
                cursor = Some(to);
                let mut r = row(Icon::Cursor, "Mover mouse", format!("{}, {} » {}, {}", from.0, from.1, to.0, to.1), String::new());
                r.end = j;
                r
            }
            Action::Click { button, x, y, .. } => {
                cursor = Some((*x, *y));
                row(
                    button_icon(*button),
                    &format!("Clique {}", button_name(*button)),
                    format!("{x}, {y}"),
                    String::new(),
                )
            }
            Action::MouseButton { button, state } => row(
                button_icon(*button),
                &format!(
                    "Botão {} {}",
                    button_name(*button),
                    if *state == PressState::Down { "pressionado" } else { "solto" }
                ),
                String::new(),
                String::new(),
            ),
            Action::Wheel { delta, horizontal } => row(
                Icon::Wheel,
                if *horizontal { "Roda horizontal" } else { "Roda do mouse" },
                if *delta > 0 { "para cima/direita".into() } else { "para baixo/esquerda".into() },
                String::new(),
            ),
            Action::Key { vk, state } => row(
                Icon::Keyboard,
                if *state == PressState::Down { "Tecla pressionar" } else { "Tecla soltar" },
                vk_name(*vk),
                String::new(),
            ),
            Action::KeyPress { vk, .. } => row(Icon::Keyboard, "Tecla", vk_name(*vk), String::new()),
            Action::Chars { text, .. } => row(Icon::Keyboard, "Caracteres", text.clone(), String::new()),
            Action::Text { text } => row(Icon::Keyboard, "Texto", text.clone(), String::new()),
            Action::Wait { ms } => row(Icon::Clock, "Esperar", format!("{ms} ms"), String::new()),
            Action::Label { name } => row(Icon::Tag, "Rótulo", String::new(), name.clone()),
            Action::Goto { label } => row(Icon::Goto, "Ir para rótulo", label.clone(), label.clone()),
            Action::Repeat { label, times } => row(
                Icon::Repeat,
                "Repetir",
                if *times == 0 { format!("de \"{label}\" infinitamente") } else { format!("de \"{label}\" {times}x") },
                label.clone(),
            ),
            Action::SetVar { name, value } => {
                row(Icon::Variable, "Definir variável", format!("{name} = {value}"), String::new())
            }
            Action::If { var, op, value, then_label, .. } => row(
                Icon::Condition,
                "Condição",
                format!("{var} {} {value}", op.label()),
                then_label.clone(),
            ),
            Action::FocusWindow { exe, title, .. } => {
                row(Icon::Window, "Troca de janela", format!("{exe}, {title}"), String::new())
            }
        };
        i = r.end + 1;
        rows.push(r);
    }
    rows
}

/// Duração estimada da macro, em milissegundos.
pub fn duration_ms(actions: &[Action]) -> u64 {
    actions
        .iter()
        .map(|a| match a {
            Action::Wait { ms } => *ms,
            Action::MouseMove { ms, .. } => *ms as u64,
            Action::Click { hold_ms, .. } => *hold_ms as u64,
            Action::KeyPress { hold_ms, .. } => *hold_ms as u64,
            Action::Chars { text, delay_ms } => text.chars().count() as u64 * *delay_ms as u64,
            _ => 0,
        })
        .sum()
}

pub fn format_duration(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Remove todas as ações da linha.
pub fn remove_row(actions: &mut Vec<Action>, row: &Row) {
    actions.drain(row.start..=row.end);
}

/// Troca duas linhas adjacentes (`a` imediatamente antes de `b`).
pub fn swap_adjacent(actions: &mut [Action], a: &Row, b: &Row) {
    debug_assert_eq!(a.end + 1, b.start);
    actions[a.start..=b.end].rotate_left(a.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(x: i32, y: i32) -> Action {
        Action::MouseMove { x, y, ms: 8 }
    }
    fn click(x: i32, y: i32) -> Action {
        Action::Click { button: MouseButton::Left, x, y, hold_ms: 50 }
    }

    #[test]
    fn consecutive_moves_collapse_into_one_row_with_start_and_end() {
        let a = vec![click(10, 10), mv(11, 11), mv(12, 12), mv(30, 40), click(30, 40), mv(50, 50)];
        let rows = build(&a);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[1].value, "10, 10 » 30, 40");
        assert_eq!((rows[1].start, rows[1].end), (1, 3));
        assert_eq!(rows[3].value, "30, 40 » 50, 50");
        assert_eq!(rows[0].title, "Clique esquerdo");
    }

    #[test]
    fn first_move_without_known_start_shows_its_own_point() {
        let rows = build(&[mv(5, 6), mv(7, 8)]);
        assert_eq!(rows[0].value, "5, 6 » 7, 8");
    }

    #[test]
    fn remove_and_swap_operate_on_whole_groups() {
        let mut a = vec![click(1, 1), mv(2, 2), mv(3, 3), Action::Wait { ms: 600 }];
        let rows = build(&a);
        assert_eq!(rows.len(), 3);
        swap_adjacent(&mut a, &rows[1], &rows[2]);
        assert_eq!(a, vec![click(1, 1), Action::Wait { ms: 600 }, mv(2, 2), mv(3, 3)]);
        let rows = build(&a);
        remove_row(&mut a, &rows[2]);
        assert_eq!(a, vec![click(1, 1), Action::Wait { ms: 600 }]);
    }

    #[test]
    fn duration_and_key_names() {
        let a = vec![Action::Wait { ms: 1500 }, mv(1, 1), Action::Chars { text: "ab".into(), delay_ms: 50 }];
        assert_eq!(duration_ms(&a), 1500 + 8 + 100);
        assert_eq!(format_duration(3_723_000), "01:02:03");
        assert_eq!(vk_name(0x2E), "Delete");
        assert_eq!(vk_name(0x41), "A");
        assert_eq!(vk_name(0x71), "F2");
    }
}
