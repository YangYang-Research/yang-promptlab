import { useEffect, useRef, useState } from "react";

import { IconMore } from "./Icons";
import { IconButton } from "./IconButton";

export type ActionsDropdownItem = {
  id: string;
  label: string;
  onClick: () => void;
  tone?: "default" | "danger";
};

type ActionsDropdownProps = {
  label?: string;
  items: ActionsDropdownItem[];
};

export function ActionsDropdown({ label = "Actions", items }: ActionsDropdownProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    function handlePointerDown(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
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

  return (
    <div className="actions-dropdown" ref={rootRef}>
      <IconButton
        ariaLabel={label}
        active={open}
        onClick={() => setOpen((value) => !value)}
      >
        <IconMore />
      </IconButton>
      {open && (
        <div className="actions-dropdown__menu" role="menu">
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="menuitem"
              className={`actions-dropdown__item ${
                item.tone === "danger" ? "actions-dropdown__item--danger" : ""
              }`}
              onClick={() => {
                setOpen(false);
                item.onClick();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
