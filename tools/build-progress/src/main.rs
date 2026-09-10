use std::collections::VecDeque;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BAR_WIDTH: usize = 32;
const RATE_WINDOW: usize = 64;
const MAX_PROBLEMS: usize = 200;
const MAX_CONTEXT_LINES: usize = 8;

struct Config {
    label: String,
    log: PathBuf,
    total: Option<u64>,
    action_patterns: Vec<String>,
    command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity { Warning, Error }

impl Severity {
    fn label(self) -> &'static str { match self { Self::Warning => "WARNING", Self::Error => "ERROR" } }
    fn color(self) -> &'static str { match self { Self::Warning => "1;33", Self::Error => "1;31" } }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    severity: Severity,
    message: String,
    location: Option<String>,
    context: Vec<String>,
}

impl Diagnostic {
    fn from_start(line: &str) -> Option<Self> {
        let trimmed = line.trim_start();
        let (severity, message) = if let Some(value) = trimmed.strip_prefix("warning:") {
            (Severity::Warning, value.trim())
        } else if let Some(value) = trimmed.strip_prefix("error:") {
            (Severity::Error, value.trim())
        } else if let Some(value) = trimmed.strip_prefix("warning[") {
            let message = value.split_once(']').map(|(_, rest)| rest.trim()).unwrap_or(value);
            (Severity::Warning, message)
        } else if let Some(value) = trimmed.strip_prefix("error[") {
            let message = value.split_once(']').map(|(_, rest)| rest.trim()).unwrap_or(value);
            (Severity::Error, message)
        } else if let Some((prefix, message)) = split_inline_diagnostic(trimmed, ": warning:") {
            return Some(Self { severity: Severity::Warning, message: message.trim().to_string(), location: Some(prefix.trim().to_string()), context: Vec::new() });
        } else if let Some((prefix, message)) = split_inline_diagnostic(trimmed, ": error:") {
            return Some(Self { severity: Severity::Error, message: message.trim().to_string(), location: Some(prefix.trim().to_string()), context: Vec::new() });
        } else {
            return None;
        };

        Some(Self { severity, message: message.to_string(), location: None, context: Vec::new() })
    }

    fn add_line(&mut self, line: &str) {
        if let Some(location) = parse_rust_location(line) {
            self.location = Some(location);
        }
        if self.context.len() < MAX_CONTEXT_LINES && is_diagnostic_context(line) {
            self.push_context_if_useful(line);
        }
    }

    fn push_context_if_useful(&mut self, line: &str) {
        let value = line.trim_end();
        if value.trim().is_empty() || self.context.iter().any(|item| item == value) { return; }
        if self.context.len() < MAX_CONTEXT_LINES { self.context.push(value.to_string()); }
    }
}

fn split_inline_diagnostic<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    line.find(marker).map(|index| { let (prefix, rest) = line.split_at(index); (prefix, &rest[marker.len()..]) })
}

fn parse_rust_location(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("-->")?.trim();
    if rest.is_empty() { None } else { Some(rest.to_string()) }
}

fn is_source_context_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() { index += 1; }
    index > 0 && trimmed[index..].starts_with(" | ")
}

fn is_diagnostic_context(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.starts_with(' ') || line.starts_with('\t') || is_source_context_line(line)
        || trimmed.starts_with("note:") || trimmed.starts_with("help:")
        || trimmed.starts_with("For more information")
}

fn usage() -> ! {
    eprintln!("usage: build-progress --label LABEL --log PATH [--total N] [--action REGEX] -- COMMAND [ARGS...]");
    std::process::exit(2);
}

fn parse_args() -> Config {
    let mut args = env::args().skip(1);
    let mut label = None;
    let mut log = None;
    let mut total = None;
    let mut action_patterns = Vec::new();
    let mut command = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--label" => label = args.next(),
            "--log" => log = args.next().map(PathBuf::from),
            "--total" => total = args.next().and_then(|value| value.parse::<u64>().ok()),
            "--action" => if let Some(pattern) = args.next() { action_patterns.push(pattern); },
            "--" => { command.extend(args); break; }
            _ => usage(),
        }
    }

    let label = label.unwrap_or_else(|| usage());
    let log = log.unwrap_or_else(|| usage());
    if command.is_empty() { usage(); }

    if action_patterns.is_empty() {
        action_patterns = vec![
            "^\\s+(?:HOST)?(?:CC|CXX|RUSTC|AR|LD|AS|OBJCOPY|OBJDUMP|STRIP|GEN|BUILD|BINDGEN|MODPOST|ZOFFSET)".into(),
            "^\\s*Compiling\\b".into(),
            "^\\s*Checking\\b".into(),
            "^\\s*\\[\\d+[/]\\d+\\]".into(),
        ];
    }
    Config { label, log, total, action_patterns, command }
}

fn matches_action(line: &str, patterns: &[String]) -> bool { patterns.iter().any(|pattern| simple_pattern_match(pattern, line)) }

fn simple_pattern_match(pattern: &str, line: &str) -> bool {
    match pattern {
        p if p.contains("HOST)?") && p.contains("ZOFFSET") => {
            let trimmed = line.trim_start();
            ["CC ", "CXX ", "RUSTC ", "AR ", "LD ", "AS ", "OBJCOPY ", "OBJDUMP ", "STRIP ", "GEN ", "BUILD ", "BINDGEN ", "MODPOST ", "ZOFFSET "]
                .iter().any(|prefix| trimmed.starts_with(prefix))
                || ["HOSTCC ", "HOSTCXX ", "HOSTRUSTC "].iter().any(|prefix| trimmed.starts_with(prefix))
        }
        p if p.contains("Compiling") => line.trim_start().starts_with("Compiling "),
        p if p.contains("Checking") => line.trim_start().starts_with("Checking "),
        p if p.contains("\\[\\d+") => {
            let t = line.trim_start();
            t.starts_with('[') && t.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) && t.contains("] ")
        }
        _ => line.contains(pattern),
    }
}

fn is_problem(line: &str, action_patterns: &[String]) -> bool {
    if matches_action(line, action_patterns) { return false; }
    let lower = line.trim_start().to_ascii_lowercase();
    Diagnostic::from_start(line).is_some()
        || lower.starts_with("fatal:") || lower.starts_with("fatal ")
        || lower.starts_with("collect2:") || lower.starts_with("ld.lld:")
        || lower.starts_with("clang:") || lower.starts_with("gcc:") || lower.starts_with("cc1:")
        || lower.starts_with("rustc:") || lower.starts_with("make: ***")
        || (lower.contains("make[") && lower.contains(": ***"))
        || lower.starts_with("ninja: build stopped") || lower.starts_with("undefined reference")
        || lower.contains("section mismatch") || lower.contains("undefined symbol")
        || lower.contains("relocation truncated") || lower.contains(": error:") || lower.contains(": warning:")
}

fn timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) { Ok(d) => format!("unix={}s", d.as_secs()), Err(_) => "unix=unknown".into() }
}

fn format_duration(duration: Duration) -> String {
    let mut seconds = duration.as_secs();
    let days = seconds / 86_400; seconds %= 86_400;
    let hours = seconds / 3_600; seconds %= 3_600;
    let minutes = seconds / 60; seconds %= 60;
    if days > 0 { format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, seconds) }
    else if hours > 0 { format!("{:02}:{:02}:{:02}", hours, minutes, seconds) }
    else { format!("{:02}:{:02}", minutes, seconds) }
}

fn terminal_width() -> usize {
    env::var("COLUMNS").ok().and_then(|value| value.parse::<usize>().ok()).filter(|&width| width >= 60).unwrap_or(100)
}

fn render_progress(label: &str, completed: u64, total: Option<u64>, elapsed: Duration, recent_steps: &VecDeque<Instant>) {
    let rate = if recent_steps.len() >= 2 {
        let first = recent_steps.front().unwrap(); let last = recent_steps.back().unwrap();
        let span = last.saturating_duration_since(*first).as_secs_f64();
        if span > 0.0 { (recent_steps.len() - 1) as f64 / span } else { 0.0 }
    } else if elapsed.as_secs_f64() > 0.0 { completed as f64 / elapsed.as_secs_f64() } else { 0.0 };

    let (percent, eta) = match total.filter(|&value| value > 0) {
        Some(total) => {
            let ratio = (completed as f64 / total as f64).clamp(0.0, 1.0);
            let eta = if rate > 0.0 && completed < total { Some(Duration::from_secs_f64((total - completed) as f64 / rate)) } else { None };
            (Some(ratio), eta)
        }
        None => (None, None),
    };

    let width = terminal_width();
    let suffix = match percent {
        Some(value) => format!(" {:>3.0}%  {:>9}  elapsed {}  ETA {}  {:>5.2}/s", value * 100.0, format!("{}/{}", completed, total.unwrap()), format_duration(elapsed), eta.map(format_duration).unwrap_or_else(|| "--:--".into()), rate),
        None => format!(" {:>9}  elapsed {}  {:>5.2}/s", completed, format_duration(elapsed), rate),
    };
    let fixed = label.len() + suffix.len() + 8;
    let bar_width = BAR_WIDTH.min(width.saturating_sub(fixed).max(10));
    let bar = match percent {
        Some(value) => { let filled = ((value * bar_width as f64).round() as usize).min(bar_width); format!("{}{}", "█".repeat(filled), "░".repeat(bar_width - filled)) }
        None => {
            let pos = if bar_width == 0 { 0 } else { (completed as usize) % (bar_width * 2).max(1) };
            let pos = if pos >= bar_width { bar_width * 2 - pos - 1 } else { pos };
            let mut chars = vec!['░'; bar_width]; if bar_width > 0 { chars[pos.min(bar_width - 1)] = '█'; }
            chars.into_iter().collect()
        }
    };
    print!("\r\x1b[2K{} [{}]{}", label, bar, suffix); let _ = io::stdout().flush();
}

fn print_problem(problem: &Diagnostic) {
    println!("\n\x1b[{}m[BUILD {}]\x1b[0m {}", problem.severity.color(), problem.severity.label(), problem.message);
    match &problem.location { Some(location) => println!("  Где: {}", location), None => println!("  Где: компилятор не указал место") }
    if !problem.context.is_empty() {
        println!("  Контекст:");
        for context in &problem.context { println!("    | {}", context.trim_end()); }
    }
}

fn flush_problem(active: &mut Option<Diagnostic>, problems: &mut Vec<Diagnostic>) {
    let Some(problem) = active.take() else { return };
    if problems.len() < MAX_PROBLEMS { print_problem(&problem); problems.push(problem); }
}

fn print_final(label: &str, status: &ExitStatus, completed: u64, elapsed: Duration, log: &Path, problems: &[Diagnostic]) {
    println!("\n{}: {}", label, if status.success() { "успешно" } else { "ОШИБКА" });
    println!("Время: {}", format_duration(elapsed));
    println!("Шагов: {}", completed);
    println!("Лог:   {}", log.display());
    if !problems.is_empty() {
        println!("\nПроблемы ({} диагностик):", problems.len());
        for problem in problems {
            println!("  [{}] {}{}", problem.severity.label(), problem.message, problem.location.as_deref().map(|value| format!(" @ {}", value)).unwrap_or_default());
        }
    }
}

fn command_string(command: &[String]) -> String {
    command.iter().map(|arg| {
        if arg.chars().all(|c| c.is_ascii_alphanumeric() || "_./-:=+".contains(c)) { arg.clone() }
        else { format!("'{}'", arg.replace('\'', "'\\''")) }
    }).collect::<Vec<_>>().join(" ")
}

fn run(config: Config) -> io::Result<i32> {
    if let Some(parent) = config.log.parent() { fs::create_dir_all(parent)?; }
    let mut log = File::create(&config.log)?;
    writeln!(log, "# Project Luna build log")?;
    writeln!(log, "# started={}", timestamp())?;
    writeln!(log, "# command={}", command_string(&config.command))?;
    writeln!(log)?;

    let mut child = Command::new(&config.command[0]).args(&config.command[1..]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    let tx_out = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line { Ok(line) => { let _ = tx_out.send(line); }, Err(error) => { let _ = tx_out.send(format!("[build-progress stdout read error: {}]", error)); break; } }
        }
    });
    let tx_err = tx.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            match line { Ok(line) => { let _ = tx_err.send(line); }, Err(error) => { let _ = tx_err.send(format!("[build-progress stderr read error: {}]", error)); break; } }
        }
    });
    drop(tx);

    let start = Instant::now();
    let mut completed = 0u64;
    let mut recent_steps = VecDeque::with_capacity(RATE_WINDOW);
    let mut problems = Vec::new();
    let mut active_problem = None;

    render_progress(&config.label, completed, config.total, Duration::ZERO, &recent_steps);
    for line in rx {
        writeln!(log, "{}", line)?;
        if let Some(diagnostic) = Diagnostic::from_start(&line) {
            flush_problem(&mut active_problem, &mut problems);
            active_problem = Some(diagnostic);
        } else if active_problem.is_some() && is_diagnostic_context(&line) {
            if let Some(problem) = active_problem.as_mut() { problem.add_line(&line); }
        } else if matches_action(&line, &config.action_patterns) {
            flush_problem(&mut active_problem, &mut problems);
            completed += 1;
            let now = Instant::now(); recent_steps.push_back(now);
            while recent_steps.len() > RATE_WINDOW { recent_steps.pop_front(); }
        } else if active_problem.is_some() {
            flush_problem(&mut active_problem, &mut problems);
        }
        render_progress(&config.label, completed, config.total, start.elapsed(), &recent_steps);
    }
    flush_problem(&mut active_problem, &mut problems);
    let status = child.wait()?;
    let elapsed = start.elapsed();
    render_progress(&config.label, completed, config.total, elapsed, &recent_steps);
    print_final(&config.label, &status, completed, elapsed, &config.log, &problems);
    Ok(status.code().unwrap_or(1))
}

fn main() -> io::Result<()> { let config = parse_args(); let code = run(config)?; std::process::exit(code); }

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns() -> Vec<String> {
        vec!["^\\s+(?:HOST)?(?:CC|CXX|RUSTC|AR|LD|AS|OBJCOPY|OBJDUMP|STRIP|GEN|BUILD|BINDGEN|MODPOST|ZOFFSET)".into()]
    }

    #[test]
    fn rust_diagnostic_keeps_location_and_context() {
        let mut diagnostic = Diagnostic::from_start("warning: struct `TargetManager` is never constructed").unwrap();
        diagnostic.add_line(" --> src/target.rs:24:1");
        diagnostic.add_line("  | ");
        diagnostic.add_line("24 | pub struct TargetManager { targets: Vec<BootTarget>, default_index: Option<usize> }");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.location.as_deref(), Some("src/target.rs:24:1"));
        assert!(diagnostic.context.iter().any(|line| line.contains("pub struct TargetManager")));
    }

    #[test]
    fn build_target_name_is_not_a_problem() {
        let line = "  CC      kernel/panic.o";
        assert!(matches_action(line, &patterns()));
        assert!(Diagnostic::from_start(line).is_none());
    }

    #[test]
    fn real_diagnostics_are_detected() {
        assert!(Diagnostic::from_start("kernel/foo.c:42:7: error: expected ';'").is_some());
        assert!(Diagnostic::from_start("warning: unused variable: x").is_some());
        assert!(Diagnostic::from_start("make[1]: *** [Makefile:123: target] Error 2").is_none());
        assert!(Diagnostic::from_start("ld.lld: error: undefined symbol: foo").is_some());
    }
}
