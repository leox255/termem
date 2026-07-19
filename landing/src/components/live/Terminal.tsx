import { useEffect, useState, type ReactNode } from "react";
import { useReducedMotion } from "motion/react";
import { cn } from "@/lib/utils";

export type TLine =
  | { kind: "cmd"; text: string }
  | { kind: "out"; node: ReactNode }
  | { kind: "sys"; node: ReactNode };

function Cursor() {
  return <span className="terminal-cursor ml-0.5" />;
}

/**
 * A real animated terminal: command lines type out character by character,
 * output lines appear in sequence, then it loops. Not a static fake, it runs.
 * Collapses to the finished state under prefers-reduced-motion.
 */
export function Terminal({ lines, className }: { lines: TLine[]; className?: string }) {
  const reduce = useReducedMotion();
  const [pos, setPos] = useState({ line: 0, char: 0 });

  useEffect(() => {
    if (reduce) {
      setPos({ line: lines.length, char: 0 });
      return;
    }
    const cur = lines[pos.line];
    let t: number;
    if (!cur) {
      t = window.setTimeout(() => setPos({ line: 0, char: 0 }), 2800);
    } else if (cur.kind === "cmd") {
      if (pos.char < cur.text.length) {
        t = window.setTimeout(() => setPos((p) => ({ line: p.line, char: p.char + 1 })), 24 + Math.random() * 34);
      } else {
        t = window.setTimeout(() => setPos((p) => ({ line: p.line + 1, char: 0 })), 380);
      }
    } else {
      t = window.setTimeout(() => setPos((p) => ({ line: p.line + 1, char: 0 })), cur.kind === "sys" ? 90 : 240);
    }
    return () => clearTimeout(t);
  }, [pos, reduce, lines]);

  return (
    <div className={cn("font-mono text-[12.5px] leading-[1.75] text-t-ink-2", className)}>
      {lines.slice(0, pos.line + 1).map((ln, i) => {
        const isCurrent = i === pos.line;
        if (ln.kind === "cmd") {
          const shown = isCurrent && !reduce ? ln.text.slice(0, pos.char) : ln.text;
          return (
            <div key={i}>
              <span className="text-t-green">❯</span>{" "}
              <span className="text-t-ink">{shown}</span>
              {isCurrent && <Cursor />}
            </div>
          );
        }
        return (
          <div key={i} className={ln.kind === "sys" ? "text-t-ink-3" : ""}>
            {ln.node}
          </div>
        );
      })}
      {(pos.line >= lines.length || reduce) && (
        <div>
          <span className="text-t-green">❯</span> {!reduce && <Cursor />}
        </div>
      )}
    </div>
  );
}
