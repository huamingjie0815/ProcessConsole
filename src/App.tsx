import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  RefreshCw,
  Search,
  Square,
  Zap,
} from "lucide-react";
import { Fragment, useCallback, useEffect, useMemo, useState } from "react";

type Confidence = "High" | "Medium" | "Low" | "Unknown";

type ListeningPort = {
  address: string;
  port: number;
  protocol: string;
};

type AgentGuess = {
  name: string | null;
  confidence: Confidence;
  evidence: string[];
};

type ListeningProcess = {
  pid: number;
  ppid: number;
  pgid: number;
  user: string;
  elapsed: string;
  command: string;
  argsMasked: string;
  cwd: string | null;
  project: string;
  ports: ListeningPort[];
  agent: AgentGuess;
};

type ProjectGroup = {
  project: string;
  processes: ListeningProcess[];
};

type ScanResult = {
  scannedAtUnix: number;
  groups: ProjectGroup[];
  errors: string[];
};

type ActiveTab = "agent" | "other";

const AUTO_REFRESH_MS = 5000;

export function App() {
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<ActiveTab>("agent");
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [expandedRows, setExpandedRows] = useState<Set<number>>(new Set());
  const [terminating, setTerminating] = useState<number | null>(null);
  const [forceCandidate, setForceCandidate] = useState<number | null>(null);
  const [pendingTermination, setPendingTermination] = useState<{
    process: ListeningProcess;
    force: boolean;
  } | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<ScanResult>("scan_processes");
      setScan(next);
      setExpandedGroups((previous) => {
        if (previous.size > 0) return previous;
        return new Set(next.groups.map((group) => group.project));
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => void refresh(), AUTO_REFRESH_MS);
    return () => window.clearInterval(id);
  }, [refresh]);

  const tabCounts = useMemo(() => {
    const counts = { agent: 0, other: 0 };
    if (!scan) return counts;

    for (const group of scan.groups) {
      for (const process of group.processes) {
        if (isAgentProcess(process)) counts.agent += 1;
        else counts.other += 1;
      }
    }

    return counts;
  }, [scan]);

  const filteredGroups = useMemo(() => {
    if (!scan) return [];
    return filterGroups(scan.groups, activeTab, query);
  }, [activeTab, query, scan]);

  const processCount = scan?.groups.reduce((sum, group) => sum + group.processes.length, 0) ?? 0;
  const portCount =
    scan?.groups.reduce(
      (sum, group) =>
        sum + group.processes.reduce((inner, process) => inner + process.ports.length, 0),
      0,
    ) ?? 0;

  async function terminate(process: ListeningProcess, force: boolean) {
    setTerminating(process.pgid);
    setError(null);
    setPendingTermination(null);
    try {
      await invoke("terminate_process_group", { pgid: process.pgid, force });
      await wait(900);
      const next = await invoke<ScanResult>("scan_processes");
      setScan(next);

      const stillRunning = next.groups.some((group) =>
        group.processes.some((candidate) => candidate.pgid === process.pgid),
      );
      setForceCandidate(stillRunning && !force ? process.pgid : null);
      if (stillRunning && !force) {
        setError(`Process group ${process.pgid} is still running. Use force kill if needed.`);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setTerminating(null);
    }
  }

  function toggleGroup(project: string) {
    setExpandedGroups((previous) => {
      const next = new Set(previous);
      if (next.has(project)) next.delete(project);
      else next.add(project);
      return next;
    });
  }

  function toggleRow(pid: number) {
    setExpandedRows((previous) => {
      const next = new Set(previous);
      if (next.has(pid)) next.delete(pid);
      else next.add(pid);
      return next;
    });
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <h1>Process Console</h1>
          <div className="stats">
            <span>{processCount} processes</span>
            <span>{portCount} ports</span>
            <span>{scan ? formatScanTime(scan.scannedAtUnix) : "Not scanned"}</span>
          </div>
        </div>
        <div className="toolbar">
          <label className="search">
            <Search size={16} aria-hidden="true" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search project, port, agent, command"
            />
          </label>
          <button className="icon-button" onClick={() => void refresh()} title="Refresh">
            <RefreshCw size={17} className={loading ? "spin" : ""} />
          </button>
        </div>
      </header>

      {error ? (
        <div className="banner error">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </div>
      ) : null}

      {scan?.errors.length ? (
        <div className="banner warning">
          <AlertTriangle size={16} />
          <span>{scan.errors.length} process details could not be read</span>
        </div>
      ) : null}

      <section className="content">
        <div className="tabs" role="tablist" aria-label="Process categories">
          <button
            className={`tab ${activeTab === "agent" ? "active" : ""}`}
            onClick={() => setActiveTab("agent")}
            role="tab"
            aria-selected={activeTab === "agent"}
          >
            <span>Agent</span>
            <span className="tab-count">{tabCounts.agent}</span>
          </button>
          <button
            className={`tab ${activeTab === "other" ? "active" : ""}`}
            onClick={() => setActiveTab("other")}
            role="tab"
            aria-selected={activeTab === "other"}
          >
            <span>Others</span>
            <span className="tab-count">{tabCounts.other}</span>
          </button>
        </div>

        {!scan && loading ? <div className="empty">Scanning...</div> : null}
        {scan && filteredGroups.length === 0 ? (
          <div className="empty">
            {activeTab === "agent" ? "No agent-related listening processes" : "No other listening processes"}
          </div>
        ) : null}

        {filteredGroups.map((group) => {
          const expanded = expandedGroups.has(group.project);
          return (
            <section className="project-section" key={group.project}>
              <button className="project-header" onClick={() => toggleGroup(group.project)}>
                {expanded ? <ChevronDown size={18} /> : <ChevronRight size={18} />}
                <span className="project-name">{shortenHome(group.project)}</span>
                <span className="project-count">{group.processes.length}</span>
              </button>

              {expanded ? (
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Agent</th>
                        <th>Ports</th>
                        <th>Command</th>
                        <th>PID / PGID</th>
                        <th>Uptime</th>
                        <th aria-label="Actions" />
                      </tr>
                    </thead>
                    <tbody>
                      {group.processes.map((process) => {
                        const rowExpanded = expandedRows.has(process.pid);
                        const canForce = forceCandidate === process.pgid;
                        return (
                          <Fragment key={process.pid}>
                            <tr>
                              <td>
                                <div className="agent-cell">
                                  <span className="agent-name">
                                    {process.agent.name ?? "Unknown"}
                                  </span>
                                  <span className={`confidence ${process.agent.confidence.toLowerCase()}`}>
                                    {process.agent.confidence}
                                  </span>
                                </div>
                              </td>
                              <td>
                                <div className="ports">
                                  {process.ports.map((port) => (
                                    <span className="port" key={`${port.address}:${port.port}`}>
                                      {port.address}:{port.port}
                                    </span>
                                  ))}
                                </div>
                              </td>
                              <td>
                                <button
                                  className="command-button"
                                  onClick={() => toggleRow(process.pid)}
                                  title="Show details"
                                >
                                  {rowExpanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                                  <span>{process.command}</span>
                                </button>
                              </td>
                              <td className="mono">
                                {process.pid} / {process.pgid}
                              </td>
                              <td className="mono">{process.elapsed}</td>
                              <td>
                                <div className="row-actions">
                                  <button
                                    className="icon-button danger"
                                    onClick={() =>
                                      setPendingTermination({ process, force: canForce })
                                    }
                                    disabled={terminating === process.pgid}
                                    title={canForce ? "Force kill process group" : "Terminate process group"}
                                  >
                                    {canForce ? <Zap size={16} /> : <Square size={15} />}
                                  </button>
                                </div>
                              </td>
                            </tr>
                            {rowExpanded ? (
                              <tr className="details-row">
                                <td colSpan={6}>
                                  <div className="details">
                                    <div>
                                      <span className="detail-label">cwd</span>
                                      <code>{shortenHome(process.cwd ?? "Unknown")}</code>
                                    </div>
                                    <div>
                                      <span className="detail-label">args</span>
                                      <code>{process.argsMasked}</code>
                                    </div>
                                    <div>
                                      <span className="detail-label">evidence</span>
                                      <code>
                                        {process.agent.evidence.length
                                          ? process.agent.evidence.join(", ")
                                          : "No agent evidence"}
                                      </code>
                                    </div>
                                  </div>
                                </td>
                              </tr>
                            ) : null}
                          </Fragment>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              ) : null}
            </section>
          );
        })}
      </section>

      {pendingTermination ? (
        <div className="modal-backdrop" role="presentation">
          <section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
            <h2 id="confirm-title">
              {pendingTermination.force ? "Force kill process group" : "Terminate process group"}
            </h2>
            <div className="confirm-body">
              <div>
                <span className="detail-label">signal</span>
                <code>{pendingTermination.force ? "SIGKILL" : "SIGTERM"}</code>
              </div>
              <div>
                <span className="detail-label">pid / pgid</span>
                <code>
                  {pendingTermination.process.pid} / {pendingTermination.process.pgid}
                </code>
              </div>
              <div>
                <span className="detail-label">command</span>
                <code>{pendingTermination.process.argsMasked}</code>
              </div>
            </div>
            <div className="confirm-actions">
              <button className="text-button" onClick={() => setPendingTermination(null)}>
                Cancel
              </button>
              <button
                className="text-button danger"
                onClick={() =>
                  void terminate(pendingTermination.process, pendingTermination.force)
                }
              >
                {pendingTermination.force ? "Force Kill" : "Terminate"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </main>
  );
}

function filterGroups(groups: ProjectGroup[], activeTab: ActiveTab, query: string) {
  const trimmed = query.trim().toLowerCase();

  return groups
    .map((group) => ({
      ...group,
      processes: group.processes.filter((process) => {
        const inActiveTab =
          activeTab === "agent" ? isAgentProcess(process) : !isAgentProcess(process);
        if (!inActiveTab) return false;
        if (!trimmed) return true;

        return [
          group.project,
          process.command,
          process.argsMasked,
          process.cwd ?? "",
          process.agent.name ?? "",
          process.ports.map((port) => port.port).join(" "),
        ]
          .join(" ")
          .toLowerCase()
          .includes(trimmed);
      }),
    }))
    .filter((group) => group.processes.length > 0);
}

function isAgentProcess(process: ListeningProcess) {
  return process.agent.name !== null && process.agent.confidence !== "Unknown";
}

function wait(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function formatScanTime(unix: number) {
  return new Date(unix * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function shortenHome(path: string) {
  const home = "/Users/hmj";
  return path.startsWith(home) ? `~${path.slice(home.length)}` : path;
}
