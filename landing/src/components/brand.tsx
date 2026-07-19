import { cn } from "@/lib/utils";

/**
 * The Even mark: three bars skewed -13deg (an "E" without its spine / a triple
 * equals). A single, simple geometric mark, rendered inline per brand spec.
 */
export function Mark({ className }: { className?: string }) {
  return (
    <svg viewBox="-317.7 -310 635.4 620" className={className} fill="currentColor" aria-hidden>
      <g transform="skewX(-13)">
        <rect x="-240" y="-302" width="480" height="132" rx="66" />
        <rect x="-240" y="-66" width="480" height="132" rx="66" />
        <rect x="-240" y="170" width="480" height="132" rx="66" />
      </g>
    </svg>
  );
}

/** Wordmark lockup: mark + "Even" set in the brand font (Saira). */
export function Logo({ className, markClass }: { className?: string; markClass?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-2.5 text-ink", className)}>
      <Mark className={cn("h-[1.05em] w-[1.05em] text-primary", markClass)} />
      <span className="font-wordmark font-semibold leading-none tracking-[-0.02em]">Even</span>
    </span>
  );
}
