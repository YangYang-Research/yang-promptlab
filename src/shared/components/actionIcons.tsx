import type { ReactNode } from "react";

import {
  IconArrowRight,
  IconExternalLink,
  IconPlus,
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
    case "Change Attack Plan":
      return <IconArrowRight />;
    case "New Scan":
      return <IconPlus />;
    default:
      return <IconExternalLink />;
  }
}
