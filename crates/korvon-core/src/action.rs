use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressState {
    Down,
    Up,
}

/// Operador de comparação das condições.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmpOp {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Greater,
    Less,
    GreaterEq,
    LessEq,
    /// Variável existe e não está vazia.
    Defined,
}

impl CmpOp {
    pub const ALL: [CmpOp; 11] = [
        CmpOp::Equals,
        CmpOp::NotEquals,
        CmpOp::Contains,
        CmpOp::NotContains,
        CmpOp::StartsWith,
        CmpOp::EndsWith,
        CmpOp::Greater,
        CmpOp::Less,
        CmpOp::GreaterEq,
        CmpOp::LessEq,
        CmpOp::Defined,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CmpOp::Equals => "é igual a",
            CmpOp::NotEquals => "é diferente de",
            CmpOp::Contains => "contém",
            CmpOp::NotContains => "não contém",
            CmpOp::StartsWith => "começa com",
            CmpOp::EndsWith => "termina com",
            CmpOp::Greater => "> (número)",
            CmpOp::Less => "< (número)",
            CmpOp::GreaterEq => ">= (número)",
            CmpOp::LessEq => "<= (número)",
            CmpOp::Defined => "está definida",
        }
    }

    /// Avalia `left <op> right`. Operadores numéricos comparam como `f64`
    /// e dão falso se algum lado não for número.
    pub fn eval(self, left: &str, right: &str) -> bool {
        let num = |a: &str, b: &str, f: fn(f64, f64) -> bool| {
            match (a.trim().replace(',', ".").parse::<f64>(), b.trim().replace(',', ".").parse::<f64>()) {
                (Ok(x), Ok(y)) => f(x, y),
                _ => false,
            }
        };
        match self {
            CmpOp::Equals => left == right,
            CmpOp::NotEquals => left != right,
            CmpOp::Contains => left.contains(right),
            CmpOp::NotContains => !left.contains(right),
            CmpOp::StartsWith => left.starts_with(right),
            CmpOp::EndsWith => left.ends_with(right),
            CmpOp::Greater => num(left, right, |x, y| x > y),
            CmpOp::Less => num(left, right, |x, y| x < y),
            CmpOp::GreaterEq => num(left, right, |x, y| x >= y),
            CmpOp::LessEq => num(left, right, |x, y| x <= y),
            CmpOp::Defined => !left.is_empty(),
        }
    }
}

impl std::fmt::Display for CmpOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Uma ação de macro. Coordenadas em pixels da área de trabalho virtual.
///
/// Textos aceitam variáveis no formato `${nome}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move o cursor até `(x, y)` ao longo de `ms` milissegundos (0 = instantâneo),
    /// partindo da posição anterior.
    MouseMove {
        x: i32,
        y: i32,
        #[serde(default)]
        ms: u32,
    },
    /// Clique completo (mover, pressionar, segurar `hold_ms`, soltar).
    Click {
        button: MouseButton,
        x: i32,
        y: i32,
        #[serde(default = "default_hold")]
        hold_ms: u32,
    },
    MouseButton { button: MouseButton, state: PressState },
    /// Tecla pressionada e solta, segurando `hold_ms`.
    KeyPress {
        vk: u16,
        #[serde(default = "default_hold")]
        hold_ms: u32,
    },
    /// Sequência de teclas (letras, dígitos e espaço), uma por vez, `delay_ms` entre elas.
    /// Caracteres sem tecla própria são enviados como texto Unicode.
    Chars {
        text: String,
        #[serde(default = "default_hold")]
        delay_ms: u32,
    },
    Wheel { delta: i32, horizontal: bool },
    /// `vk` é o código de tecla virtual do Windows.
    Key { vk: u16, state: PressState },
    Text { text: String },
    Wait { ms: u64 },

    /// Marca uma posição para `Goto`/`Repeat`/`If`.
    Label { name: String },
    Goto { label: String },
    /// Volta para `label` até o bloco ter rodado `times` vezes no total (0 = infinito).
    Repeat { label: String, times: u32 },
    SetVar { name: String, value: String },
    /// Compara `${var}` com `value`; salta para `then_label` ou `else_label`
    /// (vazio = segue em frente).
    If {
        var: String,
        op: CmpOp,
        value: String,
        then_label: String,
        else_label: String,
    },
    /// Traz para frente uma janela cujo executável e título casem (curingas `*` e `?`);
    /// opcionalmente a posiciona e redimensiona (`rect` = x, y, largura, altura).
    FocusWindow {
        exe: String,
        title: String,
        rect: Option<[i32; 4]>,
    },
}

impl Action {
    /// Nome curto do tipo e os detalhes, separados para a lista de ações poder
    /// destacar o tipo.
    pub fn summary(&self) -> (&'static str, String) {
        match self {
            Action::MouseMove { x, y, ms } => ("mover", format!("({x}, {y})  {ms} ms")),
            Action::Click { button, x, y, .. } => ("clique", format!("{button:?} em ({x}, {y})")),
            Action::KeyPress { vk, .. } => ("tecla", format!("0x{vk:02X} pressionada")),
            Action::Chars { text, .. } => ("caracteres", format!("\"{text}\"")),
            Action::MouseButton { button, state } => {
                ("botão", format!("{button:?} {}", state_label(*state)))
            }
            Action::Wheel { delta, horizontal } => (
                "roda",
                format!("{} {delta}", if *horizontal { "horizontal" } else { "vertical" }),
            ),
            Action::Key { vk, state } => ("tecla", format!("0x{vk:02X} {}", state_label(*state))),
            Action::Text { text } => ("texto", format!("\"{text}\"")),
            Action::Wait { ms } => ("esperar", format!("{ms} ms")),
            Action::Label { name } => ("rótulo", format!("\"{name}\"")),
            Action::Goto { label } => ("ir para", format!("\"{label}\"")),
            Action::Repeat { label, times } => (
                "repetir",
                if *times == 0 {
                    format!("de \"{label}\" infinitamente")
                } else {
                    format!("de \"{label}\" {times}x")
                },
            ),
            Action::SetVar { name, value } => ("variável", format!("${{{name}}} = \"{value}\"")),
            Action::If { var, op, value, then_label, else_label } => (
                "se",
                format!(
                    "${{{var}}} {} \"{value}\" → \"{then_label}\" senão \"{else_label}\"",
                    op.label()
                ),
            ),
            Action::FocusWindow { exe, title, rect } => {
                let r = rect.map(|[x, y, w, h]| format!(" [{x},{y} {w}×{h}]")).unwrap_or_default();
                ("janela", format!("{exe} | {title}{r}"))
            }
        }
    }

    /// Descrição completa em uma linha.
    pub fn describe(&self) -> String {
        let (kind, detail) = self.summary();
        format!("{kind} {detail}")
    }
}

fn state_label(s: PressState) -> &'static str {
    match s {
        PressState::Down => "pressionado",
        PressState::Up => "solto",
    }
}

fn default_hold() -> u32 {
    50
}

/// Casamento com curingas (`*` = qualquer sequência, `?` = um caractere),
/// sem diferenciar maiúsculas de minúsculas.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        assert!(wildcard_match("*Comet*", "Algo - Comet"));
        assert!(wildcard_match("comet.exe", "COMET.EXE"));
        assert!(wildcard_match("a?c", "abc"));
        assert!(!wildcard_match("a?c", "abbc"));
        assert!(wildcard_match("*", ""));
    }

    #[test]
    fn compare() {
        assert!(CmpOp::Greater.eval("10", "9"));
        assert!(!CmpOp::Greater.eval("abc", "9"));
        assert!(CmpOp::Contains.eval("banana", "nan"));
        assert!(CmpOp::LessEq.eval("1,5", "1.5"));
        assert!(CmpOp::Defined.eval("x", ""));
    }
}
