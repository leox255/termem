import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Dot, Cursor } from "@/components/window";
import { Mark } from "@/components/brand";

/* ---------- shared primitives ---------- */
const sidebar =
  "w-[210px] shrink-0 overflow-hidden border-r border-[var(--line)] bg-gradient-to-b from-[#051726] to-[#03101b] p-3 flex flex-col gap-[7px]";
const sbHead = "mx-1.5 mt-2.5 mb-1 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-3";
const sbItem = "flex items-center gap-2.5 rounded-md px-2 py-1.5 font-mono text-xs text-ink-2";
const term =
  "relative overflow-hidden bg-[#02100c] px-4 py-3.5 font-mono text-xs leading-[1.7] text-green";

function TTab({ children }: { children: ReactNode }) {
  return (
    <div className="absolute inset-x-0 top-0 flex h-[26px] items-center border-b border-[rgba(71,255,156,0.1)] bg-[#04140f] px-3 text-[10.5px] tracking-[0.08em] text-[#2f8f6a]">
      {children}
    </div>
  );
}

function Skel({ className }: { className?: string }) {
  return <div className={cn("mb-[11px] h-[11px] rounded-md bg-gradient-to-r from-[#103022] to-[#15402d]", className)} />;
}

/* ============================================================ 01 WORKSPACE */
export function WorkspaceRender() {
  return (
    <>
      <aside className={sidebar}>
        <div className="flex items-center gap-2 px-1.5 pb-3 pt-1">
          <Mark className="h-[15px] w-[15px] text-green" />
          <span className="font-mono text-xs tracking-[0.06em] text-ink-2">even</span>
        </div>
        <div className={sbHead}>Workspace</div>
        <div className={cn(sbItem, "bg-green/10 text-ink")}>
          <span className="text-green">◧</span>Terminal
        </div>
        <div className={sbItem}><span className="text-ink-3">◐</span>Browser</div>
        <div className={sbItem}><span className="text-ink-3">▤</span>Editor</div>
        <div className={sbHead}>Agents</div>
        <div className={sbItem}><Dot tone="live" />agent-1 <small className="text-ink-3">refactor</small></div>
        <div className={sbItem}><Dot tone="idle" />agent-2 <small className="text-ink-3">tests</small></div>
        <div className={sbHead}>Changes</div>
        <div className={sbItem}><Dot tone="off" />main <small className="text-ink-3">↑2</small></div>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex min-h-0 flex-1">
          <div className={cn(term, "flex-1 border-r border-[var(--line)]")}>
            <TTab>zsh · orbit</TTab>
            <div className="mt-[30px]">
              <div><span className="text-ink-3">orbit</span> <span className="text-green">❯</span> <span className="text-ink-2">cargo run --release</span></div>
              <div className="text-ink-3">   Compiling orbit v0.4.0</div>
              <div className="text-cyan">    Finished release [optimized]</div>
              <div className="text-ink-3">     Running `target/release/orbit`</div>
              <div><span className="text-amber">▸</span> listening on :7070</div>
              <div><span className="text-green">❯</span> <Cursor /></div>
            </div>
          </div>
          <div className="flex min-w-0 flex-1 flex-col bg-[#071a13]">
            <div className="flex gap-1.5 overflow-hidden bg-[#03130c] px-2.5 pt-2">
              <div className="flex max-w-[160px] items-center gap-1.5 overflow-hidden rounded-t-lg bg-[#0c2a1c] px-3 py-1.5 font-mono text-[11px] text-ink">
                <span className="h-[13px] w-[13px] rounded-[3px] bg-[#ff6154]" />localhost:7070
              </div>
              <div className="flex max-w-[160px] items-center gap-1.5 overflow-hidden rounded-t-lg px-3 py-1.5 font-mono text-[11px] text-ink-3">
                <span className="h-[13px] w-[13px] rounded-[3px] bg-[#f7df1e]" />MDN
              </div>
            </div>
            <div className="flex items-center gap-2 border-b border-[var(--line)] bg-[#0c2a1c] px-2.5 py-2">
              <div className="flex flex-1 items-center gap-2 rounded-lg border border-[var(--line)] bg-[#02110b] px-3 py-1.5 font-mono text-[11.5px] text-ink-2">
                <span className="text-[10px] text-green">●</span>localhost:7070
              </div>
            </div>
            <div className="flex-1 overflow-hidden bg-gradient-to-b from-[#0a2419] to-[#061811] px-5 py-[18px]">
              <Skel className="h-5 w-[55%] bg-gradient-to-r from-[#1a5238] to-[#226b49]" />
              <Skel className="w-[88%]" />
              <Skel className="w-[72%]" />
              <div className="mt-[18px] flex gap-3">
                <div className="h-[74px] flex-1 rounded-[10px] border border-[var(--line)] bg-gradient-to-br from-[#0f3122] to-[#0a2417]" />
                <div className="h-[74px] flex-1 rounded-[10px] border border-[var(--line)] bg-gradient-to-br from-[#0f3122] to-[#0a2417]" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

/* ============================================================ 02 TERMINAL (4 panes) */
function Pane({ tab, children, last }: { tab: string; children: ReactNode; last?: boolean }) {
  return (
    <div className={cn(term, "flex-1 min-w-0", !last && "border-r border-[var(--line)]")}>
      <TTab>{tab}</TTab>
      <div className="mt-[30px]">{children}</div>
    </div>
  );
}
export function TerminalRender() {
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1">
        <Pane tab="build">
          <div><span className="text-green">❯</span> <span className="text-ink-2">pnpm build</span></div>
          <div className="text-ink-3">vite v5.2 building…</div>
          <div className="text-cyan">✓ 412 modules transformed</div>
          <div><span className="text-amber">dist/</span> 184 kB</div>
          <div><span className="text-green">❯</span> <Cursor /></div>
        </Pane>
        <Pane tab="logs" last>
          <div className="text-ink-3">12:04:51 GET /api/orbit 200</div>
          <div className="text-ink-3">12:04:51 GET /assets 200</div>
          <div className="text-amber">12:04:52 WARN slow query 240ms</div>
          <div className="text-ink-3">12:04:53 GET /health 200</div>
          <div><span className="text-green">❯</span> <Cursor /></div>
        </Pane>
      </div>
      <div className="flex min-h-0 flex-1 border-t border-[var(--line)]">
        <Pane tab="git">
          <div><span className="text-green">❯</span> <span className="text-ink-2">git status -sb</span></div>
          <div className="text-cyan">## main...origin/main [ahead 2]</div>
          <div className="text-amber"> M src/orbit.rs</div>
          <div className="text-cyan">?? notes.md</div>
        </Pane>
        <Pane tab="watch" last>
          <div><span className="text-green">❯</span> <span className="text-ink-2">cargo watch -x test</span></div>
          <div className="text-cyan">running 24 tests</div>
          <div className="text-cyan">test result: ok. 24 passed</div>
          <div><span className="text-green">❯</span> <Cursor /></div>
        </Pane>
      </div>
    </div>
  );
}

/* ============================================================ 03 BROWSER */
function Tab({ color, label, active }: { color: string; label: string; active?: boolean }) {
  return (
    <div
      className={cn(
        "flex max-w-[170px] items-center gap-1.5 overflow-hidden rounded-t-lg px-3 py-1.5 font-mono text-[11px] whitespace-nowrap",
        active ? "bg-[#0c2a1c] text-ink" : "text-ink-3"
      )}
    >
      <span className="h-[13px] w-[13px] shrink-0 rounded-[3px]" style={{ background: color }} />
      {label}
    </div>
  );
}
export function BrowserRender() {
  return (
    <div className="flex min-w-0 flex-1 flex-col bg-[#071a13]">
      <div className="flex gap-1.5 overflow-hidden bg-[#03130c] px-2.5 pt-2">
        <Tab color="linear-gradient(135deg,#ff8a00,#ff4d4d)" label="Hacker News" active />
        <Tab color="#5e6ad2" label="Linear" />
        <Tab color="#635bff" label="Stripe Docs" />
        <Tab color="#1da1f2" label="+" />
      </div>
      <div className="flex items-center gap-2 border-b border-[var(--line)] bg-[#0c2a1c] px-2.5 py-2">
        <div className="flex gap-1.5 text-ink-3">
          <svg viewBox="0 0 16 16" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M10 3L5 8l5 5" /></svg>
          <svg viewBox="0 0 16 16" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M6 3l5 5-5 5" /></svg>
          <svg viewBox="0 0 16 16" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M3 8a5 5 0 105-5 5 5 0 00-4 2M3 3v3h3" /></svg>
        </div>
        <div className="flex flex-1 items-center gap-2 rounded-lg border border-[var(--line)] bg-[#02110b] px-3 py-1.5 font-mono text-[11.5px] text-ink-2">
          <span className="text-[10px] text-green">🔒</span>news.ycombinator.com
        </div>
        <svg viewBox="0 0 16 16" className="h-3.5 w-3.5 text-ink-3" fill="none" stroke="currentColor" strokeWidth="1.5"><circle cx="8" cy="8" r="6" /><path d="M8 5v3l2 1" /></svg>
      </div>
      <div className="flex-1 overflow-hidden bg-gradient-to-b from-[#0a2419] to-[#061811] px-5 py-[18px]">
        <Skel className="h-5 w-[55%] bg-gradient-to-r from-[#1a5238] to-[#226b49]" />
        <Skel className="w-[88%]" />
        <Skel className="w-[80%]" />
        <Skel className="w-[72%]" />
        <div className="mt-[18px] flex gap-3">
          <div className="h-[74px] flex-1 rounded-[10px] border border-[var(--line)] bg-gradient-to-br from-[#0f3122] to-[#0a2417]" />
          <div className="h-[74px] flex-1 rounded-[10px] border border-[var(--line)] bg-gradient-to-br from-[#0f3122] to-[#0a2417]" />
          <div className="h-[74px] flex-1 rounded-[10px] border border-[var(--line)] bg-gradient-to-br from-[#0f3122] to-[#0a2417]" />
        </div>
      </div>
    </div>
  );
}

/* ============================================================ 04 AGENTS */
function ARow({ tone, name, task, branch }: { tone: "live" | "idle" | "stuck" | "off"; name: string; task: string; branch: string }) {
  return (
    <div className="flex items-center gap-2.5 rounded-[10px] border border-[var(--line)] bg-[#08231a] px-3 py-2.5">
      <Dot tone={tone} />
      <span className="flex-1 font-mono text-[12.5px] text-ink">{name} <small className="font-normal text-ink-3">{task}</small></span>
      <span className="rounded-[5px] bg-green/[0.08] px-[7px] py-[3px] font-mono text-[10px] text-[#2f8f6a]">{branch}</span>
    </div>
  );
}
export function AgentsRender() {
  return (
    <>
      <aside className={cn(sidebar, "w-[248px]")}>
        <div className={sbHead}>Running agents</div>
        <div className="flex flex-col gap-2">
          <ARow tone="live" name="agent-1" task="refactor" branch="even/a1" />
          <ARow tone="idle" name="agent-2" task="add tests" branch="even/a2" />
          <ARow tone="stuck" name="agent-3" task="migrate db" branch="even/a3" />
          <ARow tone="off" name="agent-4" task="docs" branch="done" />
        </div>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className={cn(term, "flex-1")}>
          <TTab>agent-1 · claude · even/a1</TTab>
          <div className="mt-[30px]">
            <div className="text-ink-3">▸ briefed from board and recall</div>
            <div><span className="text-amber">●</span> reading src/orbit.rs</div>
            <div><span className="text-amber">●</span> editing 3 files</div>
            <div className="text-cyan">  ✓ extracted Scheduler trait</div>
            <div className="text-cyan">  ✓ updated 11 call-sites</div>
            <div><span className="text-amber">●</span> running cargo test…</div>
            <div className="text-cyan">  test result: ok. 24 passed</div>
            <div><span className="text-green">❯</span> posting done to board <Cursor /></div>
          </div>
        </div>
      </div>
    </>
  );
}

/* ============================================================ 05 GIT */
function GFile({ tg, tone, name, on }: { tg: string; tone: string; name: string; on?: boolean }) {
  return (
    <div className={cn("flex items-center gap-2.5 rounded-md px-[7px] py-[5px] font-mono text-[11.5px] text-ink-2", on && "bg-green/[0.07] text-ink")}>
      <span className={cn("w-3.5 text-center font-bold", tone)}>{tg}</span>
      {name}
    </div>
  );
}
function DL({ kind, n, code }: { kind: "add" | "del" | "ctx"; n: string; code: string }) {
  const bg = kind === "add" ? "bg-green/[0.08]" : kind === "del" ? "bg-red/[0.08]" : "";
  const tc = kind === "add" ? "text-green" : kind === "del" ? "text-red" : "text-ink-3";
  return (
    <div className={cn("flex py-px", bg)}>
      <span className="w-10 shrink-0 pr-3 text-right text-ink-3 opacity-70">{n}</span>
      <span className={cn("flex-1 whitespace-pre pl-3", tc)}>{code}</span>
    </div>
  );
}
export function GitRender() {
  return (
    <div className="flex min-w-0 flex-1 bg-[#03130c]">
      <div className="flex w-[230px] shrink-0 flex-col gap-1.5 overflow-hidden border-r border-[var(--line)] p-3.5">
        <div className="mb-1.5 flex items-center gap-2 px-1 py-1.5 font-mono text-[11.5px] text-ink-2">
          <svg viewBox="0 0 16 16" className="h-[13px] w-[13px]" fill="none" stroke="currentColor" strokeWidth="1.4"><circle cx="4" cy="4" r="2" /><circle cx="4" cy="12" r="2" /><circle cx="12" cy="6" r="2" /><path d="M4 6v4M12 8c0 2-2 2-4 2" /></svg>
          even/agent-1<span className="ml-auto text-green">↑3</span>
        </div>
        <div className={sbHead.replace("mx-1.5", "mx-1")}>Staged</div>
        <GFile tg="+" tone="text-green" name="src/scheduler.rs" on />
        <GFile tg="~" tone="text-amber" name="src/orbit.rs" />
        <div className={sbHead.replace("mx-1.5", "mx-1")}>Changes</div>
        <GFile tg="~" tone="text-amber" name="src/main.rs" />
        <GFile tg="+" tone="text-green" name="tests/sched.rs" />
        <GFile tg="−" tone="text-red" name="src/old.rs" />
      </div>
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-[#02100b] font-mono text-[11.5px]">
        <div className="flex-1">
          <DL kind="ctx" n="12" code="  pub struct Orbit {" />
          <DL kind="del" n="13" code="-     tasks: Vec<Task>," />
          <DL kind="add" n="13" code="+     scheduler: Scheduler," />
          <DL kind="add" n="14" code="+     tasks: Vec<Task>," />
          <DL kind="ctx" n="15" code="  }" />
          <DL kind="ctx" n="16" code="  " />
          <DL kind="ctx" n="17" code="  impl Orbit {" />
          <DL kind="del" n="18" code="-     fn tick(&mut self) {" />
          <DL kind="add" n="18" code="+     fn tick(&mut self) -> Status {" />
          <DL kind="add" n="19" code="+         self.scheduler.advance()" />
          <DL kind="ctx" n="20" code="      }" />
        </div>
        <div className="border-t border-[var(--line)] bg-[#03130c] px-3.5 py-[11px]">
          <div className="rounded-md border border-[var(--line)] bg-[#02100b] px-[11px] py-2 font-mono text-[11.5px] text-ink-2">
            refactor: extract Scheduler trait from Orbit
          </div>
          <div className="mt-2 flex gap-2">
            <button className="rounded-md bg-green px-[13px] py-1.5 font-mono text-[11px] font-bold text-[#021]">Commit</button>
            <button className="rounded-md border border-[var(--line)] px-[13px] py-1.5 font-mono text-[11px] text-ink-2">Commit and Push</button>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ============================================================ 06 GATED */
export function GatedRender() {
  return (
    <div className="relative flex min-w-0 flex-1">
      <div className={cn(term, "flex-1")}>
        <TTab>agent-3 · migrate db</TTab>
        <div className="mt-[30px]">
          <div className="text-ink-3">▸ worktree: ../even-a3 (branch even/a3)</div>
          <div><span className="text-amber">●</span> wrote migrations/008_orbit.sql</div>
          <div className="text-cyan">  ✓ schema validated</div>
          <div><span className="text-amber">●</span> wants to run:</div>
          <div className="text-amber">  $ git push origin even/a3</div>
          <div className="text-ink-3">  ⏸ paused, awaiting approval</div>
          <div><span className="text-green">❯</span> <Cursor /></div>
        </div>
      </div>
      <div className="gate-in absolute bottom-[18px] right-[18px] z-[5] w-[300px] rounded-[13px] border border-[rgba(255,194,102,0.4)] bg-gradient-to-b from-[#0a2a1d] to-[#072015] p-4 shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9),0_0_50px_-16px_rgba(255,194,102,0.4)]">
        <div className="mb-2.5 flex items-center gap-2.5 font-mono text-[11px] uppercase tracking-[0.12em] text-amber">
          <svg viewBox="0 0 16 16" className="h-[13px] w-[13px]" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M8 1l6 3v4c0 4-3 6-6 7-3-1-6-3-6-7V4z" /></svg>
          Approval needed
        </div>
        <div className="mb-1.5 rounded-md border border-[var(--line)] bg-[#02100b] px-[11px] py-2.5 font-mono text-xs text-ink">
          <span className="text-ink-3">$</span> git push origin even/a3
        </div>
        <div className="mb-3 font-mono text-[10.5px] text-ink-3">agent-3 · risky: pushes to remote</div>
        <div className="flex gap-2.5">
          <button className="flex-1 rounded-md bg-green py-2 font-mono text-[11.5px] font-bold text-[#021]">Approve</button>
          <button className="flex-1 rounded-md border border-[var(--line)] py-2 font-mono text-[11.5px] text-ink-2">Deny</button>
        </div>
      </div>
    </div>
  );
}

/* ============================================================ 07 RECALL */
function Ses({ tone, title, tag, tagTone, children, unfinished }: { tone: "live" | "off"; title: string; tag: string; tagTone: string; children: ReactNode; unfinished?: string }) {
  return (
    <div className="rounded-[10px] border border-[var(--line)] bg-[#08231a] px-3.5 py-3">
      <div className="mb-1.5 flex items-center gap-2.5">
        <Dot tone={tone} />
        <span className="font-mono text-xs text-ink">{title}</span>
        <span className={cn("ml-auto rounded-[5px] px-[7px] py-0.5 font-mono text-[9.5px] uppercase tracking-[0.1em]", tagTone)}>{tag}</span>
      </div>
      <div className="text-[12.5px] leading-[1.5] text-ink-2">{children}</div>
      {unfinished && <div className="mt-1.5 font-mono text-[11px] text-amber">{unfinished}</div>}
    </div>
  );
}
export function RecallRender() {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-2.5 overflow-hidden bg-[#03130c] p-4">
      <div className="mb-0.5 flex items-center gap-2 font-mono text-[11px] tracking-[0.1em] text-green">
        <svg viewBox="0 0 16 16" className="h-[13px] w-[13px]" fill="none" stroke="currentColor" strokeWidth="1.4"><circle cx="8" cy="8" r="6" /><path d="M8 4v4l3 2" /></svg>
        3 sessions in this directory
      </div>
      <Ses tone="live" title="refactor scheduler" tag="cached" tagTone="bg-green/[0.12] text-green">
        Extracted the Scheduler trait, updated 11 call-sites, all tests green.
      </Ses>
      <Ses tone="off" title="db migration" tag="stale" tagTone="bg-amber/[0.12] text-amber" unfinished="unfinished: never pushed, awaiting approval.">
        Wrote migration 008; schema validated.
      </Ses>
      <Ses tone="off" title="browser favicons" tag="cached" tagTone="bg-green/[0.12] text-green">
        Backend favicon resolver, fetches the site itself, caches to disk.
      </Ses>
    </div>
  );
}
