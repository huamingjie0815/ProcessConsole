use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scanned_at_unix: u64,
    pub groups: Vec<ProjectGroup>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGroup {
    pub project: String,
    pub processes: Vec<ListeningProcess>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListeningProcess {
    pub pid: i32,
    pub ppid: i32,
    pub pgid: i32,
    pub user: String,
    pub elapsed: String,
    pub command: String,
    pub args_masked: String,
    pub cwd: Option<String>,
    pub project: String,
    pub ports: Vec<ListeningPort>,
    pub agent: AgentGuess,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ListeningPort {
    pub address: String,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentGuess {
    pub name: Option<String>,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: i32,
    ppid: i32,
    pgid: i32,
    user: String,
    elapsed: String,
    args: String,
}

pub fn scan_processes_impl() -> Result<ScanResult, String> {
    let mut errors = Vec::new();
    let ps_output = run_command("ps", &["-axo", "pid=,ppid=,pgid=,user=,etime=,args="])?;
    let processes = parse_ps(&ps_output);
    let process_by_pid: HashMap<i32, ProcessInfo> = processes.into_iter().map(|p| (p.pid, p)).collect();

    let lsof_output = run_command("lsof", &["-nP", "-iTCP", "-sTCP:LISTEN"])?;
    let ports_by_pid = parse_lsof_listeners(&lsof_output);

    let mut rows = Vec::new();
    for (pid, ports) in ports_by_pid {
        let Some(process) = process_by_pid.get(&pid) else {
            continue;
        };

        let cwd = match read_cwd(pid) {
            Ok(cwd) => cwd,
            Err(err) => {
                errors.push(format!("PID {pid}: {err}"));
                None
            }
        };
        let project = cwd
            .as_deref()
            .and_then(find_project_root)
            .unwrap_or_else(|| "Unknown Project".to_string());
        let agent = guess_agent(process, &process_by_pid);

        rows.push(ListeningProcess {
            pid: process.pid,
            ppid: process.ppid,
            pgid: process.pgid,
            user: process.user.clone(),
            elapsed: process.elapsed.clone(),
            command: command_label(&process.args),
            args_masked: mask_sensitive_args(&process.args),
            cwd,
            project,
            ports,
            agent,
        });
    }

    rows.sort_by(|a, b| {
        a.project
            .cmp(&b.project)
            .then_with(|| a.ports.first().map(|p| p.port).cmp(&b.ports.first().map(|p| p.port)))
            .then_with(|| a.pid.cmp(&b.pid))
    });

    let mut groups_by_project: HashMap<String, Vec<ListeningProcess>> = HashMap::new();
    for row in rows {
        groups_by_project
            .entry(row.project.clone())
            .or_default()
            .push(row);
    }

    let mut groups: Vec<ProjectGroup> = groups_by_project
        .into_iter()
        .map(|(project, processes)| ProjectGroup { project, processes })
        .collect();
    groups.sort_by(|a, b| a.project.cmp(&b.project));

    Ok(ScanResult {
        scanned_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_secs(),
        groups,
        errors,
    })
}

pub fn terminate_process_group_impl(pgid: i32, force: bool) -> Result<(), String> {
    if pgid <= 1 {
        return Err("refusing to terminate a protected process group".to_string());
    }

    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let target = process_group_signal_target(pgid);
    let result = unsafe { libc::kill(target, signal) };

    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn process_group_signal_target(pgid: i32) -> i32 {
    -pgid
}

fn run_command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn parse_ps(output: &str) -> Vec<ProcessInfo> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            let pgid = parts.next()?.parse().ok()?;
            let user = parts.next()?.to_string();
            let elapsed = parts.next()?.to_string();
            let args = parts.collect::<Vec<_>>().join(" ");
            Some(ProcessInfo {
                pid,
                ppid,
                pgid,
                user,
                elapsed,
                args,
            })
        })
        .collect()
}

fn parse_lsof_listeners(output: &str) -> HashMap<i32, Vec<ListeningPort>> {
    let mut ports_by_pid: HashMap<i32, Vec<ListeningPort>> = HashMap::new();

    for line in output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }

        let Ok(pid) = cols[1].parse::<i32>() else {
            continue;
        };
        let name = cols[8..].join(" ");
        let Some((address, port)) = parse_lsof_tcp_name(&name) else {
            continue;
        };

        ports_by_pid
            .entry(pid)
            .or_default()
            .push(ListeningPort {
                address,
                port,
                protocol: "TCP".to_string(),
            });
    }

    for ports in ports_by_pid.values_mut() {
        ports.sort_by_key(|port| port.port);
        ports.dedup_by(|a, b| a.port == b.port && a.address == b.address);
    }

    ports_by_pid
}

fn parse_lsof_tcp_name(name: &str) -> Option<(String, u16)> {
    let endpoint = name
        .strip_prefix("TCP ")
        .unwrap_or(name)
        .split_whitespace()
        .next()?
        .split("->")
        .next()?;
    let (address, port_text) = endpoint.rsplit_once(':')?;
    let port = port_text
        .trim_matches(|ch: char| !ch.is_ascii_digit())
        .parse()
        .ok()?;
    Some((address.to_string(), port))
}

fn read_cwd(pid: i32) -> Result<Option<String>, String> {
    let pid_text = pid.to_string();
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid_text, "-d", "cwd", "-Fn"])
        .output()
        .map_err(|err| format!("failed to read cwd: {err}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .find_map(|line| line.strip_prefix('n').map(ToOwned::to_owned)))
}

fn find_project_root(cwd: &str) -> Option<String> {
    let mut current = PathBuf::from(cwd);
    if current.is_file() {
        current.pop();
    }

    loop {
        if has_project_marker(&current) {
            return Some(current.to_string_lossy().to_string());
        }
        if !current.pop() {
            break;
        }
    }

    Some(cwd.to_string())
}

fn has_project_marker(path: &Path) -> bool {
    [
        ".git",
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "go.mod",
        "deno.json",
    ]
    .iter()
    .any(|marker| path.join(marker).exists())
}

fn guess_agent(process: &ProcessInfo, process_by_pid: &HashMap<i32, ProcessInfo>) -> AgentGuess {
    let context = process_context(process, process_by_pid).to_lowercase();
    let candidates = [
        score_agent("Cursor", &context, &["cursor.app", "cursor helper", ".cursor/"]),
        score_agent(
            "Claude Code",
            &context,
            &["claude-code", "/claude", " claude ", ".claude/"],
        ),
        score_agent("opencode", &context, &["opencode", ".opencode/"]),
        score_agent("Codex", &context, &["@openai/codex", "/bin/codex", " codex"]),
        score_agent("OpenClaw", &context, &["openclaw", "open-claw", ".openclaw/"]),
    ];

    let Some(best) = candidates.into_iter().max_by_key(|candidate| candidate.0) else {
        return unknown_agent();
    };

    if best.0 == 0 {
        return unknown_agent();
    }

    AgentGuess {
        name: Some(best.1.to_string()),
        confidence: match best.0 {
            5.. => Confidence::High,
            3..=4 => Confidence::Medium,
            _ => Confidence::Low,
        },
        evidence: best.2,
    }
}

fn process_context(process: &ProcessInfo, process_by_pid: &HashMap<i32, ProcessInfo>) -> String {
    let mut pieces = vec![format!("pid {}: {}", process.pid, process.args)];
    let mut parent_pid = process.ppid;
    let mut seen = 0;

    while let Some(parent) = process_by_pid.get(&parent_pid) {
        pieces.push(format!("ancestor {}: {}", parent.pid, parent.args));
        parent_pid = parent.ppid;
        seen += 1;
        if seen >= 12 || parent_pid <= 1 {
            break;
        }
    }

    for sibling in process_by_pid
        .values()
        .filter(|candidate| candidate.pgid == process.pgid && candidate.pid != process.pid)
        .take(20)
    {
        pieces.push(format!("process-group {}: {}", sibling.pid, sibling.args));
    }

    pieces.join("\n")
}

fn score_agent<'a>(
    name: &'a str,
    context: &str,
    needles: &[&str],
) -> (u8, &'a str, Vec<String>) {
    let mut score = 0;
    let mut evidence = Vec::new();

    for needle in needles {
        if context.contains(needle) {
            score += if needle.starts_with('.') { 2 } else { 3 };
            evidence.push(format!("matched `{needle}`"));
        }
    }

    (score.min(8), name, evidence)
}

fn unknown_agent() -> AgentGuess {
    AgentGuess {
        name: None,
        confidence: Confidence::Unknown,
        evidence: Vec::new(),
    }
}

fn command_label(args: &str) -> String {
    let lower = args.to_lowercase();
    for known in [
        "vite",
        "next",
        "astro",
        "nuxt",
        "webpack",
        "node",
        "python",
        "uvicorn",
        "cargo",
        "tauri",
        "pnpm",
        "npm",
    ] {
        if lower.contains(known) {
            return known.to_string();
        }
    }

    args.split_whitespace()
        .next()
        .and_then(|first| Path::new(first).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("process")
        .to_string()
}

fn mask_sensitive_args(args: &str) -> String {
    let mut mask_next = false;
    let mut output = Vec::new();

    for token in args.split_whitespace() {
        let lower = token.to_lowercase();
        if mask_next {
            output.push("[redacted]".to_string());
            mask_next = false;
            continue;
        }

        if is_sensitive_flag(&lower) {
            if let Some((key, _value)) = token.split_once('=') {
                output.push(format!("{key}=[redacted]"));
            } else {
                output.push(token.to_string());
                mask_next = true;
            }
            continue;
        }

        if let Some((key, _value)) = token.split_once('=') {
            if is_sensitive_key(&key.to_lowercase()) {
                output.push(format!("{key}=[redacted]"));
                continue;
            }
        }

        output.push(token.to_string());
    }

    output.join(" ")
}

fn is_sensitive_flag(token: &str) -> bool {
    let flag = token.trim_start_matches('-');
    is_sensitive_key(flag)
}

fn is_sensitive_key(key: &str) -> bool {
    [
        "token", "secret", "password", "passwd", "api-key", "api_key", "apikey", "auth",
    ]
        .iter()
        .any(|sensitive| key.contains(sensitive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lsof_listener_ports() {
        let output = "\
COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME
node    11144  hmj   23u  IPv4  0x123      0t0  TCP 127.0.0.1:5173 (LISTEN)
python  11145  hmj   10u  IPv6  0x456      0t0  TCP *:8000 (LISTEN)
";

        let ports = parse_lsof_listeners(output);

        assert_eq!(ports[&11144][0].address, "127.0.0.1");
        assert_eq!(ports[&11144][0].port, 5173);
        assert_eq!(ports[&11145][0].address, "*");
        assert_eq!(ports[&11145][0].port, 8000);
    }

    #[test]
    fn parses_ps_with_args_containing_spaces() {
        let output = "11144 9769 43098 hmj 01:02:03 node /tmp/app/node_modules/.bin/vite --host 127.0.0.1\n";

        let processes = parse_ps(output);

        assert_eq!(processes[0].pid, 11144);
        assert_eq!(processes[0].ppid, 9769);
        assert_eq!(processes[0].pgid, 43098);
        assert_eq!(
            processes[0].args,
            "node /tmp/app/node_modules/.bin/vite --host 127.0.0.1"
        );
    }

    #[test]
    fn guesses_cursor_from_ancestor_chain() {
        let server = ProcessInfo {
            pid: 30,
            ppid: 20,
            pgid: 10,
            user: "hmj".to_string(),
            elapsed: "00:01".to_string(),
            args: "node node_modules/.bin/vite".to_string(),
        };
        let parent = ProcessInfo {
            pid: 20,
            ppid: 10,
            pgid: 10,
            user: "hmj".to_string(),
            elapsed: "00:02".to_string(),
            args: "Cursor Helper: terminal pty-host".to_string(),
        };
        let root = ProcessInfo {
            pid: 10,
            ppid: 1,
            pgid: 10,
            user: "hmj".to_string(),
            elapsed: "00:03".to_string(),
            args: "/Applications/Cursor.app/Contents/MacOS/Cursor".to_string(),
        };
        let process_by_pid = [(30, server.clone()), (20, parent), (10, root)]
            .into_iter()
            .collect();

        let guess = guess_agent(&server, &process_by_pid);

        assert_eq!(guess.name.as_deref(), Some("Cursor"));
        assert_eq!(guess.confidence, Confidence::High);
    }

    #[test]
    fn guesses_codex_from_process_group() {
        let server = ProcessInfo {
            pid: 40,
            ppid: 1,
            pgid: 90,
            user: "hmj".to_string(),
            elapsed: "00:01".to_string(),
            args: "node node_modules/.bin/vite".to_string(),
        };
        let codex = ProcessInfo {
            pid: 90,
            ppid: 1,
            pgid: 90,
            user: "hmj".to_string(),
            elapsed: "00:05".to_string(),
            args: "node /Users/hmj/.nvm/versions/node/v22/bin/codex".to_string(),
        };
        let process_by_pid = [(40, server.clone()), (90, codex)].into_iter().collect();

        let guess = guess_agent(&server, &process_by_pid);

        assert_eq!(guess.name.as_deref(), Some("Codex"));
        assert_eq!(guess.confidence, Confidence::Medium);
    }

    #[test]
    fn masks_sensitive_arguments() {
        let masked = mask_sensitive_args(
            "server --token abc API_KEY=123 --password=secret --safe value --auth bearer",
        );

        assert_eq!(
            masked,
            "server --token [redacted] API_KEY=[redacted] --password=[redacted] --safe value --auth [redacted]"
        );
    }

    #[test]
    fn uses_negative_pid_to_signal_process_group() {
        assert_eq!(process_group_signal_target(34830), -34830);
    }
}
