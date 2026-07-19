import type { ReactNode } from "react";
import {
  TerminalWindow,
  GlobeHemisphereWest,
  Robot,
  GitBranch,
  ShieldCheck,
  Brain,
  ArrowRight,
  Plus,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Logo, Mark } from "@/components/brand";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Window } from "@/components/live/Window";
import { Terminal, type TLine } from "@/components/live/Terminal";
import { CodeBlock } from "@/components/live/CodeBlock";
import { Reveal, RevealGroup, RevealItem } from "@/components/motion/Reveal";
import { useDownloads } from "@/hooks/useDownloads";
import { cn } from "@/lib/utils";

/* ============================================================ content ====== */

const HERO_TERM: TLine[] = [
  { kind: "cmd", text: "even ~/work/orbit" },
  { kind: "sys", node: "workspace ready: terminal, browser, editor, agents" },
  { kind: "cmd", text: 'even run "add rate limiting to the API"' },
  { kind: "out", node: <span className="text-t-ink-3">agent briefed from memory, working on even/agent-1</span> },
  { kind: "out", node: (<><span className="text-t-amber">●</span> editing src/api/limiter.rs</>) },
  { kind: "out", node: <span className="text-t-cyan">  done. 12 tests passed, ready to review</span> },
];

const SHOWCASE_CODE = `pub struct Scheduler {
    queue: VecDeque<Task>,
    workers: Vec<Worker>,
}

impl Scheduler {
    pub fn tick(&mut self) -> Status {
        while let Some(task) = self.queue.pop_front() {
            self.dispatch(task);
        }
        Status::Idle
    }
}`;

const SHOWCASE_TERM: TLine[] = [
  { kind: "cmd", text: "cargo run --release" },
  { kind: "sys", node: "Compiling orbit v0.4.0" },
  { kind: "out", node: <span className="text-t-cyan">Finished release in 8.2s</span> },
  { kind: "out", node: (<><span className="text-t-green">▸</span> listening on :7070</>) },
];

const DIFF_CODE = `@@ src/api/mod.rs
 pub fn route(req: Request) -> Response {
-    handle(req)
+    let req = limiter.check(req)?;
+    handle(req)
 }`;

const RECALL_TERM: TLine[] = [
  { kind: "cmd", text: "even recall" },
  { kind: "sys", node: "3 sessions in ~/work/orbit" },
  { kind: "out", node: (<><span className="text-t-green">refactor scheduler</span>   <span className="text-t-ink-3">cached · all tests green</span></>) },
  { kind: "out", node: (<><span className="text-t-amber">rate limiting</span>      <span className="text-t-ink-3">active · agent-1 on even/a1</span></>) },
  { kind: "out", node: (<><span className="text-t-green">browser favicons</span>   <span className="text-t-ink-3">cached · resolver shipped</span></>) },
];

const INTEGRATIONS = [
  { slug: "anthropic", label: "Claude" },
  { slug: "gnubash", label: "Bash" },
  { slug: "git", label: "Git" },
  { slug: "rust", label: "Rust" },
  { slug: "typescript", label: "TypeScript" },
  { slug: "python", label: "Python" },
  { slug: "go", label: "Go" },
];

const FEATURES = [
  { icon: <TerminalWindow className="h-5 w-5" />, title: "A terminal worth living in", desc: "Split, tab, and tile real shell panes." },
  { icon: <GlobeHemisphereWest className="h-5 w-5" />, title: "Browser in a pane", desc: "Docs and dashboards beside the code." },
  { icon: <Robot className="h-5 w-5" />, title: "Agents in their own panes", desc: "Briefed, isolated, watchable." },
  { icon: <GitBranch className="h-5 w-5" />, title: "Git in view", desc: "Diff, stage, and commit any branch." },
  { icon: <ShieldCheck className="h-5 w-5" />, title: "Sandboxed by default", desc: "Worktrees and approval gates." },
  { icon: <Brain className="h-5 w-5" />, title: "Shared memory", desc: "Every session, indexed and recallable." },
];

const STEPS = [
  { verb: "Open your project", cmd: "even .", line: "Your folder opens with terminal, browser, and editor in one window." },
  { verb: "Delegate the work", cmd: 'even run "fix the flaky test"', line: "An agent picks it up in its own git worktree, briefed from past sessions." },
  { verb: "Review and ship", cmd: "approve the diff", line: "Read the change in place, approve, and it commits to its branch." },
];

const FAQ = [
  { q: "Is the terminal a real terminal?", a: "Yes. Even runs a full PTY with your real shell, not a web emulator." },
  { q: "Which agents can I run?", a: "Claude and Codex today, with your own keys. More are on the way." },
  { q: "Does my code leave my machine?", a: "Only what you choose to send to an agent. Everything else stays local." },
  { q: "Do I have to change my setup?", a: "No. Even wraps the shell, git, and tools you already use." },
  { q: "Can an agent break my repo?", a: "No. Each runs in an isolated worktree, and risky commands wait for your approval." },
  { q: "Is it free?", a: "Yes, free while Even is in beta." },
];

/* ============================================================ primitives === */

function Section({ id, children, className }: { id?: string; children: ReactNode; className?: string }) {
  return (
    <section id={id} className={cn("mx-auto w-full max-w-[1200px] px-5 sm:px-8", className)}>
      {children}
    </section>
  );
}

function Eyebrow({ children }: { children: ReactNode }) {
  return <p className="mb-4 font-mono text-[12px] uppercase tracking-[0.2em] text-primary">{children}</p>;
}

function Heading({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <h2 className={cn("font-sans text-[1.9rem] font-semibold leading-[1.08] tracking-[-0.02em] text-ink sm:text-4xl md:text-[2.7rem]", className)}>
      {children}
    </h2>
  );
}

const PANE = "p-4 font-mono text-[12.5px] leading-[1.7]";

function Integration({ slug, label }: { slug: string; label: string }) {
  const url = `https://cdn.simpleicons.org/${slug}`;
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className="h-7 w-7 bg-ink-3 opacity-70 transition-opacity hover:opacity-100"
      style={{
        maskImage: `url(${url})`,
        WebkitMaskImage: `url(${url})`,
        maskRepeat: "no-repeat",
        WebkitMaskRepeat: "no-repeat",
        maskSize: "contain",
        WebkitMaskSize: "contain",
        maskPosition: "center",
      }}
    />
  );
}

/* ============================================================ app =========== */

export default function App() {
  const dl = useDownloads();
  const download = dl.urls[dl.primary];

  return (
    <div className="grain relative min-h-[100dvh] overflow-x-clip">
      <div
        className="pointer-events-none fixed inset-0 z-0"
        style={{ background: "radial-gradient(120% 80% at 50% -10%, var(--glow), transparent 55%)" }}
      />

      {/* ---- nav ---- */}
      <header className="sticky top-0 z-50 border-b border-border/70 bg-background/75 backdrop-blur-md">
        <div className="mx-auto flex h-16 max-w-[1200px] items-center justify-between px-5 sm:px-8">
          <a href="#top" aria-label="Even">
            <Logo className="text-[17px]" />
          </a>
          <nav className="hidden items-center gap-8 font-sans text-[14px] text-ink-2 md:flex">
            <a className="hover:text-ink" href="#features">Features</a>
            <a className="hover:text-ink" href="#workflow">Workflow</a>
            <a className="hover:text-ink" href="#faq">FAQ</a>
          </nav>
          <div className="flex items-center gap-3">
            <ThemeToggle />
            <Button asChild size="pill">
              <a href={download} target="_blank" rel="noopener noreferrer">Download</a>
            </Button>
          </div>
        </div>
      </header>

      <main id="top" className="relative z-10">
        {/* ===================== HERO ===================== */}
        <Section className="grid grid-cols-1 items-center gap-12 pt-14 pb-16 md:min-h-[calc(100dvh-4rem)] md:grid-cols-12 md:gap-10 md:pt-12 md:pb-20">
          <div className="md:col-span-6">
            <Reveal>
              <h1 className="font-sans text-[2.6rem] font-bold leading-[1.05] tracking-[-0.03em] text-ink sm:text-5xl lg:text-[3.6rem]">
                Your terminal, now a full workspace.
              </h1>
            </Reveal>
            <Reveal delay={0.08}>
              <p className="mt-6 max-w-[46ch] text-[1.05rem] leading-[1.6] text-ink-2">
                Browser, editor, and AI agents share the window with your shell, so you stop wiring five apps together.
              </p>
            </Reveal>
            <Reveal delay={0.16}>
              <div className="mt-9 flex flex-wrap items-center gap-3.5">
                <Button asChild size="lg">
                  <a href={download} target="_blank" rel="noopener noreferrer">Download</a>
                </Button>
                <Button asChild size="lg" variant="outline">
                  <a href="#workspace">
                    See it work
                    <ArrowRight weight="bold" className="h-4 w-4" />
                  </a>
                </Button>
              </div>
            </Reveal>
          </div>
          <Reveal delay={0.2} className="md:col-span-6">
            <Window title="zsh - even">
              <Terminal lines={HERO_TERM} className={cn(PANE, "min-h-[260px]")} />
            </Window>
          </Reveal>
        </Section>

        {/* ===================== INTEGRATIONS ===================== */}
        <Section className="border-y border-border py-10">
          <p className="mb-8 text-center text-[14px] text-ink-3">Works with the tools you already run</p>
          <div className="flex flex-wrap items-center justify-center gap-x-12 gap-y-7">
            {INTEGRATIONS.map((i) => (
              <Integration key={i.slug} slug={i.slug} label={i.label} />
            ))}
          </div>
        </Section>

        {/* ===================== SHOWCASE ===================== */}
        <Section id="workspace" className="py-20 md:py-28">
          <Reveal className="mx-auto mb-12 max-w-[42ch] text-center">
            <Heading>Everything you reach for, side by side.</Heading>
            <p className="mt-4 text-ink-2">Edit, run, browse, and hand work to agents without leaving the window.</p>
          </Reveal>
          <Reveal delay={0.1} className="relative">
            <div
              className="pointer-events-none absolute inset-0 -z-10"
              style={{ background: "radial-gradient(55% 50% at 50% 38%, var(--glow), transparent 72%)" }}
            />
            <Window title="~/work/orbit - even">
              <div className="grid h-[440px] grid-cols-1 md:grid-cols-[180px_1fr]">
                <aside className="hidden flex-col gap-1 border-r border-t-line bg-t-2 p-3 font-mono text-[12px] text-t-ink-2 md:flex">
                  <p className="px-2 pb-2 pt-1 text-[10.5px] uppercase tracking-[0.18em] text-t-ink-3">orbit</p>
                  {["src/", "  api/", "  scheduler.rs", "  main.rs", "tests/", "Cargo.toml"].map((f) => (
                    <span key={f} className={cn("px-2 py-1", f.includes("scheduler") && "bg-t-green/10 text-t-ink")}>
                      {f}
                    </span>
                  ))}
                </aside>
                <div className="grid grid-rows-[1fr_auto] overflow-hidden">
                  <div className="overflow-hidden border-b border-t-line">
                    <div className="flex h-9 items-center gap-2 border-b border-t-line bg-t-2 px-4 font-mono text-[11px]">
                      <span className="text-t-ink-2">scheduler.rs</span>
                    </div>
                    <CodeBlock code={SHOWCASE_CODE} lang="rust" className="p-4 text-[12.5px] leading-[1.7]" />
                  </div>
                  <Terminal lines={SHOWCASE_TERM} className={cn(PANE, "h-[150px]")} />
                </div>
              </div>
            </Window>
          </Reveal>
        </Section>

        {/* ===================== FEATURES (bento) ===================== */}
        <Section id="features" className="py-20 md:py-28">
          <Reveal className="mb-12 max-w-[34ch]">
            <Eyebrow>What is inside</Eyebrow>
            <Heading>Six tools that already know about each other.</Heading>
          </Reveal>
          <RevealGroup className="grid grid-cols-1 gap-4 md:grid-cols-4">
            <RevealItem className="md:col-span-2">
              <BentoCard {...FEATURES[0]}>
                <div className="mt-4 border border-t-line bg-t-bg p-3 font-mono text-[12px] leading-[1.7] text-t-ink-2">
                  <span className="text-t-green">❯</span> <span className="text-t-ink">cargo watch -x test</span>
                  <div className="text-t-cyan">running 24 tests... ok</div>
                </div>
              </BentoCard>
            </RevealItem>
            <RevealItem><BentoCard {...FEATURES[1]} /></RevealItem>
            <RevealItem><BentoCard {...FEATURES[5]} /></RevealItem>
            <RevealItem className="md:col-span-2">
              <BentoCard {...FEATURES[2]}>
                <div className="mt-4 grid gap-2">
                  {[
                    ["agent-1", "refactor scheduler", "running", true],
                    ["agent-2", "add tests", "idle 3m", false],
                  ].map(([id, task, state, live]) => (
                    <div key={id as string} className="flex items-center gap-3 border border-t-line bg-t-bg px-3 py-2 font-mono text-[12px]">
                      <span className={cn("h-1.5 w-1.5", live ? "bg-t-green" : "bg-t-amber")} />
                      <span className="text-t-ink">{id}</span>
                      <span className="text-t-ink-3">{task}</span>
                      <span className={cn("ml-auto", live ? "text-t-green" : "text-t-amber")}>{state}</span>
                    </div>
                  ))}
                </div>
              </BentoCard>
            </RevealItem>
            <RevealItem><BentoCard {...FEATURES[3]} /></RevealItem>
            <RevealItem><BentoCard {...FEATURES[4]} /></RevealItem>
          </RevealGroup>
        </Section>

        {/* ===================== WORKFLOW (command flow) ===================== */}
        <Section id="workflow" className="grid grid-cols-1 gap-12 py-20 md:grid-cols-12 md:gap-16 md:py-28">
          <Reveal className="md:col-span-5">
            <Heading>From open folder to shipped change.</Heading>
            <p className="mt-5 max-w-[42ch] text-ink-2">
              The whole loop runs in one window, with three commands you already half-know.
            </p>
            <Button asChild size="lg" className="mt-8">
              <a href={download} target="_blank" rel="noopener noreferrer">Download</a>
            </Button>
          </Reveal>
          <div className="md:col-span-7">
            <RevealGroup className="grid gap-8 border-l border-border pl-8">
              {STEPS.map((s) => (
                <RevealItem key={s.verb}>
                  <h3 className="font-sans text-[1.2rem] font-semibold tracking-[-0.01em] text-ink">{s.verb}</h3>
                  <code className="mt-3 inline-block border border-t-line bg-t-bg px-3 py-1.5 font-mono text-[12.5px] text-t-green">
                    {s.cmd}
                  </code>
                  <p className="mt-3 max-w-[52ch] text-[14.5px] leading-[1.55] text-ink-2">{s.line}</p>
                </RevealItem>
              ))}
            </RevealGroup>
          </div>
        </Section>

        {/* ===================== AGENTS + SAFETY ===================== */}
        <Section className="grid grid-cols-1 items-center gap-12 py-20 md:grid-cols-2 md:py-28">
          <Reveal>
            <Window title="agent-1 - even/a1 · sandboxed">
              <div className="grid grid-rows-[1fr_auto]">
                <div className="border-b border-t-line">
                  <div className="flex h-9 items-center border-b border-t-line bg-t-2 px-4 font-mono text-[11px] text-t-ink-3">
                    src/api/mod.rs
                  </div>
                  <CodeBlock code={DIFF_CODE} lang="diff" className="p-4 text-[12.5px] leading-[1.7]" />
                </div>
                <div className="flex items-center justify-between gap-3 p-3 font-mono text-[12px]">
                  <span className="text-t-ink-3">push to origin/even/a1</span>
                  <span className="flex gap-2">
                    <span className="bg-t-green px-3 py-1.5 font-semibold text-[#02160c]">Approve</span>
                    <span className="border border-t-line px-3 py-1.5 text-t-ink-2">Deny</span>
                  </span>
                </div>
              </div>
            </Window>
          </Reveal>
          <Reveal delay={0.1}>
            <Heading>Let agents run. Keep a hand on the wheel.</Heading>
            <p className="mt-5 max-w-[48ch] text-[1.02rem] leading-[1.65] text-ink-2">
              Each agent works in its own git worktree, so it never touches your main branch. Review the diff in place, and every push, delete, or deploy waits for one tap of approval.
            </p>
            <ul className="mt-7 grid gap-4">
              {[
                [<ShieldCheck key="a" className="h-5 w-5" />, "Worktree isolation", "One branch per agent, off your tree."],
                [<GitBranch key="b" className="h-5 w-5" />, "Review then ship", "Read the diff, approve, and commit in place."],
              ].map(([icon, t, d], i) => (
                <li key={i} className="flex items-start gap-3.5">
                  <span className="grid h-9 w-9 shrink-0 place-items-center border border-border bg-primary/10 text-primary">
                    {icon}
                  </span>
                  <span>
                    <span className="block font-medium text-ink">{t}</span>
                    <span className="block text-[14px] text-ink-3">{d}</span>
                  </span>
                </li>
              ))}
            </ul>
          </Reveal>
        </Section>

        {/* ===================== MEMORY ===================== */}
        <Section className="py-20 md:py-28">
          <div className="border border-border bg-[var(--bg-2)] p-8 md:p-14">
            <div className="grid grid-cols-1 items-center gap-10 md:grid-cols-12">
              <Reveal className="md:col-span-7">
                <Eyebrow>Powered by termem</Eyebrow>
                <Heading className="max-w-[18ch]">It remembers every session, across every tool.</Heading>
                <p className="mt-5 max-w-[52ch] text-[1.02rem] leading-[1.65] text-ink-2">
                  Underneath Even runs termem, a shared index of every terminal session from Claude, Codex, and your shell. Open a folder and continue where you, or an agent, stopped.
                </p>
              </Reveal>
              <Reveal delay={0.1} className="md:col-span-5">
                <div className="border border-t-line bg-t-bg">
                  <Terminal lines={RECALL_TERM} className={cn(PANE, "min-h-[180px]")} />
                </div>
              </Reveal>
            </div>
          </div>
        </Section>

        {/* ===================== FAQ ===================== */}
        <Section id="faq" className="py-20 md:py-28">
          <Reveal className="mx-auto mb-10 max-w-[24ch] text-center">
            <Heading>Questions, answered.</Heading>
          </Reveal>
          <Reveal delay={0.05} className="mx-auto max-w-[760px]">
            {FAQ.map((f) => (
              <details key={f.q} className="group border-b border-border py-5">
                <summary className="flex cursor-pointer list-none items-center justify-between gap-4 font-sans text-[1.05rem] font-medium text-ink">
                  {f.q}
                  <Plus weight="bold" className="h-4 w-4 shrink-0 text-ink-3 transition-transform group-open:rotate-45" />
                </summary>
                <p className="mt-3 max-w-[64ch] text-[15px] leading-[1.6] text-ink-2">{f.a}</p>
              </details>
            ))}
          </Reveal>
        </Section>

        {/* ===================== CTA ===================== */}
        <Section className="py-24 text-center md:py-32">
          <Reveal className="mx-auto flex max-w-[40ch] flex-col items-center">
            <Mark className="mb-7 h-14 w-14 text-primary" />
            <h2 className="font-sans text-[2.3rem] font-bold leading-[1.05] tracking-[-0.02em] text-ink sm:text-5xl">
              Make it Even.
            </h2>
            <p className="mt-5 text-ink-2">One window for your terminal, browser, editor, and agents. Free while in beta.</p>
            <div className="mt-9">
              <Button asChild size="lg">
                <a href={download} target="_blank" rel="noopener noreferrer">Download</a>
              </Button>
            </div>
            <div className="mt-5 flex flex-wrap items-center justify-center gap-x-4 gap-y-1 font-mono text-[12px] text-ink-3">
              <a className="hover:text-primary" href={dl.urls.mac} target="_blank" rel="noopener noreferrer">macOS</a>
              <a className="hover:text-primary" href={dl.urls.win} target="_blank" rel="noopener noreferrer">Windows</a>
              <a className="hover:text-primary" href={dl.urls.deb} target="_blank" rel="noopener noreferrer">Linux .deb</a>
              <a className="hover:text-primary" href={dl.urls.appimage} target="_blank" rel="noopener noreferrer">Linux .AppImage</a>
            </div>
          </Reveal>
        </Section>

        {/* ===================== FOOTER ===================== */}
        <footer className="border-t border-border">
          <Section className="grid grid-cols-1 gap-8 py-12 sm:grid-cols-2">
            <div>
              <Logo className="text-[16px]" />
              <p className="mt-3 max-w-[34ch] text-[14px] text-ink-3">The terminal-first workspace where your shell, the web, your editor, and agents live together.</p>
            </div>
            <nav className="flex flex-wrap gap-x-10 gap-y-3 font-sans text-[14px] text-ink-2 sm:justify-end">
              <a className="hover:text-primary" href="#features">Features</a>
              <a className="hover:text-primary" href="#workflow">Workflow</a>
              <a className="hover:text-primary" href="#faq">FAQ</a>
              <a className="hover:text-primary" href={download} target="_blank" rel="noopener noreferrer">Download</a>
            </nav>
          </Section>
          <Section className="flex items-center justify-between border-t border-border py-6 font-mono text-[12px] text-ink-3">
            <span>© 2026 Even</span>
            <a className="hover:text-primary" href="#top">Back to top</a>
          </Section>
        </footer>
      </main>
    </div>
  );
}

/* ============================================================ bento card ==== */

function BentoCard({
  icon,
  title,
  desc,
  children,
}: {
  icon: ReactNode;
  title: string;
  desc: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col border border-border bg-card p-6 transition-colors hover:border-[var(--line-strong)]">
      <span className="mb-4 grid h-10 w-10 place-items-center border border-border bg-primary/10 text-primary">
        {icon}
      </span>
      <h3 className="font-sans text-[1.15rem] font-semibold tracking-[-0.01em] text-ink">{title}</h3>
      <p className="mt-2 text-[14px] leading-[1.5] text-ink-2">{desc}</p>
      {children}
    </div>
  );
}
