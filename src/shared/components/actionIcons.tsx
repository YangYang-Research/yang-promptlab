import type { ReactNode } from "react";

import {
  IconArrowRight,
  IconExternalLink,
  IconProgress,
  IconRefresh,
} from "./Icons";

/** Icon for dynamic scan "open" actions (Continue Setup / Progress / Retry / Details). */
export function scanOpenActionIcon(label: string): ReactNode {
  switch (label) {
    case "Continue Setup":
      return <IconArrowRight />;
    case "View Scan Progress":
      return <IconProgress />;
    case "Retry Scan":
      return <IconRefresh />;
    default:
      return <IconExternalLink />;
  }
}
