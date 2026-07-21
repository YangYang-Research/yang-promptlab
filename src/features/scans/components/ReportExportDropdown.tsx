import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "@/features/reports/reportDownloads";
import { Button } from "@/shared/components";
import { useToast } from "@/shared/notifications";

const EXPORT_FORMATS: ReportExportFormat[] = ["html", "pdf", "sarif", "csv"];

type MenuPosition = {
  top: number;
  left: number;
};

const MENU_GAP_PX = 6;
const VIEWPORT_PADDING_PX = 8;

type ReportExportDropdownProps = {
  projectId: string;
  scanId: string;
  findingsCount: number;
  disabled?: boolean;
};

export function ReportExportDropdown({
  projectId,
  scanId,
  findingsCount,
  disabled = false,
}: ReportExportDropdownProps) {
  const { notify } = useToast();
  const [open, setOpen] = useState(false);
  const [exporting, setExporting] = useState<ReportExportFormat | null>(null);
  const [menuPosition, setMenuPosition] = useState<MenuPosition | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const exportDisabled = disabled || findingsCount === 0 || exporting !== null;

  useLayoutEffect(() => {
    if (!open) {
      setMenuPosition(null);
      return;
    }

    const anchor = rootRef.current;
    const menu = menuRef.current;
    if (!anchor || !menu) return;

    const anchorRect = anchor.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect();
    const menuWidth = menuRect.width;
    const menuHeight = menuRect.height;

    const spaceBelow = window.innerHeight - anchorRect.bottom - MENU_GAP_PX;
    const spaceAbove = anchorRect.top - MENU_GAP_PX;
    const openUp = menuHeight > spaceBelow && spaceAbove >= menuHeight;

    let top = openUp
      ? anchorRect.top - MENU_GAP_PX - menuHeight
      : anchorRect.bottom + MENU_GAP_PX;

    let left = anchorRect.right - menuWidth;
    left = Math.max(
      VIEWPORT_PADDING_PX,
      Math.min(left, window.innerWidth - menuWidth - VIEWPORT_PADDING_PX),
    );
    top = Math.max(
      VIEWPORT_PADDING_PX,
      Math.min(top, window.innerHeight - menuHeight - VIEWPORT_PADDING_PX),
    );

    setMenuPosition({ top, left });
  }, [open]);

  useEffect(() => {
    if (!open) return;

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) {
        return;
      }
      setOpen(false);
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [open]);

  async function handleExport(format: ReportExportFormat) {
    setExporting(format);
    setOpen(false);
    try {
      const path = await generateAndExportScanReport(projectId, scanId, format);
      notify(`${reportExportLabel(format)} report saved`, "success");
      if (path) {
        notify(`Saved to ${path}`, "info");
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Report export failed";
      notify(message, "error");
    } finally {
      setExporting(null);
    }
  }

  return (
    <div className="report-export-dropdown" ref={rootRef}>
      <Button
        variant="secondary"
        size="sm"
        disabled={exportDisabled}
        onClick={() => setOpen((value) => !value)}
      >
        {exporting ? "Exporting…" : "Export"}
      </Button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            className="actions-dropdown__menu actions-dropdown__menu--portal report-export-dropdown__menu"
            role="menu"
            style={
              menuPosition
                ? { top: menuPosition.top, left: menuPosition.left }
                : { top: 0, left: 0, visibility: "hidden" }
            }
          >
            {EXPORT_FORMATS.map((format) => (
              <button
                key={format}
                type="button"
                role="menuitem"
                className="actions-dropdown__item"
                disabled={exporting !== null}
                onClick={() => void handleExport(format)}
              >
                {exporting === format ? "Generating…" : reportExportLabel(format)}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
