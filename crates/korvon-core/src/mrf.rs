//! Importador de macros do Macro Recorder (`.mrf`, versões 4.x).
//!
//! Só lê o arquivo e converte os tipos de ação cujo significado foi validado
//! contra arquivos reais (ver `docs/analise-mrf.md`). O restante é descartado
//! com um aviso.

use crate::action::{Action, MouseButton, PressState};
use crate::macro_file::Macro;

#[derive(Debug, thiserror::Error)]
pub enum MrfError {
    #[error("arquivo truncado ou corrompido (posição {0})")]
    Truncated(usize),
    #[error("valor inesperado: {0}")]
    Invalid(&'static str),
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
}

/// Versão máxima do formato que este leitor conhece.
const MAX_KNOWN_VERSION: u32 = 4_000_043;
const MAX_ACTIONS: u32 = 5_000_000;
const MAX_DEPTH: u32 = 16;

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn raw(&mut self, n: usize) -> Result<&'a [u8], MrfError> {
        let end = self.p.checked_add(n).ok_or(MrfError::Truncated(self.p))?;
        let s = self.d.get(self.p..end).ok_or(MrfError::Truncated(self.p))?;
        self.p = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, MrfError> {
        Ok(u32::from_le_bytes(self.raw(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, MrfError> {
        Ok(self.u32()? as i32)
    }
    fn u8(&mut self) -> Result<u8, MrfError> {
        Ok(self.raw(1)?[0])
    }
    /// `u32` com o tamanho em bytes + UTF-16LE.
    fn string(&mut self) -> Result<String, MrfError> {
        let n = self.u32()? as usize;
        if n == 0 {
            return Ok(String::new());
        }
        let b = self.raw(n)?;
        let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(String::from_utf16_lossy(&units))
    }
}

/// Registro de ação como gravado no arquivo.
#[derive(Debug, Default, Clone)]
pub struct RawAction {
    pub kind: i32,
    pub delay: i32,
    pub p: [i32; 5],
    pub count: i32,
    pub enabled: bool,
    pub text: String,
    pub label: String,
    pub sub_actions: usize,
}

fn read_action(r: &mut Reader, v: u32, depth: u32) -> Result<RawAction, MrfError> {
    if depth > MAX_DEPTH {
        return Err(MrfError::Invalid("subações aninhadas demais"));
    }
    let mut a = RawAction { enabled: true, ..Default::default() };
    a.kind = r.i32()?;
    a.delay = r.i32()?;
    for x in a.p.iter_mut() {
        *x = r.i32()?;
    }
    a.count = r.i32()?;
    if v > 0xf4273 {
        r.i32()?; // ExtraData
    }
    if v > 3_000_000 {
        a.enabled = r.u8()? != 0;
        r.i32()?; // GroupColor
    }
    if v > 0x2dc6c7 {
        r.u8()?; // EndOfGroup
        r.u8()?; // Expanded
    }
    if v > 3_999_999 {
        r.u8()?; // IsBreakpoint
    }
    a.text = r.string()?;
    if v > 0x3d0915 {
        r.string()?; // Text2
    }
    a.label = r.string()?;
    if v > 0xf4273 {
        r.string()?; // TargetLabel
        r.string()?; // TimeoutLabel
    }
    if v > 0x1e848a {
        r.string()?; // Comment
    }
    if v > 3_000_000 {
        r.string()?; // Group
        for _ in 0..2 {
            let n = r.u32()?;
            if n > MAX_ACTIONS {
                return Err(MrfError::Invalid("quantidade de subações"));
            }
            for _ in 0..n {
                read_action(r, v, depth + 1)?;
                a.sub_actions += 1;
            }
        }
    }
    Ok(a)
}

/// Lê todas as ações do arquivo.
pub fn parse(bytes: &[u8]) -> Result<(u32, Vec<RawAction>), MrfError> {
    let mut r = Reader { d: bytes, p: 0 };
    let _min = r.u32()?;
    let v = r.u32()?;
    if !(0xf4273..=MAX_KNOWN_VERSION).contains(&v) {
        return Err(MrfError::Invalid("versão de arquivo não suportada"));
    }
    let monitors = r.u32()?;
    if monitors > 64 {
        return Err(MrfError::Invalid("quantidade de monitores"));
    }
    r.raw(16)?; // retângulo da área de trabalho
    r.raw(12 * monitors as usize)?;
    // Cabeçalho: 48 B na versão atual; 28 B entre 2 000 013 e 3 000 000.
    let hdr_len = if v >= 0x3d092b { 48 } else if v >= 0x2dc6d0 || v >= 0x1e848d { 28 } else { 24 };
    let hdr = r.raw(hdr_len)?;
    let count = u32::from_le_bytes(hdr[0..4].try_into().unwrap());
    if count > MAX_ACTIONS {
        return Err(MrfError::Invalid("quantidade de ações"));
    }
    let mut actions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        actions.push(read_action(&mut r, v, 0)?);
    }
    Ok((v, actions))
}

fn button_for(n: i32) -> Option<MouseButton> {
    match n {
        1 => Some(MouseButton::Left),
        2 => Some(MouseButton::Right),
        3 => Some(MouseButton::Middle),
        _ => None,
    }
}

fn wait(ms: i32) -> Option<Action> {
    (ms > 0).then_some(Action::Wait { ms: ms as u64 })
}

/// Converte as ações lidas para o modelo do Korvon. Devolve também os avisos.
pub fn convert(name: &str, raw: &[RawAction]) -> (Macro, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped = std::collections::BTreeMap::<i32, usize>::new();
    let mut cursor: Option<(i32, i32)> = None;
    for (i, a) in raw.iter().enumerate() {
        if !a.enabled {
            continue;
        }
        if !a.label.is_empty() {
            out.push(Action::Label { name: a.label.clone() });
        }
        if a.sub_actions > 0 {
            warnings.push(format!("ação {}: subações/grupo ignorados", i + 1));
        }
        let p = a.p;
        match a.kind {
            0 => {
                // O original guarda ponto inicial e final; só precisamos do inicial
                // quando ele não coincide com onde o cursor já está.
                if cursor != Some((p[0], p[1])) {
                    out.push(Action::MouseMove { x: p[0], y: p[1], ms: 0 });
                }
                out.push(Action::MouseMove { x: p[2], y: p[3], ms: a.delay.max(0) as u32 });
                cursor = Some((p[2], p[3]));
            }
            3 => match (button_for(p[0]), p[1]) {
                (Some(button), 1) => {
                    out.push(Action::Click { button, x: p[2], y: p[3], hold_ms: a.delay.max(0) as u32 });
                    cursor = Some((p[2], p[3]));
                }
                _ => {
                    warnings.push(format!("ação {}: clique com botão/modo {}/{} não suportado", i + 1, p[0], p[1]));
                }
            },
            4 => out.push(Action::Key {
                vk: p[0] as u16,
                state: if p[2] == 1 { PressState::Down } else { PressState::Up },
            }),
            5 => out.push(Action::KeyPress { vk: p[0] as u16, hold_ms: a.delay.max(0) as u32 }),
            6 => out.push(Action::Chars { text: a.text.clone(), delay_ms: a.delay.max(0) as u32 }),
            7 => out.extend(wait(p[0])),
            8 => {
                let mut parts = a.text.splitn(3, '|');
                let exe = parts.next().unwrap_or("").to_string();
                let title = parts.next().unwrap_or("").to_string();
                let rect = (p[2] > 0 && p[3] > 0).then_some([p[0], p[1], p[2], p[3]]);
                out.push(Action::FocusWindow { exe, title, rect });
            }
            other => *skipped.entry(other).or_default() += 1,
        }
    }
    for (kind, n) in skipped {
        warnings.push(format!("tipo de ação {kind} não suportado: {n} ação(ões) ignorada(s)"));
    }
    (Macro::new(name, out), warnings)
}

/// Lê um `.mrf` do disco e converte.
pub fn import_file(path: &std::path::Path) -> Result<(Macro, Vec<String>), MrfError> {
    let bytes = std::fs::read(path)?;
    let (_, raw) = parse(&bytes)?;
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Importada");
    Ok(convert(name, &raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(out: &mut Vec<u8>, t: &str) {
        let u: Vec<u8> = t.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        out.extend((u.len() as u32).to_le_bytes());
        out.extend(u);
    }

    fn action(out: &mut Vec<u8>, kind: i32, delay: i32, p: [i32; 5], text: &str, label: &str) {
        for v in [kind, delay, p[0], p[1], p[2], p[3], p[4], 1, 0] {
            out.extend(v.to_le_bytes());
        }
        out.push(1); // enabled
        out.extend(0i32.to_le_bytes()); // group color
        out.extend([0, 1]); // eog, expanded
        out.push(0); // breakpoint
        s(out, text);
        s(out, ""); // text2
        s(out, label);
        for _ in 0..4 {
            s(out, ""); // target, timeout, comment, group
        }
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
    }

    fn file(actions: &[(i32, i32, [i32; 5], &str, &str)]) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend(4_000_043u32.to_le_bytes());
        o.extend(4_000_043u32.to_le_bytes());
        o.extend(1u32.to_le_bytes()); // monitores
        o.extend([0u8; 16]);
        o.extend([0u8; 12]);
        let mut h = [0u8; 48];
        h[0..4].copy_from_slice(&(actions.len() as u32).to_le_bytes());
        o.extend(h);
        for a in actions {
            action(&mut o, a.0, a.1, a.2, a.3, a.4);
        }
        o
    }

    #[test]
    fn imports_click_text_and_window() {
        let bytes = file(&[
            (8, 0, [-8, -8, 1936, 1056, -1], "app.exe|*Título*|", ""),
            (3, 50, [1, 1, 100, 200, 0], "", "inicio"),
            (6, 10, [0; 5], "Ab 1", ""),
            (99, 0, [0; 5], "", ""),
        ]);
        let (v, raw) = parse(&bytes).unwrap();
        assert_eq!(v, 4_000_043);
        let (m, warn) = convert("t", &raw);
        assert_eq!(
            m.actions[0],
            Action::FocusWindow {
                exe: "app.exe".into(),
                title: "*Título*".into(),
                rect: Some([-8, -8, 1936, 1056])
            }
        );
        assert_eq!(m.actions[1], Action::Label { name: "inicio".into() });
        assert_eq!(m.actions[2], Action::Click { button: MouseButton::Left, x: 100, y: 200, hold_ms: 50 });
        assert_eq!(m.actions[3], Action::Chars { text: "Ab 1".into(), delay_ms: 10 });
        assert_eq!(warn.len(), 1);
        assert!(warn[0].contains("99"));
    }

    #[test]
    fn truncated_file_is_an_error_not_a_panic() {
        let bytes = file(&[(7, 0, [500, 0, 0, 0, 0], "", "")]);
        for cut in [0, 5, 40, bytes.len() - 1] {
            assert!(parse(&bytes[..cut]).is_err(), "cut={cut}");
        }
    }

    #[test]
    fn rejects_unknown_version() {
        let mut bytes = file(&[]);
        bytes[4..8].copy_from_slice(&5_000_000u32.to_le_bytes());
        assert!(parse(&bytes).is_err());
    }
}
