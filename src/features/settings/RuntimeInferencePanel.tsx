import { Card } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";

import { RUNTIME_INFERENCE_SITES } from "./runtimeInferenceSites";

export function RuntimeInferencePanel() {
  return (
    <Card>
      <ul className="ai-applied-list">
        {RUNTIME_INFERENCE_SITES.map((site) => (
          <li key={site.id} className="ai-applied-list__item">
            <span className="ai-applied-list__icon-wrap" aria-hidden="true">
              <IconAi className="ai-applied-list__icon" />
            </span>
            <div className="ai-applied-list__body">
              <div className="ai-applied-list__header">
                <strong className="ai-applied-list__title">{site.title}</strong>
                <span className="ai-applied-list__location">{site.location}</span>
              </div>
              <p className="ai-applied-list__description text-muted text-sm">{site.description}</p>
            </div>
          </li>
        ))}
      </ul>
    </Card>
  );
}
