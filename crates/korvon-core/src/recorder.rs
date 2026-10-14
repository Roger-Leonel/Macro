//! Gravador: converte eventos brutos (com timestamp) em ações, no estilo do Macro Recorder.
//!
//! Regras (deduzidas de gravações reais do original):
//! - Cada amostra do mouse vira um `MouseMove` cuja duração é o tempo desde o evento anterior.
//! - Só se grava `Wait` quando o intervalo é de `min_wait_ms` ou mais; o evento seguinte
//!   volta à duração padrão (`hold_ms`).
//! - Pressionar e soltar um botão sem mover vira um único `Click`.
//! - Letras, dígitos e espaço seguidos viram um único `Chars`; outras teclas viram `KeyPress`.
//! - Modificadores (Ctrl, Alt, Shift, Win) só geram `Key` down/up separados quando outra
//!   tecla é usada enquanto estão pressionados; sozinhos viram `KeyPress`.

use crate::action::{Action, MouseButton, PressState};

#[derive(Debug, Clone, Copy)]
pub struct RecorderOptions {
    /// Intervalos de inatividade a partir deste valor (ms) viram `Wait`.
    pub min_wait_ms: u64,
    /// Duração padrão (ms): retenção de cliques/teclas, atraso entre caracteres
    /// e duração do primeiro movimento depois de uma espera.
    pub hold_ms: u32,
    /// Movimentos menores que isto (px, distância de Chebyshev) são descartados.
    pub min_move_px: i32,
    pub record_moves: bool,
}

impl Default for RecorderOptions {
    fn default() -> Self {
        Self { min_wait_ms: 500, hold_ms: 50, min_move_px: 1, record_moves: true }
    }
}

struct PendingButton {
    button: MouseButton,
    pos: (i32, i32),
    t: u64,
}

struct HeldModifier {
    vk: u16,
    /// O `Key` down já foi gravado (outra tecla foi usada com ele).
    flushed: bool,
}

pub struct Recorder {
    opts: RecorderOptions,
    actions: Vec<Action>,
    last_t: Option<u64>,
    pos: Option<(i32, i32)>,
    after_wait: bool,
    /// Índice do `Chars` ainda aberto para receber mais letras.
    open_chars: Option<usize>,
    pending: Option<PendingButton>,
    mods: Vec<HeldModifier>,
    /// Botões cujo "pressionar" foi visto durante esta gravação.
    buttons_down: Vec<MouseButton>,
    /// Menor índice alterado desde o último `drain_changes`.
    dirty_from: usize,
}

fn is_modifier(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// Letras, dígitos e espaço: o que o original agrupa em "caracteres".
fn char_for_vk(vk: u16) -> Option<char> {
    match vk {
        0x30..=0x39 | 0x41..=0x5A | 0x20 => Some(vk as u8 as char),
        _ => None,
    }
}

impl Recorder {
    pub fn new(opts: RecorderOptions) -> Self {
        Self {
            opts,
            actions: Vec::new(),
            last_t: None,
            pos: None,
            after_wait: true,
            open_chars: None,
            pending: None,
            mods: Vec::new(),
            buttons_down: Vec::new(),
            dirty_from: 0,
        }
    }

    fn commit(&mut self, a: Action) {
        self.actions.push(a);
    }

    fn close_chars(&mut self) {
        self.open_chars = None;
    }

    /// Grava o `Key` down dos modificadores que ainda estavam pendentes.
    fn flush_mods(&mut self) {
        let pending: Vec<u16> = self.mods.iter().filter(|m| !m.flushed).map(|m| m.vk).collect();
        if pending.is_empty() {
            return;
        }
        self.close_chars();
        for vk in pending {
            self.commit(Action::Key { vk, state: PressState::Down });
        }
        for m in &mut self.mods {
            m.flushed = true;
        }
    }

    /// Converte um botão pressionado e ainda não resolvido em `MouseMove` + `Down`.
    fn flush_pending_button(&mut self) {
        if let Some(p) = self.pending.take() {
            self.close_chars();
            self.commit(Action::MouseMove { x: p.pos.0, y: p.pos.1, ms: 0 });
            self.commit(Action::MouseButton { button: p.button, state: PressState::Down });
        }
    }

    /// `t_ms` é um relógio monotônico em ms; `pos` é a posição do cursor para eventos de mouse.
    pub fn push(&mut self, t_ms: u64, action: Action, pos: Option<(i32, i32)>) {
        // Soltar um botão que não foi pressionado durante a gravação (ex.: o clique em
        // "Gravar") não faz parte da macro e nem conta como atividade.
        if let Action::MouseButton { button, state: PressState::Up } = &action {
            if !self.buttons_down.contains(button) {
                return;
            }
        }
        let gap = self.last_t.map(|l| t_ms.saturating_sub(l));
        self.last_t = Some(t_ms);

        if let Some(g) = gap {
            if g >= self.opts.min_wait_ms {
                self.flush_pending_button();
                self.close_chars();
                self.commit(Action::Wait { ms: g });
                self.after_wait = true;
            }
        }

        match action {
            Action::MouseMove { x, y, .. } => {
                if !self.opts.record_moves {
                    self.pos = Some((x, y));
                    return;
                }
                self.flush_pending_button();
                if let Some((lx, ly)) = self.pos {
                    if (x - lx).abs().max((y - ly).abs()) < self.opts.min_move_px {
                        return;
                    }
                }
                self.pos = Some((x, y));
                let ms = if self.after_wait { self.opts.hold_ms } else { gap.unwrap_or(1).max(1) as u32 };
                self.after_wait = false;
                self.close_chars();
                self.commit(Action::MouseMove { x, y, ms });
            }
            Action::MouseButton { button, state: PressState::Down } => {
                self.flush_mods();
                self.close_chars();
                let p = pos.or(self.pos).unwrap_or((0, 0));
                self.pos = Some(p);
                if !self.buttons_down.contains(&button) {
                    self.buttons_down.push(button);
                }
                self.pending = Some(PendingButton { button, pos: p, t: t_ms });
            }
            Action::MouseButton { button, state: PressState::Up } => {
                self.after_wait = false;
                self.buttons_down.retain(|b| *b != button);
                match self.pending.take() {
                    Some(p)
                        if p.button == button
                            && t_ms.saturating_sub(p.t) < self.opts.min_wait_ms
                            && pos.map_or(true, |q| q == p.pos) =>
                    {
                        self.commit(Action::Click { button, x: p.pos.0, y: p.pos.1, hold_ms: self.opts.hold_ms });
                    }
                    other => {
                        self.pending = other;
                        self.flush_pending_button();
                        self.commit(Action::MouseButton { button, state: PressState::Up });
                    }
                }
            }
            Action::Wheel { .. } => {
                self.flush_pending_button();
                self.close_chars();
                self.after_wait = false;
                self.commit(action);
            }
            Action::Key { vk, state: PressState::Down } => {
                self.after_wait = false;
                if is_modifier(vk) {
                    if !self.mods.iter().any(|m| m.vk == vk) {
                        self.mods.push(HeldModifier { vk, flushed: false });
                    }
                    return;
                }
                self.flush_pending_button();
                self.flush_mods();
                match char_for_vk(vk) {
                    Some(c) => match self.open_chars {
                        Some(i) => {
                            if let Action::Chars { text, .. } = &mut self.actions[i] {
                                text.push(c);
                            }
                            self.dirty_from = self.dirty_from.min(i);
                        }
                        None => {
                            self.open_chars = Some(self.actions.len());
                            self.commit(Action::Chars { text: c.to_string(), delay_ms: self.opts.hold_ms });
                        }
                    },
                    None => {
                        self.close_chars();
                        self.commit(Action::KeyPress { vk, hold_ms: self.opts.hold_ms });
                    }
                }
            }
            Action::Key { vk, state: PressState::Up } => {
                self.after_wait = false;
                if !is_modifier(vk) {
                    return; // já agrupada na hora do "down"
                }
                if let Some(i) = self.mods.iter().position(|m| m.vk == vk) {
                    let m = self.mods.remove(i);
                    self.close_chars();
                    if m.flushed {
                        self.commit(Action::Key { vk, state: PressState::Up });
                    } else {
                        self.commit(Action::KeyPress { vk, hold_ms: self.opts.hold_ms });
                    }
                }
            }
            other => {
                self.close_chars();
                self.commit(other);
            }
        }
    }

    /// Ações novas ou alteradas desde a última chamada: `(índice inicial, ações a partir dele)`.
    /// Permite à interface atualizar a lista em tempo real sem copiar tudo a cada vez.
    pub fn drain_changes(&mut self) -> (usize, Vec<Action>) {
        let from = self.dirty_from.min(self.actions.len());
        let changed = self.actions[from..].to_vec();
        self.dirty_from = self.open_chars.unwrap_or(self.actions.len());
        (from, changed)
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Encerra a gravação e devolve as ações. Um botão ainda pressionado é gravado como `Down`;
    /// modificadores presos recebem o `Up` correspondente.
    pub fn finish(mut self) -> Vec<Action> {
        self.flush_pending_button();
        self.close_chars();
        let held: Vec<u16> = self.mods.iter().filter(|m| m.flushed).map(|m| m.vk).collect();
        for vk in held {
            self.actions.push(Action::Key { vk, state: PressState::Up });
        }
        self.actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec() -> Recorder {
        Recorder::new(RecorderOptions::default())
    }
    fn mv(r: &mut Recorder, t: u64, x: i32, y: i32) {
        r.push(t, Action::MouseMove { x, y, ms: 0 }, Some((x, y)));
    }
    fn down(r: &mut Recorder, t: u64, vk: u16) {
        r.push(t, Action::Key { vk, state: PressState::Down }, None);
    }
    fn up(r: &mut Recorder, t: u64, vk: u16) {
        r.push(t, Action::Key { vk, state: PressState::Up }, None);
    }
    fn btn(r: &mut Recorder, t: u64, s: PressState, p: (i32, i32)) {
        r.push(t, Action::MouseButton { button: MouseButton::Left, state: s }, Some(p));
    }

    #[test]
    fn move_duration_is_elapsed_time_and_first_is_default() {
        let mut r = rec();
        mv(&mut r, 1000, 10, 10);
        mv(&mut r, 1008, 20, 10);
        mv(&mut r, 1024, 30, 10);
        let a = r.finish();
        assert_eq!(
            a,
            vec![
                Action::MouseMove { x: 10, y: 10, ms: 50 },
                Action::MouseMove { x: 20, y: 10, ms: 8 },
                Action::MouseMove { x: 30, y: 10, ms: 16 },
            ]
        );
    }

    #[test]
    fn wait_only_from_500ms_and_next_move_resets_to_default() {
        let mut r = rec();
        mv(&mut r, 0, 1, 1);
        mv(&mut r, 481, 5, 5); // < 500: vira duração do movimento
        mv(&mut r, 1100, 9, 9); // 619 ms: Wait
        mv(&mut r, 1110, 12, 12);
        let a = r.finish();
        assert_eq!(a[1], Action::MouseMove { x: 5, y: 5, ms: 481 });
        assert_eq!(a[2], Action::Wait { ms: 619 });
        assert_eq!(a[3], Action::MouseMove { x: 9, y: 9, ms: 50 });
        assert_eq!(a[4], Action::MouseMove { x: 12, y: 12, ms: 10 });
    }

    #[test]
    fn button_press_release_in_place_is_a_click() {
        let mut r = rec();
        mv(&mut r, 0, 100, 100);
        btn(&mut r, 20, PressState::Down, (100, 100));
        btn(&mut r, 90, PressState::Up, (100, 100));
        let a = r.finish();
        assert_eq!(a[1], Action::Click { button: MouseButton::Left, x: 100, y: 100, hold_ms: 50 });
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn release_without_a_recorded_press_is_ignored() {
        let mut r = rec();
        btn(&mut r, 0, PressState::Up, (5, 5)); // soltou o botão de gravar
        mv(&mut r, 20, 9, 9);
        let a = r.finish();
        assert_eq!(a, vec![Action::MouseMove { x: 9, y: 9, ms: 50 }]);
    }

    #[test]
    fn dragging_keeps_down_move_up() {
        let mut r = rec();
        btn(&mut r, 0, PressState::Down, (10, 10));
        mv(&mut r, 10, 30, 30);
        btn(&mut r, 20, PressState::Up, (30, 30));
        let a = r.finish();
        assert_eq!(a[0], Action::MouseMove { x: 10, y: 10, ms: 0 });
        assert_eq!(a[1], Action::MouseButton { button: MouseButton::Left, state: PressState::Down });
        assert!(matches!(a[2], Action::MouseMove { x: 30, y: 30, .. }));
        assert_eq!(a[3], Action::MouseButton { button: MouseButton::Left, state: PressState::Up });
    }

    #[test]
    fn letters_merge_into_chars_and_wait_splits_them() {
        let mut r = rec();
        for (i, vk) in [0x41u16, 0x42, 0x43].iter().enumerate() {
            down(&mut r, 100 + i as u64 * 80, *vk);
            up(&mut r, 130 + i as u64 * 80, *vk);
        }
        down(&mut r, 2000, 0x44);
        up(&mut r, 2030, 0x44);
        let a = r.finish();
        assert_eq!(a[0], Action::Chars { text: "ABC".into(), delay_ms: 50 });
        assert!(matches!(a[1], Action::Wait { .. }));
        assert_eq!(a[2], Action::Chars { text: "D".into(), delay_ms: 50 });
    }

    #[test]
    fn modifier_held_with_other_key_is_down_chars_up() {
        let mut r = rec();
        down(&mut r, 0, 0x5B); // Win
        down(&mut r, 100, 0x52); // R
        up(&mut r, 130, 0x52);
        up(&mut r, 200, 0x5B);
        let a = r.finish();
        assert_eq!(
            a,
            vec![
                Action::Key { vk: 0x5B, state: PressState::Down },
                Action::Chars { text: "R".into(), delay_ms: 50 },
                Action::Key { vk: 0x5B, state: PressState::Up },
            ]
        );
    }

    #[test]
    fn lone_modifier_and_special_key_are_key_presses() {
        let mut r = rec();
        down(&mut r, 0, 0xA4); // Alt sozinho
        up(&mut r, 60, 0xA4);
        down(&mut r, 100, 0x2E); // Delete
        up(&mut r, 140, 0x2E);
        let a = r.finish();
        assert_eq!(a[0], Action::KeyPress { vk: 0xA4, hold_ms: 50 });
        assert_eq!(a[1], Action::KeyPress { vk: 0x2E, hold_ms: 50 });
    }

    #[test]
    fn drain_changes_reports_growing_chars_in_place() {
        let mut r = rec();
        down(&mut r, 0, 0x41);
        let (from, a) = r.drain_changes();
        assert_eq!((from, a.len()), (0, 1));
        down(&mut r, 40, 0x42);
        let (from, a) = r.drain_changes();
        assert_eq!(from, 0); // o Chars aberto foi alterado
        assert_eq!(a, vec![Action::Chars { text: "AB".into(), delay_ms: 50 }]);
        mv(&mut r, 80, 5, 5);
        let (from, a) = r.drain_changes();
        assert_eq!(from, 0);
        assert_eq!(a.len(), 2);
        let (from, a) = r.drain_changes();
        assert_eq!((from, a.len()), (2, 0));
    }
}
