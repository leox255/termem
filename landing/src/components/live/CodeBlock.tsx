import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

/** Real syntax highlighting via Shiki. Falls back to plain mono until ready. */
export function CodeBlock({
  code,
  lang = "tsx",
  className,
}: {
  code: string;
  lang?: string;
  className?: string;
}) {
  const trimmed = code.replace(/\n+$/, "");
  const [html, setHtml] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    // lazy-load Shiki so it stays out of the initial bundle (code is below the fold)
    import("@/lib/highlighter")
      .then(({ getHighlighter, THEME }) =>
        getHighlighter().then((hl) => {
          if (alive) setHtml(hl.codeToHtml(trimmed, { lang, theme: THEME }));
        })
      )
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [trimmed, lang]);

  if (!html) {
    return (
      <pre className={cn("overflow-x-auto whitespace-pre font-mono text-[12.5px] leading-[1.65] text-ink-2", className)}>
        {trimmed}
      </pre>
    );
  }
  return (
    <div
      className={cn("[&_pre]:m-0 [&_pre]:overflow-x-auto [&_pre]:bg-transparent", className)}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
