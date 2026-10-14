//! Núcleo do Korvon Macro: modelo de ações, gravador, player e formato `.kmacro`.

pub mod action;
pub mod macro_file;
pub mod mrf;
pub mod player;
pub mod recorder;

pub use action::{wildcard_match, Action, CmpOp, MouseButton, PressState};
pub use macro_file::{Macro, MacroError, FORMAT_VERSION};
pub use player::{expand_vars, play, resolve_labels, InputSink, PlayOptions, PlayOutcome};
pub use recorder::{Recorder, RecorderOptions};
