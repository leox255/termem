import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function TrafficLights() {
  return (
    <div className="flex gap-2">
      <i className="block h-3 w-3 rounded-full bg-[#ff5f57]" />
      <i className="block h-3 w-3 rounded-full bg-[#febc2e]" />
      <i className="block h-3 w-3 rounded-full bg-[#28c840]" />
    </div>
  );
}

export function Window({
  title,
  children,
  className,
}: {
  title: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "w-full overflow-hidden rounded-[14px] border border-[rgba(120,200,170,0.14)]",
        "bg-gradient-to-b from-[#072032] to-[#04141f]",
        "shadow-[0_40px_90px_-30px_rgba(0,0,0,0.9),0_0_60px_-20px_rgba(71,255,156,0.18)]",
        className
      )}
    >
      <div className="flex h-[38px] items-center gap-3.5 border-b border-black/40 bg-gradient-to-b from-[#0c2638] to-[#082031] px-3.5">
        <TrafficLights />
        <div className="flex-1 text-center font-mono text-xs tracking-[0.04em] text-ink-3">{title}</div>
      </div>
      <div className="flex h-[clamp(258px,36vh,420px)] bg-void">{children}</div>
    </div>
  );
}

type Tone = "live" | "idle" | "stuck" | "off";
const toneMap: Record<Tone, string> = {
  live: "bg-green shadow-[0_0_8px_var(--green)] dot-pulse",
  idle: "bg-amber shadow-[0_0_8px_rgba(255,194,102,0.6)]",
  stuck: "bg-red shadow-[0_0_8px_rgba(255,122,107,0.6)] dot-pulse-fast",
  off: "bg-[#3a4f56]",
};

export function Dot({ tone, className }: { tone: Tone; className?: string }) {
  return <span className={cn("block h-[7px] w-[7px] shrink-0 rounded-full", toneMap[tone], className)} />;
}

export function Cursor() {
  return <span className="term-cursor align-[-2px]" />;
}
