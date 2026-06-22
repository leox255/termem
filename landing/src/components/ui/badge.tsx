import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-2 rounded-full border font-mono text-[11px] tracking-wide whitespace-nowrap px-3 py-1.5",
  {
    variants: {
      variant: {
        default: "border-border bg-[var(--paneglass,rgba(8,28,44,.72))] text-ink",
        muted: "border-border bg-card/60 text-ink-2",
      },
    },
    defaultVariants: { variant: "default" },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
