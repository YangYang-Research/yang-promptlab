import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { IconChevronDown, IconMore } from "./Icons";
import { IconButton } from "./IconButton";

export type ActionsDropdownItem = {
  id: string;
  label: string;
  onClick: () => void;
  icon?: ReactNode;
  tone?: "default" | "danger";
  disabled?: boolean;
};

type ActionsDropdownProps = {
  label?: string;
  items: ActionsDropdownItem[];
  disabled?: boolean;
  /** Labeled trigger (e.g. primary CTA). Defaults to the overflow icon button. */
  buttonLabel?: string;
  buttonVariant?: "primary" | "secondary" | "ghost";
};

type MenuPosition = {
  top: number;
  left: number;
};

const MENU_GAP_PX = 6;
const VIEWPORT_PADDING_PX = 8;

export function ActionsDropdown({
  label = "Actions",
  items,
  disabled,
  buttonLabel,
  buttonVariant = "secondary",
}: ActionsDropdownProps) {
  const [open, setOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<MenuPosition | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

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
  }, [open, items]);

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

  return (
    <div className="actions-dropdown" ref={rootRef}>
      {buttonLabel ? (
        <button
          type="button"
          className={`btn btn--${buttonVariant} btn--md`}
          aria-label={label}
          aria-haspopup="menu"
          aria-expanded={open}
          disabled={disabled}
          onClick={() => setOpen((value) => !value)}
        >
          {buttonLabel}
          <IconChevronDown className="btn__caret-icon" />
        </button>
      ) : (
        <IconButton
          ariaLabel={label}
          active={open}
          disabled={disabled}
          onClick={() => setOpen((value) => !value)}
        >
          <IconMore />
        </IconButton>
      )}
      {open &&
        createPortal(
          <div
            ref={menuRef}
            className={`actions-dropdown__menu actions-dropdown__menu--portal${
              buttonLabel ? " actions-dropdown__menu--wide" : ""
            }`}
            role="menu"
            style={
              menuPosition
                ? { top: menuPosition.top, left: menuPosition.left }
                : { top: 0, left: 0, visibility: "hidden" }
            }
          >
            {items.map((item) => (
              <button
                key={item.id}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                className={`actions-dropdown__item ${
                  item.tone === "danger" ? "actions-dropdown__item--danger" : ""
                } ${item.disabled ? "actions-dropdown__item--disabled" : ""}`}
                onClick={() => {
                  if (item.disabled) return;
                  setOpen(false);
                  item.onClick();
                }}
              >
                {item.icon ? (
                  <span className="actions-dropdown__item-icon" aria-hidden="true">
                    {item.icon}
                  </span>
                ) : null}
                <span className="actions-dropdown__item-label">{item.label}</span>
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
