type IconProps = {
  className?: string;
  "aria-label"?: string;
};

export function IconBack({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path
        d="M11 4 6 9l5 5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconTable({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <rect x="2.5" y="3.5" width="13" height="11" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M2.5 7.5h13M2.5 11h13M7 3.5v11" fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function IconList({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path d="M3.5 5h11M3.5 9h11M3.5 13h11" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
    </svg>
  );
}

export function IconDiscovery({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <circle cx="8" cy="8" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="m11.5 11.5 3 3" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

export function IconMore({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <circle cx="4.5" cy="9" r="1.25" fill="currentColor" />
      <circle cx="9" cy="9" r="1.25" fill="currentColor" />
      <circle cx="13.5" cy="9" r="1.25" fill="currentColor" />
    </svg>
  );
}

export function IconRefresh({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path
        d="M14.5 9a5.5 5.5 0 0 1-9.2 4M3.5 9a5.5 5.5 0 0 1 9.2-4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <path
        d="M12.5 3.5h2v2M5.5 14.5h-2v-2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconWarning({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path
        d="M9 3.5 15.5 14.5H2.5L9 3.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      <path d="M9 7.5v3.5" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="9" cy="13" r="0.75" fill="currentColor" />
    </svg>
  );
}

export function IconArrowRight({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path
        d="M7 4.5 11.5 9 7 13.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconOnDevice({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <rect
        x="3.5"
        y="4.5"
        width="11"
        height="8"
        rx="1.25"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path
        d="M2 13.5h14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function IconCloud({ className }: IconProps) {
  return (
    <svg className={className} width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path
        d="M6.25 13.5h6.75a2.75 2.75 0 0 0 .45-5.46A3.5 3.5 0 0 0 5.2 6.35 2.75 2.75 0 0 0 6.25 13.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconCheck({ className }: IconProps) {
  return (
    <svg className={className} width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      <path
        d="M3.25 7.25 5.75 9.75 10.75 4.75"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function IconX({ className }: IconProps) {
  return (
    <svg className={className} width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      <path
        d="M4 4l6 6M10 4 4 10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
    </svg>
  );
}

/** Bold dual-sparkle mark for AI-planned modes. */
export function IconAi({ className, "aria-label": ariaLabel }: IconProps) {
  return (
    <svg
      className={className}
      width="24"
      height="24"
      viewBox="0 0 24 24"
      aria-hidden={ariaLabel ? undefined : true}
      aria-label={ariaLabel}
      role={ariaLabel ? "img" : undefined}
    >
      <path
        d="M12 2.75 14.1 10.15 21.5 12.25 14.1 14.35 12 21.75 9.9 14.35 2.5 12.25 9.9 10.15Z"
        fill="currentColor"
        stroke="currentColor"
        strokeWidth="1.25"
        strokeLinejoin="round"
      />
      <path
        d="M18.25 4.25 18.85 6.05 20.65 6.65 18.85 7.25 18.25 9.05 17.65 7.25 15.85 6.65 17.65 6.05Z"
        fill="currentColor"
      />
      <path
        d="M6.15 15.9 6.55 17.1 7.75 17.5 6.55 17.9 6.15 19.1 5.75 17.9 4.55 17.5 5.75 17.1Z"
        fill="currentColor"
      />
    </svg>
  );
}

/** Person mark for manual custom attack mode. */
export function IconHuman({ className, "aria-label": ariaLabel }: IconProps) {
  return (
    <svg
      className={className}
      width="18"
      height="18"
      viewBox="0 0 18 18"
      aria-hidden={ariaLabel ? undefined : true}
      aria-label={ariaLabel}
      role={ariaLabel ? "img" : undefined}
    >
      <circle cx="9" cy="6" r="2.75" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path
        d="M4.75 14.75c.85-2.45 2.55-3.75 4.25-3.75s3.4 1.3 4.25 3.75"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}
