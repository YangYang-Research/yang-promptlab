import { useState } from "react";
import { Link } from "react-router-dom";

import { IconArrowRight, IconCheck } from "@/shared/components";

import { useSetupChecklist } from "./useSetupChecklist";

export function SetupChecklistPanel() {
  const { visible, steps, doneCount, total } = useSetupChecklist();
  const [expanded, setExpanded] = useState(true);

  if (!visible) return null;

  return (
    <aside
      className={[
        "setup-checklist",
        expanded ? "setup-checklist--expanded" : "setup-checklist--collapsed",
      ].join(" ")}
      aria-label="First-time setup"
    >
      <header className="setup-checklist__header">
        <button
          type="button"
          className="setup-checklist__toggle"
          onClick={() => setExpanded((value) => !value)}
          aria-expanded={expanded}
        >
          <span className="setup-checklist__eyebrow">Getting started</span>
          <span className="setup-checklist__progress">
            {doneCount}/{total} complete
          </span>
        </button>
      </header>

      {expanded ? (
        <ol className="setup-checklist__list">
          {steps.map((step, index) => {
            const marker = (
              <span
                className={[
                  "setup-checklist__marker",
                  step.done ? "setup-checklist__marker--done" : "",
                  step.locked ? "setup-checklist__marker--locked" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                aria-hidden
              >
                {step.done ? <IconCheck /> : index + 1}
              </span>
            );
            const copy = (
              <div className="setup-checklist__copy">
                <span className="setup-checklist__title">{step.title}</span>
                <span className="setup-checklist__desc">
                  {step.locked
                    ? `Complete step ${index} first.`
                    : step.description}
                </span>
              </div>
            );

            if (step.done) {
              return (
                <li key={step.id}>
                  <div className="setup-checklist__item setup-checklist__item--done">
                    {marker}
                    {copy}
                  </div>
                </li>
              );
            }

            if (step.locked) {
              return (
                <li key={step.id}>
                  <div
                    className="setup-checklist__item setup-checklist__item--locked"
                    aria-disabled="true"
                    title={`Complete step ${index} first`}
                  >
                    {marker}
                    {copy}
                  </div>
                </li>
              );
            }

            return (
              <li key={step.id}>
                <Link
                  to={step.to}
                  className="setup-checklist__item setup-checklist__item--current"
                  aria-label={`Step ${index + 1}: ${step.title}`}
                >
                  {marker}
                  {copy}
                  <IconArrowRight className="setup-checklist__chevron" />
                </Link>
              </li>
            );
          })}
        </ol>
      ) : null}
    </aside>
  );
}
