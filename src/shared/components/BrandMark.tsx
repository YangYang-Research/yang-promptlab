import type { CSSProperties } from "react";

import flaskMask from "@/assets/brand/flask-mask.png";

type BrandMarkProps = {
  className?: string;
  size?: number;
};

/** PromptLab mark from source flask artwork. Colors follow theme. */
export function BrandMark({ className, size = 32 }: BrandMarkProps) {
  return (
    <span
      className={["brand-mark", className].filter(Boolean).join(" ")}
      style={
        {
          width: size,
          height: size,
          "--brand-mark-mask": `url("${flaskMask}")`,
        } as CSSProperties
      }
      aria-hidden="true"
    >
      <span className="brand-mark__glyph" />
    </span>
  );
}
