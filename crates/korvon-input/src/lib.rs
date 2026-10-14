//! Captura global (hooks de baixo nível) e injeção (SendInput) de mouse e teclado no Windows.

#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod sink;

#[cfg(windows)]
pub use hooks::{start_hooks, RawEvent};
#[cfg(windows)]
pub use sink::SendInputSink;
