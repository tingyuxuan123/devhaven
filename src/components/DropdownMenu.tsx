import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

export type DropdownItem = {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  destructive?: boolean;
  active?: boolean;
};

type DropdownMenuProps = {
  label: ReactNode;
  items: DropdownItem[];
  align?: "left" | "right";
  ariaLabel?: string;
  triggerClassName?: string;
};

/** 通用下拉菜单，支持外部点击关闭与键盘退出。 */
export default function DropdownMenu({
  label,
  items,
  align = "right",
  ariaLabel,
  triggerClassName,
}: DropdownMenuProps) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const resolvedLabel = ariaLabel ?? (typeof label === "string" ? label : "更多操作");
  const triggerClass = triggerClassName ?? "icon-button";

  useEffect(() => {
    if (!open) {
      return;
    }
    const handleClick = (event: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  return (
    <div className="dropdown" ref={wrapperRef}>
      <button
        className={`dropdown-trigger ${triggerClass}`}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((prev) => !prev);
        }}
        aria-haspopup="menu"
        aria-label={resolvedLabel}
      >
        {label}
      </button>
      {open ? (
        <div className={`dropdown-menu ${align === "left" ? "is-left" : "is-right"}`}>
          {items.map((item) => (
            <button
              key={item.label}
              className={`dropdown-item${item.destructive ? " is-destructive" : ""}${item.active ? " is-active" : ""}`}
              onClick={(event) => {
                event.stopPropagation();
                if (item.disabled) {
                  return;
                }
                item.onClick?.();
                setOpen(false);
              }}
              disabled={item.disabled}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
