import { cn } from "@/lib/utils";

/**
 * The Even mark: three bars skewed -13deg (an "E" without its spine / a triple
 * equals). It is a custom geometric mark, not a font glyph, so it stays inline
 * SVG. `animate` adds the staggered bar-draw entrance (driven by [data-in]).
 */
export function Mark({
  className,
  animate = false,
}: {
  className?: string;
  animate?: boolean;
}) {
  return (
    <svg viewBox="-317.7 -310 635.4 620" className={className} fill="currentColor" aria-hidden>
      <g transform="skewX(-13)">
        <rect className={cn(animate && "mark-bar")} x="-240" y="-302" width="480" height="132" rx="66" />
        <rect className={cn(animate && "mark-bar")} x="-240" y="-66" width="480" height="132" rx="66" />
        <rect className={cn(animate && "mark-bar")} x="-240" y="170" width="480" height="132" rx="66" />
      </g>
    </svg>
  );
}

/** The "even" wordmark, set in the logo font (Saira 600) as live text. */
export function Wordmark({ className }: { className?: string }) {
  return (
    <span className={cn("font-sans font-semibold leading-none tracking-[-0.01em] text-green", className)}>
      even
    </span>
  );
}
