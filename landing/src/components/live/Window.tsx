import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Frames real, live components as a (always-dark) terminal window. */
export function Window({
  title,
  children,
  className,
  bodyClassName,
}: {
  title?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <div
      className={cn(
        "overflow-hidden border border-t-line bg-t-bg",
        "shadow-[0_34px_90px_-34px_rgba(1,8,16,0.7)]",
        className
      )}
    >
      <div className="flex h-10 items-center gap-3 border-b border-t-line bg-t-2 px-4">
        <span className="flex gap-2">
          <i className="h-3 w-3 bg-[#ff5f57]" />
          <i className="h-3 w-3 bg-[#febc2e]" />
          <i className="h-3 w-3 bg-[#28c840]" />
        </span>
        {title ? <span className="truncate font-mono text-[11.5px] text-t-ink-3">{title}</span> : null}
      </div>
      <div className={cn("bg-t-bg", bodyClassName)}>{children}</div>
    </div>
  );
}
