import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export type MultiSelectOption = {
  value: string;
  label: string;
};

type MenuPosition = {
  top: number;
  left: number;
  width: number;
};

type MultiSelectProps = {
  label: string;
  options: MultiSelectOption[];
  values: string[];
  onChange: (values: string[]) => void;
  allLabel?: string;
  className?: string;
};

const MENU_GAP_PX = 4;
const VIEWPORT_PADDING_PX = 8;

export function MultiSelect({
  label,
  options,
  values,
  onChange,
  allLabel = "All",
  className,
}: MultiSelectProps) {
  const [open, setOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<MenuPosition | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selectedLabels = options
    .filter((option) => values.includes(option.value))
    .map((option) => option.label);

  const summary =
    values.length === 0
      ? allLabel
      : values.length === 1
        ? selectedLabels[0] ?? allLabel
        : `${label} (${values.length})`;

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
    const width = Math.max(anchorRect.width, 180);
    const spaceBelow = window.innerHeight - anchorRect.bottom - MENU_GAP_PX;
    const openUp = menuRect.height > spaceBelow && anchorRect.top > menuRect.height;

    let top = openUp
      ? anchorRect.top - MENU_GAP_PX - menuRect.height
      : anchorRect.bottom + MENU_GAP_PX;
    let left = anchorRect.left;

    left = Math.max(
      VIEWPORT_PADDING_PX,
      Math.min(left, window.innerWidth - width - VIEWPORT_PADDING_PX),
    );
    top = Math.max(
      VIEWPORT_PADDING_PX,
      Math.min(top, window.innerHeight - menuRect.height - VIEWPORT_PADDING_PX),
    );

    setMenuPosition({ top, left, width });
  }, [open, values.length, options.length]);

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

  function toggleValue(value: string) {
    if (values.includes(value)) {
      onChange(values.filter((item) => item !== value));
    } else {
      onChange([...values, value]);
    }
  }

  return (
    <div className={["multi-select", className].filter(Boolean).join(" ")} ref={rootRef}>
      <button
        type="button"
        className={`multi-select__trigger ${open ? "multi-select__trigger--open" : ""} ${
          values.length > 0 ? "multi-select__trigger--active" : ""
        }`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={label}
        onClick={() => setOpen((current) => !current)}
      >
        <span className="multi-select__summary">{summary}</span>
      </button>

      {open &&
        createPortal(
          <div
            ref={menuRef}
            id={listId}
            className="multi-select__menu"
            role="listbox"
            aria-multiselectable="true"
            aria-label={label}
            style={
              menuPosition
                ? {
                    top: menuPosition.top,
                    left: menuPosition.left,
                    width: menuPosition.width,
                  }
                : { top: 0, left: 0, visibility: "hidden" }
            }
          >
            <button
              type="button"
              className="multi-select__option multi-select__option--all"
              role="option"
              aria-selected={values.length === 0}
              onClick={() => onChange([])}
            >
              <span className="multi-select__check" aria-hidden="true">
                {values.length === 0 ? "✓" : ""}
              </span>
              {allLabel}
            </button>
            <div className="multi-select__divider" />
            {options.map((option) => {
              const selected = values.includes(option.value);
              return (
                <button
                  key={option.value}
                  type="button"
                  className={`multi-select__option ${
                    selected ? "multi-select__option--selected" : ""
                  }`}
                  role="option"
                  aria-selected={selected}
                  onClick={() => toggleValue(option.value)}
                >
                  <span className="multi-select__check" aria-hidden="true">
                    {selected ? "✓" : ""}
                  </span>
                  {option.label}
                </button>
              );
            })}
          </div>,
          document.body,
        )}
    </div>
  );
}
