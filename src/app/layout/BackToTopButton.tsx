import { useEffect, useState, type RefObject } from "react";

import { IconArrowUp, IconButton } from "@/shared/components";

const SHOW_AFTER_PX = 240;

type BackToTopButtonProps = {
  scrollRootRef: RefObject<HTMLElement | null>;
};

export function BackToTopButton({ scrollRootRef }: BackToTopButtonProps) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const root = scrollRootRef.current;
    if (!root) return;

    const update = () => {
      setVisible(root.scrollTop >= SHOW_AFTER_PX);
    };

    update();
    root.addEventListener("scroll", update, { passive: true });
    return () => root.removeEventListener("scroll", update);
  }, [scrollRootRef]);

  return (
    <div className={["back-to-top", visible ? "back-to-top--visible" : ""].filter(Boolean).join(" ")}>
      <IconButton
        ariaLabel="Back to top"
        variant="ghost"
        onClick={() => {
          scrollRootRef.current?.scrollTo({ top: 0, behavior: "smooth" });
        }}
      >
        <IconArrowUp />
      </IconButton>
    </div>
  );
}
