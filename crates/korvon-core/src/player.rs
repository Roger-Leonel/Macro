use crate::action::{Action, PressState};
use crate::macro_file::Macro;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Destino da reprodução (SendInput no Windows, mock nos testes).
pub trait InputSink {
    fn perform(&mut self, action: &Action) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy)]
pub struct PlayOptions {
    /// 1.0 = velocidade gravada; 2.0 = o dobro.
    pub speed: f64,
    /// 0 = infinito.
    pub repeat: u32,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self { speed: 1.0, repeat: 1 }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlayOutcome {
    Finished,
    Cancelled,
    Failed(String),
}

fn interruptible_sleep(total: Duration, cancel: &AtomicBool) -> bool {
    let end = Instant::now() + total;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let now = Instant::now();
        if now >= end {
            return true;
        }
        std::thread::sleep((end - now).min(Duration::from_millis(10)));
    }
}

/// Substitui `${nome}` pelo valor da variável (vazio se não existir).
/// `${` sem fechamento é mantido literalmente.
pub fn expand_vars(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find('}') {
            Some(end) => {
                let name = &rest[start + 2..start + 2 + end];
                out.push_str(vars.get(name).map(String::as_str).unwrap_or(""));
                rest = &rest[start + 2 + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Posição de cada rótulo; erro se houver rótulo duplicado ou salto para rótulo inexistente.
pub fn resolve_labels(m: &Macro) -> Result<HashMap<String, usize>, String> {
    let mut labels = HashMap::new();
    for (i, a) in m.actions.iter().enumerate() {
        if let Action::Label { name } = a {
            if labels.insert(name.clone(), i).is_some() {
                return Err(format!("rótulo duplicado: \"{name}\""));
            }
        }
    }
    let check = |l: &str, i: usize| -> Result<(), String> {
        if l.is_empty() || labels.contains_key(l) {
            Ok(())
        } else {
            Err(format!("ação {}: rótulo \"{l}\" não existe", i + 1))
        }
    };
    for (i, a) in m.actions.iter().enumerate() {
        match a {
            Action::Goto { label } | Action::Repeat { label, .. } => {
                if label.is_empty() {
                    return Err(format!("ação {}: rótulo vazio", i + 1));
                }
                check(label, i)?;
            }
            Action::If { then_label, else_label, .. } => {
                check(then_label, i)?;
                check(else_label, i)?;
            }
            _ => {}
        }
    }
    Ok(labels)
}

struct Runner<'a> {
    sink: &'a mut dyn InputSink,
    cancel: &'a AtomicBool,
    speed: f64,
    /// Última posição do cursor conhecida (ponto de partida dos movimentos com duração).
    pos: Option<(i32, i32)>,
}

impl Runner<'_> {
    /// Dorme `ms` (ajustado pela velocidade). `false` se foi cancelado.
    fn sleep(&self, ms: f64) -> bool {
        interruptible_sleep(Duration::from_secs_f64((ms / 1000.0 / self.speed).max(0.0)), self.cancel)
    }

    fn key(&mut self, vk: u16, state: PressState) -> Result<(), String> {
        self.sink.perform(&Action::Key { vk, state })
    }

    fn jump(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.sink.perform(&Action::MouseMove { x, y, ms: 0 })?;
        self.pos = Some((x, y));
        Ok(())
    }

    /// Executa uma ação que não é controle de fluxo. `Ok(true)` = cancelado no meio.
    fn exec(&mut self, a: &Action) -> Result<bool, String> {
        match a {
            Action::MouseMove { x, y, ms } => {
                let (x, y, ms) = (*x, *y, *ms);
                match self.pos {
                    Some((px, py)) if ms > 0 => {
                        let steps = (ms / 8).clamp(1, 250);
                        for k in 1..=steps {
                            let nx = px + ((x - px) as i64 * k as i64 / steps as i64) as i32;
                            let ny = py + ((y - py) as i64 * k as i64 / steps as i64) as i32;
                            self.sink.perform(&Action::MouseMove { x: nx, y: ny, ms: 0 })?;
                            if !self.sleep(ms as f64 / steps as f64) {
                                return Ok(true);
                            }
                        }
                        self.pos = Some((x, y));
                    }
                    _ => {
                        self.jump(x, y)?;
                        if ms > 0 && !self.sleep(ms as f64) {
                            return Ok(true);
                        }
                    }
                }
            }
            Action::Click { button, x, y, hold_ms } => {
                self.jump(*x, *y)?;
                self.sink.perform(&Action::MouseButton { button: *button, state: PressState::Down })?;
                let cancelled = !self.sleep(*hold_ms as f64);
                // Sempre solta o botão, mesmo se cancelado, para não deixá-lo preso.
                self.sink.perform(&Action::MouseButton { button: *button, state: PressState::Up })?;
                return Ok(cancelled);
            }
            Action::KeyPress { vk, hold_ms } => {
                self.key(*vk, PressState::Down)?;
                let cancelled = !self.sleep(*hold_ms as f64);
                self.key(*vk, PressState::Up)?;
                return Ok(cancelled);
            }
            Action::Chars { text, delay_ms } => {
                for c in text.chars() {
                    match char_vk(c) {
                        Some(vk) => {
                            self.key(vk, PressState::Down)?;
                            self.key(vk, PressState::Up)?;
                        }
                        None => self.sink.perform(&Action::Text { text: c.to_string() })?,
                    }
                    if !self.sleep(*delay_ms as f64) {
                        return Ok(true);
                    }
                }
            }
            other => self.sink.perform(other)?,
        }
        Ok(false)
    }
}

/// Tecla virtual de letras, dígitos e espaço (sem Shift).
fn char_vk(c: char) -> Option<u16> {
    let u = c.to_ascii_uppercase();
    (u.is_ascii_uppercase() || u.is_ascii_digit() || u == ' ').then_some(u as u16)
}

fn expand_action(a: &Action, vars: &HashMap<String, String>) -> Action {
    match a {
        Action::Text { text } => Action::Text { text: expand_vars(text, vars) },
        Action::Chars { text, delay_ms } => Action::Chars { text: expand_vars(text, vars), delay_ms: *delay_ms },
        Action::FocusWindow { exe, title, rect } => Action::FocusWindow {
            exe: expand_vars(exe, vars),
            title: expand_vars(title, vars),
            rect: *rect,
        },
        other => other.clone(),
    }
}

pub fn play(
    m: &Macro,
    opts: PlayOptions,
    sink: &mut dyn InputSink,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(usize, usize),
) -> PlayOutcome {
    let labels = match resolve_labels(m) {
        Ok(l) => l,
        Err(e) => return PlayOutcome::Failed(e),
    };
    let speed = if opts.speed.is_finite() && opts.speed > 0.0 { opts.speed } else { 1.0 };
    let n = m.actions.len();
    let mut vars: HashMap<String, String> = HashMap::new();
    let mut runner = Runner { sink, cancel, speed, pos: None };
    let mut iteration = 0u32;
    loop {
        let mut counters: HashMap<usize, u32> = HashMap::new();
        let mut pc = 0usize;
        while pc < n {
            if cancel.load(Ordering::Relaxed) {
                return PlayOutcome::Cancelled;
            }
            on_progress(pc, n);
            let mut next = pc + 1;
            match &m.actions[pc] {
                Action::Wait { ms } => {
                    if !runner.sleep(*ms as f64) {
                        return PlayOutcome::Cancelled;
                    }
                }
                Action::Label { .. } => {}
                Action::Goto { label } => next = labels[label],
                Action::Repeat { label, times } => {
                    let c = counters.entry(pc).or_insert(0);
                    *c += 1;
                    if *times == 0 || *c < *times {
                        next = labels[label];
                    } else {
                        counters.remove(&pc);
                    }
                }
                Action::SetVar { name, value } => {
                    vars.insert(name.clone(), expand_vars(value, &vars));
                }
                Action::If { var, op, value, then_label, else_label } => {
                    let left = vars.get(var).cloned().unwrap_or_default();
                    let right = expand_vars(value, &vars);
                    let target = if op.eval(&left, &right) { then_label } else { else_label };
                    if !target.is_empty() {
                        next = labels[target];
                    }
                }
                other => {
                    let a = expand_action(other, &vars);
                    match runner.exec(&a) {
                        Ok(false) => {}
                        Ok(true) => return PlayOutcome::Cancelled,
                        Err(e) => return PlayOutcome::Failed(e),
                    }
                }
            }
            pc = next;
        }
        iteration += 1;
        if opts.repeat != 0 && iteration >= opts.repeat {
            return PlayOutcome::Finished;
        }
        if n == 0 {
            return PlayOutcome::Finished; // evita laço infinito ocupado
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::CmpOp;

    struct Mock(Vec<Action>);
    impl InputSink for Mock {
        fn perform(&mut self, a: &Action) -> Result<(), String> {
            self.0.push(a.clone());
            Ok(())
        }
    }

    fn run(m: &Macro, repeat: u32) -> (PlayOutcome, Vec<Action>) {
        let mut sink = Mock(vec![]);
        let cancel = AtomicBool::new(false);
        let out = play(m, PlayOptions { speed: 1000.0, repeat }, &mut sink, &cancel, |_, _| {});
        (out, sink.0)
    }

    fn mv(x: i32) -> Action {
        Action::MouseMove { x, y: 0, ms: 0 }
    }
    fn label(n: &str) -> Action {
        Action::Label { name: n.into() }
    }

    #[test]
    fn repeats_and_skips_waits() {
        let m = Macro::new("t", vec![mv(1), Action::Wait { ms: 1 }]);
        let (out, done) = run(&m, 3);
        assert_eq!(out, PlayOutcome::Finished);
        assert_eq!(done.len(), 3);
    }

    #[test]
    fn cancel_stops_infinite_loop() {
        let m = Macro::new("t", vec![Action::Wait { ms: 5 }]);
        let mut sink = Mock(vec![]);
        let cancel = AtomicBool::new(true);
        let out = play(&m, PlayOptions { speed: 1.0, repeat: 0 }, &mut sink, &cancel, |_, _| {});
        assert_eq!(out, PlayOutcome::Cancelled);
    }

    #[test]
    fn repeat_block_runs_n_times_total() {
        let m = Macro::new(
            "t",
            vec![label("a"), mv(1), Action::Repeat { label: "a".into(), times: 3 }, mv(2)],
        );
        let (_, done) = run(&m, 1);
        assert_eq!(done, vec![mv(1), mv(1), mv(1), mv(2)]);
    }

    #[test]
    fn nested_repeat_resets_counter() {
        // externo 2x { interno 2x { mv(1) } }
        let m = Macro::new(
            "t",
            vec![
                label("o"),
                label("i"),
                mv(1),
                Action::Repeat { label: "i".into(), times: 2 },
                Action::Repeat { label: "o".into(), times: 2 },
            ],
        );
        let (_, done) = run(&m, 1);
        assert_eq!(done.len(), 4);
    }

    #[test]
    fn variables_and_condition() {
        // i=0; loop: i=${i}+... (sem aritmética): usa If sobre texto
        let m = Macro::new(
            "t",
            vec![
                Action::SetVar { name: "modo".into(), value: "B".into() },
                Action::If {
                    var: "modo".into(),
                    op: CmpOp::Equals,
                    value: "A".into(),
                    then_label: "a".into(),
                    else_label: "b".into(),
                },
                label("a"),
                mv(1),
                Action::Goto { label: "fim".into() },
                label("b"),
                Action::Text { text: "modo=${modo}".into() },
                label("fim"),
            ],
        );
        let (out, done) = run(&m, 1);
        assert_eq!(out, PlayOutcome::Finished);
        assert_eq!(done, vec![Action::Text { text: "modo=B".into() }]);
    }

    #[test]
    fn unknown_label_fails_before_running() {
        let m = Macro::new("t", vec![mv(1), Action::Goto { label: "x".into() }]);
        let (out, done) = run(&m, 1);
        assert!(matches!(out, PlayOutcome::Failed(_)));
        assert!(done.is_empty());
    }

    #[test]
    fn click_and_keypress_and_chars_expand_to_primitives() {
        use crate::action::MouseButton;
        let m = Macro::new(
            "t",
            vec![
                Action::Click { button: MouseButton::Left, x: 5, y: 6, hold_ms: 1 },
                Action::KeyPress { vk: 0x2E, hold_ms: 1 },
                Action::Chars { text: "a b!".into(), delay_ms: 0 },
            ],
        );
        let (_, done) = run(&m, 1);
        let k = |vk, state| Action::Key { vk, state };
        assert_eq!(done[0], Action::MouseMove { x: 5, y: 6, ms: 0 });
        assert_eq!(done[1], Action::MouseButton { button: MouseButton::Left, state: PressState::Down });
        assert_eq!(done[2], Action::MouseButton { button: MouseButton::Left, state: PressState::Up });
        assert_eq!(done[3], k(0x2E, PressState::Down));
        assert_eq!(done[4], k(0x2E, PressState::Up));
        assert_eq!(done[5], k(b'A' as u16, PressState::Down));
        assert_eq!(done[7], k(0x20, PressState::Down));
        assert_eq!(done[9], Action::Key { vk: b'B' as u16, state: PressState::Down });
        assert_eq!(done[11], Action::Text { text: "!".into() });
    }

    #[test]
    fn timed_move_interpolates_from_previous_position() {
        let m = Macro::new(
            "t",
            vec![
                Action::MouseMove { x: 0, y: 0, ms: 0 },
                Action::MouseMove { x: 100, y: 50, ms: 80 },
            ],
        );
        let (_, done) = run(&m, 1);
        assert_eq!(done.len(), 1 + 10); // salto inicial + 10 passos de 8 ms
        assert_eq!(done.last(), Some(&Action::MouseMove { x: 100, y: 50, ms: 0 }));
        assert_eq!(done[1], Action::MouseMove { x: 10, y: 5, ms: 0 });
    }

    #[test]
    fn expand_keeps_unclosed() {
        let mut v = HashMap::new();
        v.insert("a".to_string(), "1".to_string());
        assert_eq!(expand_vars("x${a}y${b}z${", &v), "x1yz${");
    }
}
