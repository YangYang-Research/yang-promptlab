import type { SelectHTMLAttributes } from "react";

type SelectProps = SelectHTMLAttributes<HTMLSelectElement> & {
  inline?: boolean;
};

export function Select({ className, inline, ...props }: SelectProps) {
  const classes = ["select", inline ? "select--inline" : "", className]
    .filter(Boolean)
    .join(" ");
  return <select className={classes} {...props} />;
}
