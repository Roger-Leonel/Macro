#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icons;
mod library;
mod rows;
mod schedule;
mod theme;
mod tray;

use icons::{Icon, Icons};
use iced::widget::{
    button, column, container, horizontal_space, mouse_area, pick_list, row, scrollable, stack, text, text_input,
    Column, Space,
};
use schedule::Freq;
use iced::{keyboard, time, window, Alignment, Element, Font, Length, Padding, Subscription, Task};
use korvon_core::{play, Action, Macro, PlayOptions, PlayOutcome, PressState, Recorder, RecorderOptions};
use korvon_input::{start_hooks, RawEvent, SendInputSink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

const VK_ESC: u16 = 0x1B; // para a gravação
const VK_F9: u16 = 0x78; // gravar / parar gravação
const VK_F10: u16 = 0x79; // reproduzir
const VK_F11: u16 = 0x7A; // parar
/// Máximo de linhas desenhadas; com mais, mostra as últimas.
const MAX_ROWS: usize = 3000;

/// Modo sem janela usado pelo Agendador de Tarefas:
/// `korvon-macro --run <arquivo.kmacro> [--speed X] [--repeat N]`. F11 interrompe.
/// Devolve o código de saída, ou `None` se não for uma execução agendada.
fn run_headless() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--run") {
        return None;
    }
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
    let Some(path) = args.get(1) else { return Some(2) };
    let speed = value("--speed").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
    let repeat = value("--repeat").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let Ok(m) = Macro::load(std::path::Path::new(path)) else { return Some(2) };
    if korvon_core::resolve_labels(&m).is_err() {
        return Some(2);
    }

    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(rx) = start_hooks() {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            for ev in rx {
                if matches!(ev.action, Action::Key { vk: VK_F11, state: PressState::Down }) {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        });
    }
    let mut sink = SendInputSink;
    let outcome = play(&m, PlayOptions { speed, repeat }, &mut sink, &cancel, |_, _| {});
    Some(if matches!(outcome, PlayOutcome::Failed(_)) { 1 } else { 0 })
}

fn main() -> iced::Result {
    if let Some(code) = run_headless() {
        std::process::exit(code);
    }
    iced::application("Korvon Macro", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| theme::theme())
        .default_font(Font::with_name("Segoe UI"))
        .window_size((1100.0, 700.0))
        .run_with(|| (App::new(), Task::none()))
}

/// Abas da faixa de opções (a aba "Arquivo" é um menu, tratado à parte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Edit,
    Schedules,
    View,
    Help,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Edit, Tab::Schedules, Tab::View, Tab::Help];

    fn title(self) -> &'static str {
        match self {
            Tab::Edit => "Gravar e Editar",
            Tab::Schedules => "Agendamentos",
            Tab::View => "Exibir",
            Tab::Help => "Ajuda",
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SetTab(Tab),
    ToggleMenu,
    CloseMenu,
    ToggleNumbers,
    Tick,
    New,
    Open,
    Import,
    Save,
    SaveAs,
    Quit,
    Record,
    Play,
    SpeedChanged(String),
    RepeatChanged(String),
    Select(usize),
    Delete,
    MoveUp,
    MoveDown,
    SchedFreq(Freq),
    SchedTimeText(String),
    SchedDateText(String),
    SchedDay(usize),
    SchedDelay(String),
    SchedPick,
    SchedCreate,
    /// Linha da lista: macro (`None`) ou agendamento existente (`Some(tarefa)`).
    SchedEdit(std::path::PathBuf, Option<String>),
    /// Linha pendente (macro aberta, ainda sem agendamento criado).
    SchedDraft(u32),
    SchedDuplicate,
    SchedToggle,
    SchedRunNow,
    SchedDelete,
}

type SchedResult = Result<Vec<schedule::Entry>, String>;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Mode {
    Idle,
    Recording,
    Playing,
}

enum PlayMsg {
    Progress(usize, usize),
    Done(PlayOutcome),
}

struct App {
    hooks: Option<Receiver<RawEvent>>,
    mode: Mode,
    recorder: Option<Recorder>,
    current: Macro,
    path: Option<std::path::PathBuf>,
    speed: String,
    repeat: String,
    status: String,
    play_rx: Option<Receiver<PlayMsg>>,
    cancel: Arc<AtomicBool>,
    /// Linha selecionada na tabela (índice da linha agrupada, não da ação).
    selected: Option<usize>,
    playing_at: Option<usize>,
    list_id: scrollable::Id,
    tab: Tab,
    menu_open: bool,
    show_numbers: bool,
    icons: Icons,
    /// Linhas agrupadas da tabela; refeitas só quando as ações mudam.
    rows: Vec<rows::Row>,
    rows_dirty: bool,
    /// Macros da biblioteca (lista da aba Agendamentos).
    library: Vec<library::Item>,
    /// Tarefas existentes no Agendador.
    schedules: Vec<schedule::Entry>,
    /// Tarefa em edição; `None` = novo agendamento da macro escolhida.
    editing: Option<String>,
    /// Macro escolhida para agendar (pelo Explorer ou pela lista); o formulário só aparece com ela.
    pending: Option<std::path::PathBuf>,
    sched_freq: Freq,
    /// Texto com máscara `HH:MM`.
    sched_time: String,
    /// Texto com máscara `dd/mm/aaaa`.
    sched_date: String,
    sched_days: [bool; 7],
    /// Atraso em segundos após o logon (só dígitos).
    sched_delay: String,
    sched_rx: Option<Receiver<SchedResult>>,
    /// Linhas pendentes: macros abertas aguardando o agendamento ser finalizado.
    drafts: Vec<Draft>,
    /// Pendente em edição no formulário.
    editing_draft: Option<u32>,
    next_draft: u32,
    tray: Option<tray_icon::TrayIcon>,
}

/// Macro aberta na aba Agendamentos com a configuração própria dela.
struct Draft {
    id: u32,
    path: std::path::PathBuf,
    freq: Freq,
    time: String,
    date: String,
    days: [bool; 7],
    delay: String,
}

fn shortcut(key: keyboard::Key, mods: keyboard::Modifiers) -> Option<Message> {
    if !mods.command() {
        return None;
    }
    match key.as_ref() {
        keyboard::Key::Character("n") => Some(Message::New),
        keyboard::Key::Character("o") => Some(Message::Open),
        keyboard::Key::Character("s") => Some(if mods.shift() { Message::SaveAs } else { Message::Save }),
        _ => None,
    }
}

impl App {
    fn new() -> Self {
        let (hooks, status) = match start_hooks() {
            Ok(rx) => (Some(rx), "Pronto. F9 grava (ESC para a gravação), F10 reproduz, F11 para.".to_string()),
            Err(e) => (None, e),
        };
        let mut app = Self {
            hooks,
            mode: Mode::Idle,
            recorder: None,
            current: Macro::new("Nova macro", vec![]),
            path: None,
            speed: "1.0".into(),
            repeat: "1".into(),
            status,
            play_rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            selected: None,
            playing_at: None,
            list_id: scrollable::Id::new("acoes"),
            tab: Tab::Edit,
            menu_open: false,
            show_numbers: false,
            icons: Icons::new(),
            rows: Vec::new(),
            rows_dirty: false,
            library: Vec::new(),
            schedules: Vec::new(),
            editing: None,
            pending: None,
            sched_freq: Freq::Daily,
            sched_time: "08:00".into(),
            sched_date: String::new(),
            sched_days: [true, true, true, true, true, false, false],
            sched_delay: "0".into(),
            sched_rx: None,
            drafts: Vec::new(),
            editing_draft: None,
            next_draft: 1,
            tray: None,
        };
        // `korvon-macro <arquivo.kmacro|arquivo.mrf>` abre o arquivo ao iniciar.
        if let Some(arg) = std::env::args_os().nth(1) {
            app.open_path(std::path::PathBuf::from(arg));
        }
        app.rows = rows::build(&app.current.actions);
        app
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Parado: só precisa ver as teclas de atalho; gravando/reproduzindo: mais fluido.
            time::every(Duration::from_millis(if self.mode == Mode::Idle { 80 } else { 30 })).map(|_| Message::Tick),
            keyboard::on_key_press(shortcut),
        ])
    }

    fn editable(&self) -> bool {
        self.mode == Mode::Idle
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Qualquer ação do usuário fecha o menu Arquivo (exceto abrir/fechar o próprio menu).
        if !matches!(message, Message::Tick | Message::ToggleMenu) {
            self.menu_open = false;
        }
        let is_tick = matches!(message, Message::Tick);
        let mut task = Task::none();
        match message {
            Message::SetTab(t) => {
                self.tab = t;
                if t == Tab::Schedules {
                    self.refresh_schedules();
                }
            }
            Message::SchedFreq(f) => self.sched_freq = f,
            // A máscara reescreve o texto (insere ':' / '/'); o cursor volta ao fim para não ficar para trás.
            Message::SchedTimeText(t) => {
                self.sched_time = mask_time(&t);
                task = text_input::move_cursor_to_end(text_input::Id::new("sched-time"));
            }
            Message::SchedDateText(t) => {
                self.sched_date = mask_date(&t);
                task = text_input::move_cursor_to_end(text_input::Id::new("sched-date"));
            }
            Message::SchedDelay(t) => {
                self.sched_delay = t.chars().filter(char::is_ascii_digit).take(5).collect();
            }
            Message::SchedDay(i) => self.sched_days[i] = !self.sched_days[i],
            Message::SchedPick => self.sched_pick(),
            Message::SchedCreate => self.sched_create(),
            Message::SchedEdit(path, task) => self.sched_open(path, task),
            Message::SchedDraft(id) => self.sched_open_draft(id),
            Message::SchedDuplicate => self.sched_duplicate(),
            Message::SchedToggle => self.sched_toggle(),
            Message::SchedRunNow => self.sched_run_now(),
            Message::SchedDelete => self.sched_delete(),
            Message::ToggleMenu => self.menu_open = !self.menu_open,
            Message::CloseMenu => {}
            Message::ToggleNumbers => self.show_numbers = !self.show_numbers,
            Message::Tick => task = self.tick(),
            Message::New => {
                if self.editable() {
                    self.current = Macro::new("Nova macro", vec![]);
                    self.path = None;
                    self.selected = None;
                    self.status = "Nova macro.".into();
                }
            }
            Message::Open => self.open(),
            Message::Import => self.import(),
            Message::Save => self.save(false),
            Message::SaveAs => self.save(true),
            Message::Quit => task = iced::exit(),
            Message::Record => task = self.toggle_record(),
            Message::Play => self.start_play(),
            Message::SpeedChanged(s) => self.speed = s,
            Message::RepeatChanged(s) => self.repeat = s,
            Message::Select(i) => self.selected = Some(i),
            Message::Delete => self.delete_selected(),
            Message::MoveUp => self.move_selected(-1),
            Message::MoveDown => self.move_selected(1),
        }
        if self.rows_dirty || !is_tick {
            self.rows = rows::build(&self.current.actions);
            self.rows_dirty = false;
        }
        task
    }

    // ------------------------------------------------------------ agendamentos

    /// Relê a biblioteca e as tarefas do Agendador (o PowerShell leva um instante, então vai numa thread).
    fn refresh_schedules(&mut self) {
        self.library = library::list();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(schedule::list());
        });
        self.sched_rx = Some(rx);
    }

    /// Macro em edição no formulário de agendamento.
    fn selected_macro(&self) -> Option<library::Item> {
        let path = self.pending.clone()?;
        Some(library::Item { name: path.file_stem()?.to_str()?.to_string(), path })
    }

    /// Agendamentos (tarefas) da macro `name`.
    fn entries_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a schedule::Entry> + 'a {
        let name = schedule::clean_name(name);
        self.schedules.iter().filter(move |e| schedule::clean_name(&e.macro_name()) == name)
    }

    fn entry_by_task(&self, task: &str) -> Option<&schedule::Entry> {
        self.schedules.iter().find(|e| e.name == task)
    }

    /// Agendamento em edição, se for um já existente.
    fn editing_entry(&self) -> Option<&schedule::Entry> {
        self.editing.as_deref().and_then(|t| self.entry_by_task(t))
    }

    /// Botão Abrir Macro: escolhe a macro e já a coloca na lista como pendente.
    fn sched_pick(&mut self) {
        if let Some(p) = dialog_in_library().add_filter("Korvon Macro", &["kmacro"]).pick_file() {
            self.stash_draft();
            self.add_draft(p, false);
        }
    }

    /// Guarda no rascunho em edição o que está no formulário.
    fn stash_draft(&mut self) {
        let Some(id) = self.editing_draft else { return };
        let (freq, time, date, days, delay) =
            (self.sched_freq, self.sched_time.clone(), self.sched_date.clone(), self.sched_days, self.sched_delay.clone());
        if let Some(d) = self.drafts.iter_mut().find(|d| d.id == id) {
            (d.freq, d.time, d.date, d.days, d.delay) = (freq, time, date, days, delay);
        }
    }

    /// Cria uma linha pendente (em branco ou com a configuração atual do formulário) e a seleciona.
    fn add_draft(&mut self, path: std::path::PathBuf, copy_form: bool) {
        let id = self.next_draft;
        self.next_draft += 1;
        let d = if copy_form {
            Draft {
                id,
                path,
                freq: self.sched_freq,
                time: self.sched_time.clone(),
                date: self.sched_date.clone(),
                days: self.sched_days,
                delay: self.sched_delay.clone(),
            }
        } else {
            Draft {
                id,
                path,
                freq: Freq::Daily,
                time: "08:00".into(),
                date: String::new(),
                days: [true, true, true, true, true, false, false],
                delay: "0".into(),
            }
        };
        self.drafts.push(d);
        self.sched_open_draft(id);
    }

    fn sched_open_draft(&mut self, id: u32) {
        if self.editing_draft != Some(id) {
            self.stash_draft();
        }
        let Some(d) = self.drafts.iter().find(|d| d.id == id) else { return };
        let (path, freq, time, date, days, delay) = (d.path.clone(), d.freq, d.time.clone(), d.date.clone(), d.days, d.delay.clone());
        self.pending = Some(path);
        self.editing = None;
        self.editing_draft = Some(id);
        (self.sched_freq, self.sched_time, self.sched_date, self.sched_days, self.sched_delay) = (freq, time, date, days, delay);
        if let Some(item) = self.selected_macro() {
            self.status = format!("Macro \"{}\": defina quando executar e clique em Finalizar agendamento.", item.name);
        }
    }

    /// Duplicar: nova linha pendente da mesma macro, começando com a configuração atual.
    fn sched_duplicate(&mut self) {
        let Some(path) = self.pending.clone() else { return };
        self.stash_draft();
        self.add_draft(path, true);
    }

    /// Escolhe a macro; com `task`, carrega no formulário aquele agendamento.
    fn sched_open(&mut self, path: std::path::PathBuf, task: Option<String>) {
        self.stash_draft();
        self.editing_draft = None;
        self.pending = Some(path);
        self.editing = task;
        let Some(item) = self.selected_macro() else { return };
        if let Some(info) = self.editing_entry().and_then(|e| e.info()) {
            self.sched_freq = info.freq;
            self.sched_time = format!("{:02}:{:02}", info.time.0, info.time.1);
            self.sched_date = if info.freq == Freq::Once {
                format!("{:02}/{:02}/{:04}", info.date.2, info.date.1, info.date.0)
            } else {
                String::new()
            };
            self.sched_days = info.days;
            self.sched_delay = info.delay.to_string();
        }
        self.status = format!("Macro \"{}\": defina quando executar e clique em Finalizar agendamento.", item.name);
    }

    /// Copia a macro para a biblioteca se ela veio de fora, devolvendo o caminho final.
    fn into_library(item: &library::Item) -> Result<std::path::PathBuf, String> {
        let dir = library::dir().ok_or("Pasta da biblioteca indisponível.")?;
        let dest = dir.join(format!("{}.kmacro", item.name));
        let same = |a: &std::path::Path, b: &std::path::Path| {
            matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
        };
        if same(&item.path, &dest) {
            return Ok(dest);
        }
        if dest.exists() {
            return Err(format!(
                "Já existe uma macro chamada \"{}\" na biblioteca. Renomeie o arquivo e tente de novo.",
                item.name
            ));
        }
        std::fs::copy(&item.path, &dest).map_err(|e| format!("Erro ao copiar a macro para a biblioteca: {e}"))?;
        Ok(dest)
    }

    fn sched_create(&mut self) {
        let Some(item) = self.selected_macro() else { return };
        let time = if self.sched_freq == Freq::Logon { Ok((0, 0)) } else { parse_time(&self.sched_time) };
        let date = if self.sched_freq == Freq::Once { parse_date(&self.sched_date) } else { Ok((2000, 1, 1)) };
        let (time, date) = match (time, date) {
            (Ok(t), Ok(d)) => (t, d),
            (Err(e), _) | (_, Err(e)) => {
                self.status = e;
                return;
            }
        };
        let file = match Self::into_library(&item) {
            Ok(f) => f,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let task = self.editing.clone();
        let enabled = self.editing_entry().is_none_or(|e| e.enabled());
        let req = schedule::Request {
            name: &item.name,
            task: task.as_deref(),
            enabled,
            file: &file,
            freq: self.sched_freq,
            time,
            date,
            days: self.sched_days,
            delay: self.sched_delay.parse().unwrap_or(0),
            speed: self.speed.replace(',', ".").trim().parse::<f64>().unwrap_or(1.0),
            repeat: self.repeat.trim().parse::<u32>().unwrap_or(1),
        };
        match schedule::create(&req) {
            Ok(_) => {
                self.status = format!("\"{}\" agendada no Agendador de Tarefas do Windows.", item.name);
                self.pending = None;
                self.editing = None;
                let done = self.editing_draft.take();
                self.drafts.retain(|d| Some(d.id) != done);
                self.refresh_schedules();
            }
            Err(e) => self.status = e,
        }
    }

    fn sched_run_now(&mut self) {
        let Some(task) = self.editing.clone() else { return };
        let name = self.selected_macro().map(|m| m.name).unwrap_or_default();
        self.status = match schedule::run_now(&task) {
            Ok(()) => format!("\"{name}\" iniciada."),
            Err(e) => format!("Falha ao executar \"{name}\": {e}"),
        };
    }

    fn sched_toggle(&mut self) {
        let Some(task) = self.editing.clone() else { return };
        let enable = self.editing_entry().is_some_and(|e| !e.enabled());
        let name = self.selected_macro().map(|m| m.name).unwrap_or_default();
        self.status = match schedule::set_enabled(&task, enable) {
            Ok(()) if enable => format!("Agendamento de \"{name}\" habilitado."),
            Ok(()) => format!("Agendamento de \"{name}\" desabilitado."),
            Err(e) => format!("Falha ao alterar o agendamento de \"{name}\": {e}"),
        };
        self.refresh_schedules();
    }

    fn sched_delete(&mut self) {
        if let Some(id) = self.editing_draft.take() {
            self.drafts.retain(|d| d.id != id);
            self.pending = None;
            self.status = "Linha pendente removida.".into();
            return;
        }
        let Some(task) = self.editing.clone() else { return };
        let name = self.selected_macro().map(|m| m.name).unwrap_or_default();
        self.status = match schedule::delete(&task) {
            Ok(()) => {
                self.pending = None;
                self.editing = None;
                format!("Agendamento de \"{name}\" removido.")
            }
            Err(e) => format!("Falha ao remover o agendamento de \"{name}\": {e}"),
        };
        self.refresh_schedules();
    }

    fn delete_selected(&mut self) {
        if !self.editable() {
            return;
        }
        let rows = rows::build(&self.current.actions);
        if let Some(r) = self.selected.and_then(|i| rows.get(i)) {
            rows::remove_row(&mut self.current.actions, r);
            let left = rows.len() - 1;
            self.selected = if left == 0 { None } else { self.selected.map(|i| i.min(left - 1)) };
        }
    }

    fn move_selected(&mut self, delta: isize) {
        if !self.editable() {
            return;
        }
        let rows = rows::build(&self.current.actions);
        let Some(i) = self.selected else { return };
        let j = i as isize + delta;
        if j < 0 || j as usize >= rows.len() || i >= rows.len() {
            return;
        }
        let (a, b) = if delta < 0 { (&rows[j as usize], &rows[i]) } else { (&rows[i], &rows[j as usize]) };
        rows::swap_adjacent(&mut self.current.actions, a, b);
        self.selected = Some(j as usize);
    }

    fn tick(&mut self) -> Task<Message> {
        let mut scroll = false;
        let mut tasks: Vec<Task<Message>> = vec![];
        // Eventos globais (hotkeys + gravação).
        let events: Vec<RawEvent> = match &self.hooks {
            Some(rx) => rx.try_iter().collect(),
            None => vec![],
        };
        for ev in events {
            // ESC encerra a gravação (e não entra nela).
            if self.mode == Mode::Recording && matches!(ev.action, Action::Key { vk: VK_ESC, .. }) {
                if matches!(ev.action, Action::Key { state: PressState::Down, .. }) {
                    tasks.push(self.stop());
                }
                continue;
            }
            let hotkey = match ev.action {
                Action::Key { vk, state } if matches!(vk, VK_F9 | VK_F10 | VK_F11) => Some((vk, state)),
                _ => None,
            };
            if let Some((vk, state)) = hotkey {
                if state == PressState::Down {
                    match vk {
                        VK_F9 => tasks.push(self.toggle_record()),
                        VK_F10 => self.start_play(),
                        _ => tasks.push(self.stop()),
                    }
                }
                continue; // hotkeys nunca entram na gravação
            }
            if self.mode == Mode::Recording {
                if let Some(r) = self.recorder.as_mut() {
                    r.push(ev.t_ms, ev.action, ev.pos);
                }
            }
        }

        // Gravação em tempo real: a lista acompanha o que o gravador já produziu.
        if self.mode == Mode::Recording {
            if let Some(r) = self.recorder.as_mut() {
                let (from, changed) = r.drain_changes();
                if !changed.is_empty() {
                    self.current.actions.truncate(from);
                    self.current.actions.extend(changed);
                    scroll = true;
                    self.rows_dirty = true;
                }
            }
        }

        // Progresso da reprodução.
        let msgs: Vec<PlayMsg> = match &self.play_rx {
            Some(rx) => rx.try_iter().collect(),
            None => vec![],
        };
        for m in msgs {
            match m {
                PlayMsg::Progress(i, n) => {
                    self.playing_at = Some(i);
                    self.status = format!("Reproduzindo {} de {n}…", i + 1);
                }
                PlayMsg::Done(outcome) => {
                    self.rows_dirty = true;
                    self.mode = Mode::Idle;
                    self.play_rx = None;
                    self.playing_at = None;
                    self.status = match outcome {
                        PlayOutcome::Finished => "Reprodução concluída.".into(),
                        PlayOutcome::Cancelled => "Reprodução interrompida.".into(),
                        PlayOutcome::Failed(e) => format!("Falha na reprodução: {e}"),
                    };
                }
            }
        }
        if let Some(Ok(result)) = self.sched_rx.as_ref().map(|rx| rx.try_recv()) {
            self.sched_rx = None;
            match result {
                Ok(list) => self.schedules = list,
                Err(e) => self.status = format!("Erro ao ler agendamentos: {e}"),
            }
        }
        if self.mode == Mode::Recording {
            self.status = format!("Gravando… {} ações", self.current.actions.len());
        }
        if scroll {
            tasks.push(scrollable::snap_to(self.list_id.clone(), scrollable::RelativeOffset::END));
        }
        Task::batch(tasks)
    }

    fn toggle_record(&mut self) -> Task<Message> {
        match self.mode {
            Mode::Idle => {
                if self.hooks.is_none() {
                    return Task::none();
                }
                self.current.actions.clear();
                self.selected = None;
                self.recorder = Some(Recorder::new(RecorderOptions::default()));
                self.mode = Mode::Recording;
                // Durante a gravação o app some da tela e fica só na área de notificação.
                self.tray = tray::show();
                window::get_oldest().and_then(|id| window::change_mode(id, window::Mode::Hidden))
            }
            Mode::Recording => self.stop(),
            Mode::Playing => Task::none(),
        }
    }

    fn stop(&mut self) -> Task<Message> {
        match self.mode {
            Mode::Recording => {
                if let Some(r) = self.recorder.take() {
                    let actions = r.finish();
                    self.status = format!("Gravação concluída: {} ações.", actions.len());
                    self.current.actions = actions;
                    self.selected = None;
                }
                self.mode = Mode::Idle;
                self.tray = None;
                return window::get_oldest().and_then(|id| {
                    window::change_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                });
            }
            Mode::Playing => self.cancel.store(true, Ordering::Relaxed),
            Mode::Idle => {}
        }
        Task::none()
    }

    fn start_play(&mut self) {
        if self.mode != Mode::Idle || self.current.actions.is_empty() {
            return;
        }
        if let Err(e) = korvon_core::resolve_labels(&self.current) {
            self.status = format!("Macro inválida: {e}");
            return;
        }
        let speed = self.speed.replace(',', ".").trim().parse::<f64>().unwrap_or(1.0);
        let repeat = self.repeat.trim().parse::<u32>().unwrap_or(1);
        let m = self.current.clone();
        let (tx, rx) = channel();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let mut sink = SendInputSink;
            let outcome = play(&m, PlayOptions { speed, repeat }, &mut sink, &cancel, |i, n| {
                let _ = tx.send(PlayMsg::Progress(i, n));
            });
            let _ = tx.send(PlayMsg::Done(outcome));
        });
        self.play_rx = Some(rx);
        self.mode = Mode::Playing;
    }

    fn open_path(&mut self, p: std::path::PathBuf) {
        let is_mrf = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mrf"));
        if is_mrf {
            match korvon_core::mrf::import_file(&p) {
                Ok((m, warnings)) => {
                    self.status = if warnings.is_empty() {
                        format!("Importado: {} ações.", m.actions.len())
                    } else {
                        format!("Importado: {} ações. Avisos: {}", m.actions.len(), warnings.join("; "))
                    };
                    self.current = m;
                    self.path = None; // força "Salvar como" em .kmacro
                    self.selected = None;
                }
                Err(e) => self.status = format!("Erro ao importar: {e}"),
            }
        } else {
            match Macro::load(&p) {
                Ok(m) => {
                    self.status = format!("Aberto: {} ({} ações).", p.display(), m.actions.len());
                    self.current = m;
                    self.path = Some(p);
                    self.selected = None;
                }
                Err(e) => self.status = format!("Erro ao abrir: {e}"),
            }
        }
    }

    fn open(&mut self) {
        if !self.editable() {
            return;
        }
        if let Some(p) = dialog_in_library().add_filter("Korvon Macro", &["kmacro"]).pick_file() {
            self.open_path(p);
        }
    }

    fn import(&mut self) {
        if !self.editable() {
            return;
        }
        if let Some(p) = rfd::FileDialog::new().add_filter("Macro Recorder", &["mrf"]).pick_file() {
            self.open_path(p);
        }
    }

    fn save(&mut self, force_dialog: bool) {
        if !self.editable() {
            return;
        }
        let path = match (&self.path, force_dialog) {
            (Some(p), false) => p.clone(),
            _ => {
                let dialog = dialog_in_library()
                    .add_filter("Korvon Macro", &["kmacro"])
                    .set_file_name(format!("{}.kmacro", self.current.name));
                match dialog.save_file() {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            self.current.name = stem.to_string();
        }
        match self.current.save(&path) {
            Ok(()) => {
                self.status = format!("Salvo em {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.status = format!("Erro ao salvar: {e}"),
        }
    }

    // ------------------------------------------------------------------ view

    /// Lista das macros da biblioteca: uma linha por agendamento (ou uma linha sem agendamento).
    fn schedules_table(&self) -> iced::widget::Container<'_, Message> {
        use theme::*;
        let head = |s: &'static str, w: u16| text(s).size(12).color(MUTED).width(Length::FillPortion(w));
        let table_head = container(
            row![head("Macro", 4), head("Frequência", 4), head("Hora", 2), head("Data", 2), head("Estado", 2)]
                .spacing(10)
                .padding([0, 8]),
        )
        .padding([6, 0])
        .width(Length::Fill)
        .style(surface(HEADER, 0.0, Some(BORDER)));

        // (nome, caminho, agendamento)
        let mut entries: Vec<(String, std::path::PathBuf, Option<&schedule::Entry>)> = vec![];
        for m in &self.library {
            let mine: Vec<_> = self.entries_of(&m.name).collect();
            if mine.is_empty() && !self.drafts.iter().any(|d| d.path == m.path) {
                entries.push((m.name.clone(), m.path.clone(), None));
            }
            entries.extend(mine.into_iter().map(|e| (m.name.clone(), m.path.clone(), Some(e))));
        }
        // Agendamentos de macros que não estão (mais) na biblioteca.
        let dir = library::dir().unwrap_or_default();
        for e in &self.schedules {
            let name = e.macro_name();
            if !self.library.iter().any(|m| schedule::clean_name(&m.name) == schedule::clean_name(&name)) {
                entries.push((name.clone(), dir.join(format!("{name}.kmacro")), Some(e)));
            }
        }

        let badge = |label: &'static str, color| -> Element<Message> {
            container(text(label).size(12).color(WHITE)).padding([2, 10]).style(surface(color, 10.0, None)).into()
        };
        let dash = || "—".to_string();
        let mut lines: Vec<Element<Message>> = vec![];
        for d in &self.drafts {
            let name = d.path.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            let live = self.editing_draft == Some(d.id);
            let (freq, time, date) = if live {
                (self.sched_freq, self.sched_time.clone(), self.sched_date.clone())
            } else {
                (d.freq, d.time.clone(), d.date.clone())
            };
            let cell = |s: String, w: u16| text(s).size(13).width(Length::FillPortion(w));
            lines.push(
                button(
                    row![
                        cell(name, 4),
                        cell(freq.to_string(), 4),
                        cell(if freq == Freq::Logon { dash() } else { time }, 2),
                        cell(if freq == Freq::Once { date } else { dash() }, 2),
                        container(badge("Pendente", rgb_hex(0xe08a00))).width(Length::FillPortion(2)),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([6, 8])
                .width(Length::Fill)
                .style(row_style(live, false))
                .on_press(Message::SchedDraft(d.id))
                .into(),
            );
        }
        lines.extend(entries
            .into_iter()
            .map(|(name, path, entry)| {
                let cell = |s: String, w: u16| text(s).size(13).width(Length::FillPortion(w));
                let info = entry.and_then(|e| e.info());
                let state: Element<Message> = match entry {
                    Some(e) if e.enabled() => badge("Agendada", PLAY_GREEN),
                    Some(_) => badge("Desabilitada", MUTED),
                    None => Space::new(0, 0).into(),
                };
                let task = entry.map(|e| e.name.clone());
                let selected = self.editing_draft.is_none() && self.pending.as_ref() == Some(&path) && self.editing == task;
                button(
                    row![
                        cell(name, 4),
                        cell(info.as_ref().map_or_else(dash, |i| i.freq_label()), 4),
                        cell(info.as_ref().map_or_else(dash, |i| i.time_label()), 2),
                        cell(info.as_ref().map_or_else(dash, |i| i.date_label()), 2),
                        container(state).width(Length::FillPortion(2)),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([6, 8])
                .width(Length::Fill)
                .style(row_style(selected, false))
                .on_press(Message::SchedEdit(path, task))
                .into()
            }));
        if lines.is_empty() {
            lines.push(
                container(
                    text("Nenhuma macro na biblioteca. Grave uma macro e salve-a (Ctrl+S): ela aparecerá aqui para ser agendada.")
                        .size(14)
                        .color(MUTED),
                )
                .padding(16)
                .into(),
            );
        }
        container(column![
            table_head,
            container(scrollable(Column::with_children(lines).spacing(1).width(Length::Fill)))
                .height(Length::Fill)
                .style(surface(rgb_hex(0xe6e6e6), 0.0, None)),
        ])
        .height(Length::Fill)
        .width(Length::Fill)
        .style(surface(WHITE, 0.0, Some(BORDER)))
    }

    fn view(&self) -> Element<'_, Message> {
        use theme::*;
        let idle = self.editable();
        let recording = self.mode == Mode::Recording;
        let rows = &self.rows;
        let has_sel = idle && self.selected.is_some_and(|i| i < rows.len());

        // --- Abas ---------------------------------------------------------
        let mut tabs = row![button(text("Arquivo").size(13))
            .padding([6, 18])
            .style(tab_style(self.menu_open, true))
            .on_press(Message::ToggleMenu)]
        .spacing(2)
        .padding(Padding { top: 6.0, right: 8.0, bottom: 0.0, left: 8.0 });
        for t in Tab::ALL {
            tabs = tabs.push(
                button(text(t.title()).size(13))
                    .padding([6, 16])
                    .style(tab_style(self.tab == t, false))
                    .on_press(Message::SetTab(t)),
            );
        }
        let tab_bar = container(tabs).width(Length::Fill).style(surface(WINDOW, 0.0, None));

        // --- Faixa de opções -------------------------------------------------
        let group = |title: &'static str, content: Element<'static, Message>| -> Element<'static, Message> {
            let sep = container(Space::new(1, 70)).style(surface(BORDER, 0.0, None));
            row![
                column![content, text(title).size(11).color(MUTED)].spacing(4).align_x(Alignment::Center),
                sep,
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        };
        let ic = &self.icons;
        let ribbon_row: Element<Message> = match self.tab {
            Tab::Edit => row![
                group(
                    "Reprodução e gravação",
                    row![
                        rbtn(ic, Icon::Play, "Reproduzir (F10)", Message::Play, idle && !self.current.actions.is_empty()),
                        if recording {
                            rbtn(ic, Icon::Record, "Parar gravação (ESC)", Message::Record, true)
                        } else {
                            rbtn(ic, Icon::Record, "Gravar (F9)", Message::Record, idle && self.hooks.is_some())
                        },
                    ]
                    .spacing(4)
                    .into()
                ),
                group(
                    "Propriedades de reprodução",
                    row![
                        field(
                            "Velocidade",
                            text_input("1.0", &self.speed)
                                .on_input(Message::SpeedChanged)
                                .style(input)
                                .padding(6)
                                .width(80)
                                .into()
                        ),
                        field(
                            "Repetir (0 = infinito)",
                            text_input("1", &self.repeat)
                                .on_input(Message::RepeatChanged)
                                .style(input)
                                .padding(6)
                                .width(150)
                                .into()
                        ),
                    ]
                    .spacing(14)
                    .into()
                ),
                group(
                    "Editar",
                    row![
                        rbtn(ic, Icon::Delete, "Excluir", Message::Delete, has_sel),
                        rbtn(ic, Icon::Up, "Subir", Message::MoveUp, has_sel && self.selected != Some(0)),
                        rbtn(
                            ic,
                            Icon::Down,
                            "Descer",
                            Message::MoveDown,
                            has_sel && self.selected.is_some_and(|i| i + 1 < rows.len())
                        ),
                    ]
                    .spacing(4)
                    .into()
                ),
            ]
            .spacing(12)
            .into(),
            Tab::Schedules => {
                let sel = self.selected_macro();
                let current = self.editing_entry();
                let scheduled = current.is_some();
                let removable = scheduled || self.editing_draft.is_some();
                let is_enabled = current.is_none_or(|e| e.enabled());
                let pick = rbtn_w(ic, Icon::Explorer, "Abrir Macro", Message::SchedPick, true, 90);
                let form: Element<Message> = if sel.is_none() {
                    row![pick, text("Clique em Abrir Macro para escolher uma macro.").size(13).color(MUTED)]
                        .spacing(12)
                        .align_y(Alignment::Center)
                        .into()
                } else {
                    let mut form = row![field(
                        "Frequência",
                        pick_list(Freq::ALL, Some(self.sched_freq), Message::SchedFreq).padding(6).width(170).into()
                    )]
                    .spacing(10);
                    if self.sched_freq != Freq::Logon {
                        form = form.push(field(
                            "Hora",
                            text_input("HH:MM", &self.sched_time)
                                .id(text_input::Id::new("sched-time"))
                                .on_input(Message::SchedTimeText)
                                .style(input)
                                .padding(6)
                                .width(70)
                                .into(),
                        ));
                    }
                    match self.sched_freq {
                        Freq::Once => {
                            form = form.push(field(
                                "Data",
                                text_input("dd/mm/aaaa", &self.sched_date)
                                    .id(text_input::Id::new("sched-date"))
                                    .on_input(Message::SchedDateText)
                                    .style(input)
                                    .padding(6)
                                    .width(100)
                                    .into(),
                            ))
                        }
                        Freq::Logon => {
                            form = form.push(field(
                                "Atraso após entrar (s)",
                                text_input("0", &self.sched_delay)
                                    .on_input(Message::SchedDelay)
                                    .style(input)
                                    .padding(6)
                                    .width(150)
                                    .into(),
                            ))
                        }
                        Freq::Weekly => {
                            let days = schedule::DAY_LABELS.iter().enumerate().fold(row![].spacing(3), |r, (i, d)| {
                                r.push(
                                    button(text(*d).size(12).align_x(Alignment::Center).width(28))
                                        .padding([6, 3])
                                        .style(day_button(self.sched_days[i]))
                                        .on_press(Message::SchedDay(i)),
                                )
                            });
                            form = form.push(field("Dias", days.into()));
                        }
                        _ => {}
                    }
                    let name = sel.as_ref().map(|m| m.name.clone()).unwrap_or_default();
                    let title = if self.editing.is_some() { "Editando agendamento de" } else { "Novo agendamento de" };
                    row![
                        pick,
                        column![
                            row![text(format!("{title}:")).size(12).color(MUTED), text(name).size(14)].spacing(6),
                            form
                        ]
                        .spacing(6),
                        rbtn_w(ic, Icon::Clock, "Finalizar agendamento", Message::SchedCreate, true, 90)
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into()
                };
                row![
                    group("Agendamento", form),
                    group(
                        "Gerenciar",
                        row![
                            rbtn_w(ic, Icon::Play, "Executar agora", Message::SchedRunNow, scheduled, 90),
                            if is_enabled {
                                rbtn_w(ic, Icon::Stop, "Desabilitar", Message::SchedToggle, scheduled, 90)
                            } else {
                                rbtn_w(ic, Icon::Check, "Habilitar", Message::SchedToggle, scheduled, 90)
                            },
                            rbtn_w(ic, Icon::New, "Duplicar", Message::SchedDuplicate, sel.is_some(), 90),
                            rbtn_w(ic, Icon::Delete, "Remover", Message::SchedDelete, removable, 90),
                        ]
                        .spacing(4)
                        .into()
                    ),
                ]
                .spacing(12)
                .into()
            }
            Tab::View => row![group(
                "Tabela",
                row![rbtn(
                    ic,
                    Icon::Lines,
                    if self.show_numbers { "Ocultar números" } else { "Números de linha" },
                    Message::ToggleNumbers,
                    true
                )]
                .into()
            )]
            .into(),
            Tab::Help => row![
                group(
                    "Atalhos globais",
                    column![
                        text("F9    gravar (o app vai para a bandeja)").size(13),
                        text("ESC   parar gravação").size(13),
                        text("F10   reproduzir").size(13),
                        text("F11   parar").size(13),
                    ]
                    .spacing(2)
                    .into()
                ),
                group(
                    "Sobre",
                    row![
                        ic.get(Icon::Info, 28.0, true),
                        column![text("Korvon Macro 1.0").size(14), text("Gravador e reprodutor de macros").size(12).color(MUTED)]
                            .spacing(2),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into()
                ),
            ]
            .spacing(12)
            .into(),
        };
        let ribbon = container(ribbon_row)
            .padding([8, 16])
            .width(Length::Fill)
            .height(Length::Fixed(if self.tab == Tab::Schedules { 124.0 } else { 104.0 }))
            .style(surface(RIBBON, 0.0, Some(BORDER)));

        // --- Tabela de ações --------------------------------------------------
        let head = |s: &'static str| text(s).size(12).color(MUTED);
        let mut head_row = row![Space::new(26, 1)];
        if self.show_numbers {
            head_row = head_row.push(head("#").width(44));
        }
        let table_head = container(
            head_row
                .push(head("Ação").width(Length::FillPortion(3)))
                .push(head("Valor").width(Length::FillPortion(8)))
                .push(head("Rótulo").width(Length::FillPortion(3)))
                .push(head("Comentário").width(Length::FillPortion(3)))
                .spacing(10)
                .padding([0, 8]),
        )
        .padding([6, 0])
        .width(Length::Fill)
        .style(surface(HEADER, 0.0, Some(BORDER)));

        let first = rows.len().saturating_sub(MAX_ROWS);
        let mut lines: Vec<Element<Message>> = rows
            .iter()
            .enumerate()
            .skip(first)
            .map(|(i, r)| {
                let playing = self.playing_at.is_some_and(|p| r.contains(p));
                let mut line = row![ic.get(r.icon, 18.0, true)];
                if self.show_numbers {
                    line = line.push(text(i + 1).size(13).color(MUTED).width(44));
                }
                let line = line
                    .push(text(r.title.clone()).size(13).width(Length::FillPortion(3)))
                    .push(text(r.value.clone()).size(13).width(Length::FillPortion(8)))
                    .push(text(r.label.clone()).size(13).width(Length::FillPortion(3)))
                    .push(text("").size(13).width(Length::FillPortion(3)))
                    .spacing(10)
                    .align_y(Alignment::Center);
                button(line)
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(row_style(self.selected == Some(i), playing))
                    .on_press(Message::Select(i))
                    .into()
            })
            .collect();
        if rows.is_empty() {
            lines.push(
                container(
                    text("Nenhuma ação. Pressione F9 ou clique em Gravar para começar, ou abra/importe uma macro pelo menu Arquivo.")
                        .size(14)
                        .color(MUTED),
                )
                .padding(16)
                .into(),
            );
        }
        let table = if self.tab == Tab::Schedules {
            self.schedules_table()
        } else {
            container(column![
                table_head,
                container(
                    scrollable(Column::with_children(lines).spacing(1).width(Length::Fill)).id(self.list_id.clone())
                )
                .height(Length::Fill)
                .style(surface(rgb_hex(0xe6e6e6), 0.0, None)),
            ])
            .height(Length::Fill)
            .width(Length::Fill)
            .style(surface(WHITE, 0.0, Some(BORDER)))
        };

        // --- Barra de status -------------------------------------------------
        let total = rows::duration_ms(&self.current.actions);
        let status_bar = container(
            row![
                text(&self.status).size(12).color(if recording { REC_RED } else { MUTED }),
                horizontal_space(),
                text(format!("{} ações", rows.len())).size(12).color(MUTED),
                text(rows::format_duration(total)).size(12).color(MUTED),
            ]
            .spacing(24),
        )
        .padding([4, 12])
        .width(Length::Fill)
        .style(surface(WINDOW, 0.0, Some(BORDER)));

        let base: Element<Message> = column![
            tab_bar,
            ribbon,
            container(table).padding(Padding { top: 8.0, right: 0.0, bottom: 0.0, left: 0.0 }).height(Length::Fill),
            status_bar,
        ]
        .into();

        if !self.menu_open {
            return base;
        }

        // --- Menu Arquivo (dropdown) -------------------------------------------
        let item = |label: &'static str, shortcut: &'static str, msg: Message, enabled: bool| -> Element<'static, Message> {
            let b = button(row![text(label).size(13), horizontal_space(), text(shortcut).size(12).color(MUTED)])
                .padding([7, 16])
                .width(Length::Fill)
                .style(menu_item);
            if enabled {
                b.on_press(msg).into()
            } else {
                b.into()
            }
        };
        let rule = || container(Space::new(Length::Fill, 1)).style(surface(BORDER, 0.0, None));
        let menu = container(column![
            item("Novo", "Ctrl+N", Message::New, idle),
            item("Abrir…", "Ctrl+O", Message::Open, idle),
            item("Arquivos recentes", "", Message::CloseMenu, false),
            rule(),
            item("Salvar", "Ctrl+S", Message::Save, idle),
            item("Salvar como…", "Ctrl+Shift+S", Message::SaveAs, idle),
            item("Importar do Macro Recorder (.mrf)…", "", Message::Import, idle),
            rule(),
            item("Sair", "", Message::Quit, true),
        ])
        .width(320)
        .style(surface(WHITE, 2.0, Some(BORDER)));
        let positioned = container(menu)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding { top: 33.0, right: 0.0, bottom: 0.0, left: 8.0 });
        let catcher = mouse_area(Space::new(Length::Fill, Length::Fill)).on_press(Message::CloseMenu);
        stack![base, catcher, positioned].into()
    }
}

/// Diálogo de arquivo que começa na pasta da biblioteca de macros.
fn dialog_in_library() -> rfd::FileDialog {
    match library::dir() {
        Some(d) => rfd::FileDialog::new().set_directory(d),
        None => rfd::FileDialog::new(),
    }
}

/// Máscara `HH:MM`: só dígitos, hora até 23 e minuto até 59 (dígitos inválidos são descartados).
fn mask_time(input: &str) -> String {
    let mut d = String::new();
    for c in input.chars().filter(char::is_ascii_digit) {
        let ok = match d.len() {
            0 => c <= '2',
            1 => d != "2" || c <= '3',
            2 => c <= '5',
            _ => true,
        };
        if !ok {
            break;
        }
        d.push(c);
        if d.len() == 4 {
            break;
        }
    }
    if d.len() > 2 {
        format!("{}:{}", &d[..2], &d[2..])
    } else {
        d
    }
}

/// Máscara `dd/mm/aaaa`: só dígitos, dia até 31 e mês até 12.
fn mask_date(input: &str) -> String {
    let mut d = String::new();
    for c in input.chars().filter(char::is_ascii_digit) {
        let b = d.as_bytes();
        let ok = match d.len() {
            0 => c <= '3',
            1 => b[0] != b'3' || c <= '1',
            2 => c <= '1',
            3 => match b[2] {
                b'0' => c >= '1',
                _ => c <= '2',
            },
            _ => true,
        };
        if !ok {
            break;
        }
        d.push(c);
        if d.len() == 8 {
            break;
        }
    }
    match d.len() {
        0..=2 => d,
        3..=4 => format!("{}/{}", &d[..2], &d[2..]),
        _ => format!("{}/{}/{}", &d[..2], &d[2..4], &d[4..]),
    }
}

fn parse_time(s: &str) -> Result<(u32, u32), String> {
    let err = || "Hora incompleta. Digite no formato HH:MM.".to_string();
    let (h, m) = s.split_once(':').ok_or_else(err)?;
    if h.len() != 2 || m.len() != 2 {
        return Err(err());
    }
    Ok((h.parse().map_err(|_| err())?, m.parse().map_err(|_| err())?))
}

fn parse_date(s: &str) -> Result<(i32, u32, u32), String> {
    let err = || "Data inválida. Digite no formato dd/mm/aaaa.".to_string();
    let p: Vec<&str> = s.split('/').collect();
    if p.len() != 3 || p[0].len() != 2 || p[1].len() != 2 || p[2].len() != 4 {
        return Err(err());
    }
    let (d, m, y) = (p[0].parse::<u32>().map_err(|_| err())?, p[1].parse::<u32>().map_err(|_| err())?, p[2].parse::<i32>().map_err(|_| err())?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 28 + leap as u32,
        _ => return Err(err()),
    };
    if !(2000..=2100).contains(&y) || d == 0 || d > dim {
        return Err(err());
    }
    Ok((y, m, d))
}

fn rgb_hex(hex: u32) -> iced::Color {
    iced::Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Campo com legenda pequena acima.
fn field<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
    column![text(label.to_string()).size(11).color(theme::MUTED), content].spacing(4).into()
}

/// Botão grande da faixa de opções: ícone colorido sobre o rótulo.
fn rbtn<'a>(icons: &Icons, ic: Icon, label: &'a str, msg: Message, enabled: bool) -> Element<'a, Message> {
    rbtn_w(icons, ic, label, msg, enabled, 104)
}

/// Como `rbtn`, com largura própria (a aba Agendamentos usa botões mais estreitos).
fn rbtn_w<'a>(icons: &Icons, ic: Icon, label: &'a str, msg: Message, enabled: bool, width: u16) -> Element<'a, Message> {
    let b = button(
        column![icons.get(ic, 34.0, enabled), text(label).size(12)]
            .spacing(5)
            .width(Length::Fixed(width as f32))
            .align_x(Alignment::Center),
    )
    .padding([6, 4])
    .style(theme::ribbon_button);
    if enabled {
        b.on_press(msg).into()
    } else {
        b.into()
    }
}


#[cfg(test)]
mod mask_tests {
    use super::*;

    #[test]
    fn time_mask_limits_hour_and_minute() {
        assert_eq!(mask_time("0830"), "08:30");
        assert_eq!(mask_time("2359"), "23:59");
        assert_eq!(mask_time("24"), "2");
        assert_eq!(mask_time("2460"), "2");
        assert_eq!(mask_time("0860"), "08");
        assert_eq!(mask_time("3"), "");
        assert_eq!(mask_time("08:3"), "08:3");
        assert_eq!(mask_time("08:"), "08");
        assert_eq!(parse_time("08:30"), Ok((8, 30)));
        assert!(parse_time("08:3").is_err());
    }

    #[test]
    fn date_mask_and_parse() {
        assert_eq!(mask_date("25122030"), "25/12/2030");
        assert_eq!(mask_date("2512"), "25/12");
        assert_eq!(mask_date("32"), "3");
        assert_eq!(mask_date("1313"), "13/1");
        assert_eq!(mask_date("1200"), "12/0");
        assert_eq!(parse_date("25/12/2030"), Ok((2030, 12, 25)));
        assert!(parse_date("31/02/2030").is_err());
        assert!(parse_date("25/12/20").is_err());
    }
}
