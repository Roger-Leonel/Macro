//! Captura global (hooks de baixo nível) e injeção (SendInput) de mouse e teclado no Windows.

#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod sink;

#[cfg(windows)]
pub use hooks::{start_hooks, RawEvent};
#[cfg(windows)]
pub use sink::SendInputSink;

/// Marca (`dwExtraInfo`) nos eventos que o próprio app injeta via `SendInput`.
/// A captura ignora só eventos com esta marca; eventos injetados por terceiros
/// (AnyDesk, TeamViewer, RDP...) continuam sendo gravados.
#[cfg(windows)]
pub(crate) const SELF_MARK: usize = 0x4B4F_5256; // "KORV"
