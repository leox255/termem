import { useEffect, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Mark, Wordmark } from "@/components/brand";
import { Window } from "@/components/window";
import {
  WorkspaceRender,
  TerminalRender,
  BrowserRender,
  AgentsRender,
  GitRender,
  GatedRender,
  RecallRender,
} from "@/components/renders";
import { cn } from "@/lib/utils";
import { useDownloads } from "@/hooks/useDownloads";

/* ============================================================ scroll deck hook
 * Reveals content as a panel starts entering, and dims a covered panel via a
 * child overlay. It never transforms or filters a sticky element, which is what
 * makes the stacked-panel effect flicker / go dark in WebKit (Safari).
 */
function useDeck() {
  useEffect(() => {
    const panels = Array.from(document.querySelectorAll<HTMLElement>("[data-panel]"));
    const contents = panels.map((p) => p.querySelector<HTMLElement>("[data-content]"));
    const nav = document.getElementById("topnav");
    const cue = document.getElementById("cue");
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const revealed = new Array(panels.length).fill(false);
    const ease = (t: number) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);
    const clamp = (v: number, a: number, b: number) => (v < a ? a : v > b ? b : v);

    const reveal = (i: number) => {
      if (!revealed[i]) {
        revealed[i] = true;
        panels[i].setAttribute("data-in", "");
      }
    };
    const vh0 = window.innerHeight;
    panels.forEach((p, i) => {
      if (p.getBoundingClientRect().top < vh0 * 0.95) reveal(i);
    });
    // failsafe: no panel may ever stay hidden, even if the rAF loop is interrupted.
    const failsafe = window.setTimeout(() => panels.forEach((_, i) => reveal(i)), 1600);

    let raf = 0;
    const frame = () => {
      const vh = window.innerHeight;
      const y = window.scrollY;
      for (let i = 0; i < panels.length; i++) {
        if (!revealed[i] && panels[i].getBoundingClientRect().top < vh * 0.9) reveal(i);
      }
      if (!reduce) {
        // Fade a panel's own content as the next panel rises to cover it.
        // Opacity on the (non-sticky) content element only — no overlay layer,
        // no transform/filter on the sticky element. The active panel and the
        // last panel always stay at full opacity, so none can read as "missing".
        for (let j = 0; j < panels.length; j++) {
          const next = panels[j + 1];
          const p = next ? ease(clamp(1 - next.getBoundingClientRect().top / vh, 0, 1)) : 0;
          const c = contents[j];
          if (c) c.style.opacity = (1 - 0.6 * p).toFixed(3);
        }
      }
      if (nav) nav.style.opacity = y > vh * 0.5 ? "1" : "0";
      if (cue) cue.style.opacity = y > 40 ? "0" : "1";
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    const onKey = (e: KeyboardEvent) => {
      const cur = Math.round(window.scrollY / window.innerHeight);
      const go = (i: number) =>
        window.scrollTo({ top: clamp(i, 0, panels.length - 1) * window.innerHeight, behavior: reduce ? "auto" : "smooth" });
      if (e.key === "ArrowDown" || e.key === "PageDown" || e.key === " ") { e.preventDefault(); go(cur + 1); }
      else if (e.key === "ArrowUp" || e.key === "PageUp") { e.preventDefault(); go(cur - 1); }
      else if (e.key === "Home") { e.preventDefault(); go(0); }
      else if (e.key === "End") { e.preventDefault(); go(panels.length - 1); }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(failsafe);
      window.removeEventListener("keydown", onKey);
    };
  }, []);
}

/* ============================================================ primitives */
const PANEL_BG =
  "radial-gradient(120% 80% at 50% -10%, rgba(71,255,156,0.05), transparent 60%), linear-gradient(180deg, var(--bg-2), var(--background) 60%, var(--void))";

function Panel({ id, first, children }: { id: string; first?: boolean; children: ReactNode }) {
  return (
    <section
      data-panel
      id={id}
      className={cn(
        "isolate sticky top-0 flex h-[100svh] min-h-[640px] items-center justify-center overflow-hidden",
        "px-[clamp(18px,4vw,42px)] py-[clamp(74px,9vh,96px)]",
        first ? "rounded-none" : "rounded-t-[22px]"
      )}
      style={{
        background: PANEL_BG,
        boxShadow: first ? undefined : "0 -1px 0 var(--line) inset, 0 -36px 80px -20px rgba(0,0,0,0.85)",
      }}
    >
      {!first && <span className="panel-lip" />}
      <div data-content className="relative z-10 mx-auto w-full max-w-[1180px]">
        {children}
      </div>
    </section>
  );
}

function Eyebrow({ children, center }: { children: ReactNode; center?: boolean }) {
  return (
    <div
      className={cn(
        "mb-5 font-sans text-[12.5px] font-semibold uppercase tracking-[0.22em] text-green",
        center && "text-center"
      )}
    >
      {children}
    </div>
  );
}

function Display({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <h2 className={cn("font-sans text-[clamp(2.1rem,4.7vw,4.05rem)] font-bold leading-[1.08] tracking-[-0.015em] text-ink", className)}>
      {children}
    </h2>
  );
}

function Lede({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <p className={cn("mt-[22px] max-w-[50ch] font-sans text-[clamp(1.05rem,1.5vw,1.3rem)] leading-[1.6] text-ink-2", className)}>
      {children}
    </p>
  );
}

function Reveal({ children, d = 0, className }: { children: ReactNode; d?: number; className?: string }) {
  return (
    <div className={cn("reveal", className)} style={{ ["--d" as string]: d }}>
      {children}
    </div>
  );
}

function Feat({ icon, title, desc }: { icon: ReactNode; title: string; desc: string }) {
  return (
    <div className="flex items-start gap-3.5">
      <span className="grid h-[30px] w-[30px] shrink-0 place-items-center rounded-lg border border-[var(--line)] bg-green/10 text-green">
        {icon}
      </span>
      <div>
        <div className="mb-0.5 font-sans text-sm font-semibold text-ink">{title}</div>
        <div className="font-sans text-[13.5px] leading-[1.45] text-ink-3">{desc}</div>
      </div>
    </div>
  );
}

function Split({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("grid items-center gap-[clamp(26px,4vw,56px)] md:grid-cols-2", className)}>{children}</div>;
}

/* small inline icons */
const I = {
  split: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="2" y="2" width="5" height="12" rx="1" /><rect x="9" y="2" width="5" height="12" rx="1" /></svg>,
  tabs: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M2 5h12M2 5v7a1 1 0 001 1h10a1 1 0 001-1V5M5 5V3h6v2" /></svg>,
  globe: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><circle cx="8" cy="8" r="6" /><path d="M2 8h12" /></svg>,
  clock: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><circle cx="8" cy="8" r="6" /><path d="M8 5v3l2 1" /></svg>,
  box: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="2" y="2" width="12" height="12" rx="2" /><path d="M2 6h12M6 6v8" /></svg>,
  shield: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M8 1l6 3v4c0 4-3 6-6 7-3-1-6-3-6-7V4z" /><path d="M6 8l1.5 1.5L10.5 6.5" /></svg>,
  grid: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M2 4h12v9H2z" /><path d="M2 7h12" /></svg>,
  back: <svg viewBox="0 0 16 16" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M3 8a5 5 0 105-5 5 5 0 00-4 2M3 3v3h3" /></svg>,
};

/* ============================================================ app */
export default function App() {
  useDeck();
  const dl = useDownloads();
  return (
    <>
      {/* atmosphere */}
      <div className="pointer-events-none fixed inset-0 z-0">
        <div className="atmos-grid absolute -inset-0.5" />
        <div className="atmos-glow absolute left-1/2 top-[34%] h-[1100px] w-[1100px] -translate-x-1/2 -translate-y-1/2" />
        <div className="absolute inset-0" style={{ background: "radial-gradient(130% 110% at 50% 40%, transparent 55%, rgba(1,8,16,0.85) 100%)" }} />
        <div className="atmos-grain absolute inset-0" />
      </div>

      {/* top nav */}
      <header
        id="topnav"
        className="pointer-events-none fixed inset-x-0 top-0 z-[60] flex items-center justify-between px-[clamp(18px,4vw,42px)] py-5 opacity-0 transition-opacity duration-500"
      >
        <a href="#p0" className="pointer-events-auto flex items-center gap-[11px]">
          <Mark className="h-[18px] w-[18px] text-green drop-shadow-[0_0_10px_rgba(71,255,156,0.4)]" />
          <Wordmark className="text-[19px] drop-shadow-[0_0_10px_rgba(71,255,156,0.35)]" />
        </a>
        <div className="pointer-events-auto flex items-center gap-[18px]">
          <Button asChild size="pill" className="shadow-[0_0_0_1px_rgba(71,255,156,0.5),0_8px_30px_-8px_rgba(71,255,156,0.6)]">
            <a href={dl.urls[dl.primary]} target="_blank" rel="noopener noreferrer">Download</a>
          </Button>
        </div>
      </header>

      {/* scroll cue: fixed to the viewport (not the hero content) so it sits at
          the bottom edge and never overlaps the description. Fades out on scroll. */}
      <div
        id="cue"
        className="pointer-events-none fixed bottom-8 left-1/2 z-50 flex -translate-x-1/2 flex-col items-center gap-2.5 font-sans text-[11px] font-medium uppercase tracking-[0.24em] text-ink-3 transition-opacity duration-500"
      >
        Scroll
        <span className="scroll-dot h-[5px] w-[5px] rounded-full bg-green shadow-[0_0_12px_var(--green)]" />
      </div>

      <main className="relative z-[1]">
        {/* 00 HERO */}
        <Panel id="p0" first>
          <div className="flex flex-col items-center text-center">
            <Reveal d={0}>
              <Mark animate className="mb-[30px] w-[clamp(96px,15vw,168px)] text-green drop-shadow-[0_0_40px_rgba(71,255,156,0.45)]" />
            </Reveal>
            <Reveal d={260}>
              <Wordmark className="text-[clamp(4.5rem,13vw,11rem)] drop-shadow-[0_0_30px_rgba(71,255,156,0.3)]" />
            </Reveal>
            <Reveal d={520}>
              <h1 className="mt-[34px] max-w-[32ch] font-sans text-[clamp(1.2rem,2.3vw,1.75rem)] font-medium leading-[1.34] tracking-[-0.005em] text-ink-2">
                Your terminal, browser, editor, and coding agents{" "}
                <span className="text-ink">in one window.</span>
              </h1>
            </Reveal>
            <Reveal d={760}>
              <div className="mt-6 font-sans text-[12.5px] font-medium uppercase tracking-[0.18em] text-ink-3">
                A terminal-first workspace
              </div>
            </Reveal>
          </div>
        </Panel>

        {/* 01 ONE WORKSPACE */}
        <Panel id="p1">
          <div className="flex flex-col items-center gap-[clamp(14px,2.2vw,30px)]">
            <Reveal d={0} className="text-center">
              <Eyebrow center>Workspace</Eyebrow>
              <Display>Everything in one window.</Display>
            </Reveal>
            <Reveal d={160} className="w-full">
              <Window title={<><b className="font-medium text-ink-2">~/work/orbit</b> · even</>}>
                <WorkspaceRender />
              </Window>
            </Reveal>
            <Reveal d={340}>
              <Lede className="max-w-[56ch] text-center">
                Terminal, browser, editor, and coding agents share one native window, so you stop alt-tabbing between five apps to make one change.
              </Lede>
            </Reveal>
          </div>
        </Panel>

        {/* 02 TERMINAL */}
        <Panel id="p2">
          <Split>
            <Reveal d={0}>
              <Eyebrow>Terminal</Eyebrow>
              <Display>Split it however you work.</Display>
              <Lede>Tile and tab panes any way the task needs. A real shell underneath, fast and yours to arrange.</Lede>
              <div className="mt-7 flex flex-col gap-[13px]">
                <Feat icon={I.split} title="Split and tile" desc="Drag any pane to split horizontally or vertically." />
                <Feat icon={I.tabs} title="Tabs per pane" desc="Every pane keeps its own stack of tabs." />
              </div>
            </Reveal>
            <Reveal d={200}>
              <Window title={<><b className="font-medium text-ink-2">terminal</b> · 4 panes</>}>
                <TerminalRender />
              </Window>
            </Reveal>
          </Split>
        </Panel>

        {/* 03 BROWSER */}
        <Panel id="p3">
          <Split>
            <Reveal d={200}>
              <Window title={<b className="font-medium text-ink-2">browser</b>}>
                <BrowserRender />
              </Window>
            </Reveal>
            <Reveal d={0}>
              <Eyebrow>Browser</Eyebrow>
              <Display>A real browser, built in.</Display>
              <Lede>A native browser pane with tabs, history, and bookmarks. Keep docs and dashboards next to the code that uses them.</Lede>
              <div className="mt-7 flex flex-col gap-[13px]">
                <Feat icon={I.globe} title="Native rendering" desc="A real browser engine, not an iframe." />
                <Feat icon={I.clock} title="Tabs and history" desc="The browsing you already rely on, in a pane." />
              </div>
            </Reveal>
          </Split>
        </Panel>

        {/* 04 AGENTS */}
        <Panel id="p4">
          <div className="flex flex-col items-center gap-[clamp(14px,2.2vw,30px)]">
            <Reveal d={0} className="max-w-[62ch] text-center">
              <Eyebrow center>Agents</Eyebrow>
              <Display>Run agents where you work.</Display>
              <Lede className="mx-auto max-w-[58ch] text-center">
                Launch a coding agent into its own pane with the right context. See which are running, idle, or stuck at a glance.
              </Lede>
            </Reveal>
            <Reveal d={200} className="w-full">
              <Window title={<><b className="font-medium text-ink-2">agents</b> · running</>}>
                <AgentsRender />
              </Window>
            </Reveal>
          </div>
        </Panel>

        {/* 05 GIT */}
        <Panel id="p5">
          <div className="flex flex-col items-center gap-[clamp(14px,2vw,30px)] text-center">
            <Reveal d={0} className="max-w-[60ch]">
              <Eyebrow center>Source control</Eyebrow>
              <Display>Review, stage, and commit in place.</Display>
              <Lede className="mx-auto mt-4 max-w-[64ch] text-center">
                Every change and diff in one panel, including each agent's branch. Stage, commit, and push without leaving Even.
              </Lede>
            </Reveal>
            <Reveal d={200} className="w-full">
              <Window title={<><b className="font-medium text-ink-2">git</b> · even/a1 ↑3</>}>
                <GitRender />
              </Window>
            </Reveal>
          </div>
        </Panel>

        {/* 06 ISOLATED & GATED */}
        <Panel id="p6">
          <Split>
            <Reveal d={200}>
              <Window title={<><b className="font-medium text-ink-2">agent-3</b> · even/a3 · sandboxed</>}>
                <GatedRender />
              </Window>
            </Reveal>
            <Reveal d={0}>
              <Eyebrow>Sandboxing</Eyebrow>
              <Display>Agents that stay sandboxed.</Display>
              <Lede>Each agent works in its own git worktree, so it never touches your main branch. Risky commands wait for your approval before they run.</Lede>
              <div className="mt-7 flex flex-col gap-[13px]">
                <Feat icon={I.box} title="Worktree isolation" desc="One branch per agent, off your tree." />
                <Feat icon={I.shield} title="Approval gates" desc="Pushes, deletes, and deploys pause for you." />
              </div>
            </Reveal>
          </Split>
        </Panel>

        {/* 07 SHARED MEMORY */}
        <Panel id="p7">
          <Split>
            <Reveal d={200}>
              <Window title={<><b className="font-medium text-ink-2">termem</b> recall · ~/work/orbit</>}>
                <RecallRender />
              </Window>
            </Reveal>
            <Reveal d={0}>
              <Eyebrow>Memory</Eyebrow>
              <Display>Picks up where you left off.</Display>
              <Lede>
                Even runs on <b className="font-semibold text-green">termem</b>, a shared index of every terminal session across your tools. Open a folder and continue where you, or an agent, stopped.
              </Lede>
              <div className="mt-7 flex flex-col gap-[13px]">
                <Feat icon={I.grid} title="Cross-tool" desc="Claude, Codex, and your shell, indexed together." />
                <Feat icon={I.back} title="Resume anything" desc="Reopen any past session in place." />
              </div>
            </Reveal>
          </Split>
        </Panel>

        {/* 08 CTA */}
        <Panel id="p8">
          <div className="flex flex-col items-center text-center" id="download">
            <Reveal d={0}>
              <Mark animate className="mb-[26px] w-[clamp(70px,9vw,108px)] text-green drop-shadow-[0_0_34px_rgba(71,255,156,0.5)]" />
            </Reveal>
            <Reveal d={160}>
              <Eyebrow center>Now in beta</Eyebrow>
            </Reveal>
            <Reveal d={280}>
              <Display className="mb-3.5">Make it Even.</Display>
            </Reveal>
            <Reveal d={400}>
              <Lede className="mx-auto max-w-[48ch] text-center">
                One window for your terminal, browser, editor, and agents. Free while in beta.
              </Lede>
            </Reveal>
            <Reveal d={520}>
              <div className="mt-[30px] flex flex-col items-center gap-4">
                <Button asChild size="lg" className="shadow-[0_0_0_1px_rgba(71,255,156,0.5),0_12px_40px_-10px_rgba(71,255,156,0.6)]">
                  <a href={dl.urls[dl.primary]} target="_blank" rel="noopener noreferrer">Download</a>
                </Button>
                <div className="flex flex-wrap items-center justify-center gap-x-3.5 gap-y-1 font-mono text-[12px] text-ink-3">
                  <a href={dl.urls.mac} target="_blank" rel="noopener noreferrer" className="hover:text-green">macOS</a>
                  <span className="opacity-40">·</span>
                  <a href={dl.urls.win} target="_blank" rel="noopener noreferrer" className="hover:text-green">Windows</a>
                  <span className="opacity-40">·</span>
                  <a href={dl.urls.deb} target="_blank" rel="noopener noreferrer" className="hover:text-green">Linux .deb</a>
                  <span className="opacity-40">·</span>
                  <a href={dl.urls.appimage} target="_blank" rel="noopener noreferrer" className="hover:text-green">Linux .AppImage</a>
                  <span className="opacity-40">·</span>
                  <a href={dl.page} target="_blank" rel="noopener noreferrer" className="hover:text-green">{dl.version}</a>
                </div>
              </div>
            </Reveal>
          </div>
          <div className="absolute inset-x-0 bottom-0 flex flex-wrap items-center justify-between gap-2.5 border-t border-[var(--line-2)] px-[clamp(18px,4vw,42px)] py-6 font-sans text-xs text-ink-3">
            <div className="flex items-center gap-2.5">
              <Mark className="h-3.5 w-3.5 text-green-dim" />© 2026 Even
            </div>
            <div className="flex gap-5">
              <a href="#p0" className="hover:text-green">Top</a>
              <a href={dl.urls[dl.primary]} target="_blank" rel="noopener noreferrer" className="hover:text-green">Download</a>
            </div>
          </div>
        </Panel>
      </main>
    </>
  );
}
