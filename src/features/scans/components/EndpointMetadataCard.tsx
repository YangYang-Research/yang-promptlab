import { Badge } from "@/shared/components";
import type { EndpointDto } from "@/shared/ipc";
import { endpointPath } from "../endpointMethod";
import { endpointPlatformLabel, endpointTypeLabel } from "../fingerprintPlan";
import { endpointSourceLabel } from "../discoveryPhases";

type EndpointMetadataCardProps = {
  endpoint: EndpointDto;
  selected: boolean;
  onToggle: () => void;
  onSelect: () => void;
};

function cap(enabled: boolean, label: string) {
  return (
    <span className={`endpoint-cap ${enabled ? "endpoint-cap--on" : "endpoint-cap--off"}`}>
      {label} {enabled ? "✓" : "✗"}
    </span>
  );
}

export function EndpointMetadataCard({
  endpoint,
  selected,
  onToggle,
  onSelect,
}: EndpointMetadataCardProps) {
  const caps = endpoint.metadata?.capabilities;
  const confidencePct = Math.round((endpoint.metadata_confidence || 0) * 100);

  return (
    <article
      className={`endpoint-metadata-card${selected ? " endpoint-metadata-card--selected" : ""}`}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onSelect();
      }}
      role="button"
      tabIndex={0}
    >
      <header className="endpoint-metadata-card__header">
        <label className="endpoint-metadata-card__check" onClick={(e) => e.stopPropagation()}>
          <input type="checkbox" checked={selected} onChange={onToggle} aria-label={`Select ${endpoint.url}`} />
        </label>
        <div className="endpoint-metadata-card__title">
          <span className="mono text-sm">
            {endpoint.method ?? "GET"} {endpointPath(endpoint.url)}
          </span>
          <span className="text-muted text-sm">
            {endpointPlatformLabel(endpoint)} ·{" "}
            {endpointTypeLabel(
              endpoint.endpoint_type ??
                endpoint.metadata?.classification.endpointType,
            )}
          </span>
        </div>
        <Badge variant="muted">{endpointSourceLabel(endpoint.kind, endpoint.source_url)}</Badge>
      </header>

      <dl className="endpoint-metadata-card__stats">
        <div>
          <dt>Confidence</dt>
          <dd>{confidencePct}%</dd>
        </div>
        <div>
          <dt>Risk</dt>
          <dd>{endpoint.risk_score}</dd>
        </div>
        <div>
          <dt>Auth</dt>
          <dd>{endpoint.auth_required ? "Required" : "Anonymous"}</dd>
        </div>
      </dl>

      {caps && (
        <div className="endpoint-metadata-card__caps">
          {cap(caps.supportsStreaming, "Streaming")}
          {cap(caps.supportsTools, "Tools")}
          {cap(caps.supportsVision, "Vision")}
          {cap(caps.supportsEmbedding, "Embedding")}
        </div>
      )}
    </article>
  );
}
