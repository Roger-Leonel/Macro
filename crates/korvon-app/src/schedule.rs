//! Agendamento de macros no Agendador de Tarefas do Windows.
//!
//! Cada agendamento é uma tarefa em `\Korvon Macro\<macro> (<id>)` que executa
//! `korvon-macro.exe --run <arquivo.kmacro> --speed X --repeat N`. Uma macro pode ter
//! vários agendamentos (um id diferente para cada). O Agendador é a fonte da verdade: frequência, hora, data e
//! dias são lidos de volta da descrição da tarefa, sem índice próprio.

use korvon_core::Macro;
use serde::Deserialize;
use std::fmt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const FOLDER: &str = "Korvon Macro";
/// Prefixo da descrição legível por máquina: `KV1|freq|HH:MM|AAAA-MM-DD|dias|atraso|macro`.
const TAG: &str = "KV1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    Once,
    Daily,
    Weekly,
    Logon,
}

impl Freq {
    pub const ALL: [Freq; 4] = [Freq::Once, Freq::Daily, Freq::Weekly, Freq::Logon];

    fn code(self) -> &'static str {
        match self {
            Freq::Once => "once",
            Freq::Daily => "daily",
            Freq::Weekly => "weekly",
            Freq::Logon => "logon",
        }
    }

    fn from_code(s: &str) -> Option<Freq> {
        Freq::ALL.into_iter().find(|f| f.code() == s)
    }
}

impl fmt::Display for Freq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Freq::Once => "Uma vez",
            Freq::Daily => "Diariamente",
            Freq::Weekly => "Semanalmente",
            Freq::Logon => "Ao entrar no Windows",
        })
    }
}

pub const DAY_LABELS: [&str; 7] = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
const DAY_XML: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Tudo o que o formulário coleta.
pub struct Request<'a> {
    /// Nome da macro.
    pub name: &'a str,
    /// Tarefa existente a substituir; `None` cria um novo agendamento.
    pub task: Option<&'a str>,
    pub enabled: bool,
    pub file: &'a Path,
    pub freq: Freq,
    pub time: (u32, u32),
    /// `(ano, mês, dia)`, usado só em `Freq::Once`.
    pub date: (i32, u32, u32),
    pub days: [bool; 7],
    /// Segundos de espera após o logon (só `Freq::Logon`).
    pub delay: u32,
    pub speed: f64,
    pub repeat: u32,
}

/// Configuração de um agendamento, como gravada na descrição da tarefa.
#[derive(Debug, Clone)]
pub struct Info {
    pub freq: Freq,
    pub time: (u32, u32),
    pub date: (i32, u32, u32),
    pub days: [bool; 7],
    pub delay: u32,
    /// Nome da macro agendada.
    pub macro_name: String,
}

impl Info {
    fn encode(&self) -> String {
        let days: String = self.days.iter().map(|&d| if d { '1' } else { '0' }).collect();
        format!(
            "{TAG}|{}|{:02}:{:02}|{:04}-{:02}-{:02}|{days}|{}|{}",
            self.freq.code(),
            self.time.0,
            self.time.1,
            self.date.0,
            self.date.1,
            self.date.2,
            self.delay,
            self.macro_name
        )
    }

    fn decode(s: &str) -> Option<Info> {
        let p: Vec<&str> = s.splitn(7, '|').collect();
        if p.len() < 5 || p[0] != TAG {
            return None;
        }
        let (h, m) = p[2].split_once(':')?;
        let mut d = p[3].split('-');
        let mut days = [false; 7];
        for (i, c) in p[4].chars().take(7).enumerate() {
            days[i] = c == '1';
        }
        Some(Info {
            freq: Freq::from_code(p[1])?,
            time: (h.parse().ok()?, m.parse().ok()?),
            date: (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?),
            days,
            delay: p.get(5).and_then(|d| d.parse().ok()).unwrap_or(0),
            macro_name: p.get(6).map(|m| m.to_string()).unwrap_or_default(),
        })
    }

    pub fn freq_label(&self) -> String {
        match self.freq {
            Freq::Weekly => {
                let names: Vec<&str> = (0..7).filter(|&i| self.days[i]).map(|i| DAY_LABELS[i]).collect();
                format!("Semanal ({})", names.join(", "))
            }
            f => f.to_string(),
        }
    }

    pub fn time_label(&self) -> String {
        if self.freq == Freq::Logon {
            if self.delay > 0 { format!("+{}s", self.delay) } else { "—".into() }
        } else {
            format!("{:02}:{:02}", self.time.0, self.time.1)
        }
    }

    pub fn date_label(&self) -> String {
        if self.freq == Freq::Once {
            format!("{:02}/{:02}/{:04}", self.date.2, self.date.1, self.date.0)
        } else {
            "—".into()
        }
    }
}

/// Tarefa lida de volta do Agendador.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub name: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    state: String,
}

impl Entry {
    pub fn enabled(&self) -> bool {
        self.state != "Disabled"
    }

    /// Macro agendada (tarefas antigas, sem o campo, usam o nome da tarefa).
    pub fn macro_name(&self) -> String {
        match self.info() {
            Some(i) if !i.macro_name.is_empty() => i.macro_name,
            _ => self.name.clone(),
        }
    }

    pub fn info(&self) -> Option<Info> {
        Info::decode(&self.desc)
    }
}

pub fn clean_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !"\\/:*?\"<>|".contains(*c) && !c.is_control()).collect();
    n.trim().trim_end_matches('.').to_string()
}

fn task_path(name: &str) -> String {
    format!("{FOLDER}\\{name}")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(d) if !d.is_empty() => format!("{d}\\{user}"),
        _ => user,
    }
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("não foi possível executar {program}: {e}"))?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
    if out.status.success() {
        Ok(text(&out.stdout))
    } else {
        let msg = text(&out.stderr);
        Err(if msg.is_empty() { text(&out.stdout) } else { msg })
    }
}

fn new_id() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ (d.as_secs() as u32)).unwrap_or(0);
    format!("{:06x}", n & 0xff_ffff)
}

/// Cria (ou substitui, se `req.task` for informado) o agendamento. Devolve o nome da tarefa.
pub fn create(req: &Request) -> Result<String, String> {
    let name = clean_name(req.name);
    if name.is_empty() {
        return Err("Macro sem nome.".into());
    }
    let task = match req.task {
        Some(t) => clean_name(t),
        None => format!("{name} ({})", new_id()),
    };
    let m = Macro::load(req.file).map_err(|e| format!("Erro ao abrir a macro: {e}"))?;
    if m.actions.is_empty() {
        return Err("A macro está vazia.".into());
    }
    korvon_core::resolve_labels(&m).map_err(|e| format!("Macro inválida: {e}"))?;

    let (h, mi) = req.time;
    let (y, mo, d) = req.date;
    let trigger = match req.freq {
        Freq::Once => format!(
            "<TimeTrigger><StartBoundary>{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:00</StartBoundary><Enabled>true</Enabled></TimeTrigger>"
        ),
        Freq::Daily => format!(
            "<CalendarTrigger><StartBoundary>2000-01-01T{h:02}:{mi:02}:00</StartBoundary><Enabled>true</Enabled>\
             <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>"
        ),
        Freq::Weekly => {
            if !req.days.iter().any(|&d| d) {
                return Err("Escolha ao menos um dia da semana.".into());
            }
            let days: String = (0..7).filter(|&i| req.days[i]).map(|i| format!("<{}/>", DAY_XML[i])).collect();
            format!(
                "<CalendarTrigger><StartBoundary>2000-01-01T{h:02}:{mi:02}:00</StartBoundary><Enabled>true</Enabled>\
                 <ScheduleByWeek><WeeksInterval>1</WeeksInterval><DaysOfWeek>{days}</DaysOfWeek></ScheduleByWeek></CalendarTrigger>"
            )
        }
        // Sem `UserId` o gatilho vale para qualquer usuário e exigiria administrador.
        Freq::Logon => format!(
            "<LogonTrigger><Enabled>true</Enabled><UserId>{}</UserId><Delay>PT{}S</Delay></LogonTrigger>",
            xml_escape(&current_user()),
            req.delay
        ),
    };

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let speed = if req.speed.is_finite() && req.speed > 0.0 { req.speed } else { 1.0 };
    let arguments = format!("--run \"{}\" --speed {speed} --repeat {}", req.file.display(), req.repeat);
    let info = Info { freq: req.freq, time: req.time, date: req.date, days: req.days, delay: req.delay, macro_name: name.clone() };

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>{desc}</Description></RegistrationInfo>
  <Triggers>{trigger}</Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>false</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>{enabled}</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{cmd}</Command><Arguments>{args}</Arguments></Exec></Actions>
</Task>"#,
        enabled = req.enabled,
        desc = xml_escape(&info.encode()),
        cmd = xml_escape(&exe.display().to_string()),
        args = xml_escape(&arguments),
    );
    // O Agendador exige UTF-16 com BOM.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(|u| u.to_le_bytes()));
    let xml_path = std::env::temp_dir().join(format!("korvon-task-{}.xml", std::process::id()));
    std::fs::write(&xml_path, bytes).map_err(|e| e.to_string())?;

    let result = run("schtasks", &["/Create", "/TN", &task_path(&task), "/XML", &xml_path.display().to_string(), "/F"]);
    let _ = std::fs::remove_file(&xml_path);
    result.map(|_| task).map_err(|e| format!("O Agendador recusou a tarefa: {e}"))
}

/// Remove o agendamento (a macro em si não é tocada).
pub fn delete(name: &str) -> Result<(), String> {
    run("schtasks", &["/Delete", "/TN", &task_path(&clean_name(name)), "/F"]).map(|_| ())
}

/// Habilita ou desabilita (pausa) o agendamento sem apagá-lo.
pub fn set_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let flag = if enabled { "/ENABLE" } else { "/DISABLE" };
    run("schtasks", &["/Change", "/TN", &task_path(&clean_name(name)), flag]).map(|_| ())
}

pub fn run_now(name: &str) -> Result<(), String> {
    run("schtasks", &["/Run", "/TN", &task_path(&clean_name(name))]).map(|_| ())
}

/// Lê as tarefas da pasta `\Korvon Macro\` (usa PowerShell; demora algumas centenas de ms).
pub fn list() -> Result<Vec<Entry>, String> {
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
         $t=@(Get-ScheduledTask -TaskPath '\\{FOLDER}\\' -ErrorAction SilentlyContinue); \
         $r=@($t | ForEach-Object {{ [pscustomobject]@{{name=$_.TaskName;desc=[string]$_.Description;state=[string]$_.State}} }}); \
         ConvertTo-Json -InputObject $r -Compress"
    );
    let out = run("powershell", &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])?;
    let out = out.trim_start_matches('\u{feff}');
    if out.is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(out).map_err(|e| format!("resposta inesperada do Agendador: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use korvon_core::Action;

    #[test]
    fn info_roundtrip() {
        let i = Info {
            freq: Freq::Weekly,
            time: (8, 5),
            date: (2030, 12, 25),
            days: [true, false, true, false, false, false, true],
            delay: 45,
            macro_name: "Minha | macro".into(),
        };
        let d = Info::decode(&i.encode()).unwrap();
        assert_eq!((d.freq, d.time, d.date, d.days, d.delay), (i.freq, i.time, i.date, i.days, 45));
        assert_eq!(d.macro_name, "Minha | macro");
        assert_eq!(d.freq_label(), "Semanal (Seg, Qua, Dom)");
        assert!(Info::decode("lixo").is_none());
    }

    /// Toca no Agendador real; rode com `cargo test -p korvon-app -- --ignored`.
    #[test]
    #[ignore]
    fn create_list_delete() {
        let file = std::env::temp_dir().join("zz-teste àç.kmacro");
        Macro::new("t", vec![Action::Wait { ms: 10 }]).save(&file).unwrap();
        let req = |freq, task| Request {
            name: "zz-teste àç",
            task,
            enabled: true,
            file: &file,
            freq,
            time: (8, 30),
            date: (2030, 12, 25),
            days: [true, false, true, false, false, false, false],
            delay: 30,
            speed: 1.0,
            repeat: 1,
        };
        let mut tasks = vec![];
        for f in Freq::ALL {
            let t = create(&req(f, None)).unwrap_or_else(|e| panic!("{f}: {e}"));
            let l = list().unwrap();
            let e = l.iter().find(|e| e.name == t).unwrap_or_else(|| panic!("{f}: {l:?}"));
            assert_eq!(e.info().unwrap().freq, f);
            assert_eq!(e.macro_name(), "zz-teste àç");
            assert!(e.enabled());
            tasks.push(t);
        }
        // Quatro agendamentos distintos da mesma macro.
        assert_eq!(tasks.iter().collect::<std::collections::HashSet<_>>().len(), 4);
        set_enabled(&tasks[0], false).unwrap();
        assert!(!list().unwrap().iter().find(|e| e.name == tasks[0]).unwrap().enabled());
        set_enabled(&tasks[0], true).unwrap();
        assert!(list().unwrap().iter().find(|e| e.name == tasks[0]).unwrap().enabled());
        // Substituir mantendo o nome da tarefa.
        assert_eq!(create(&req(Freq::Daily, Some(&tasks[0]))).unwrap(), tasks[0]);
        for t in &tasks {
            delete(t).unwrap();
        }
        assert!(!list().unwrap().iter().any(|e| e.macro_name() == "zz-teste àç"));
        let _ = std::fs::remove_file(&file);
    }
}
