import type { EndpointDto } from "@/shared/ipc";
import { endpointPath } from "../endpointMethod";
import { endpointPlatformLabel, endpointTypeLabel } from "../fingerprintPlan";

type EndpointDetailsPanelProps = {
  endpoint: EndpointDto | null;
};

export function EndpointDetailsPanel({ endpoint }: EndpointDetailsPanelProps) {
  if (!endpoint?.metadata) {
    return (
      <div className="endpoint-details-panel endpoint-details-panel--empty">
        <p className="text-muted text-sm">Select an endpoint to inspect AI metadata.</p>
      </div>
    );
  }

  const m = endpoint.metadata;
  const caps = m.capabilities;

  return (
    <div className="endpoint-details-panel">
      <h4 className="endpoint-details-panel__title">
        {endpoint.method ?? "GET"} {endpointPath(endpoint.url)}
      </h4>

      <section>
        <h5>Fingerprint</h5>
        <dl className="runtime-kv-grid">
          <div><dt>Framework</dt><dd>{m.fingerprint.framework || "—"}</dd></div>
          <div><dt>Provider</dt><dd>{m.fingerprint.provider || "—"}</dd></div>
          <div><dt>Version</dt><dd>{m.fingerprint.version || "—"}</dd></div>
          <div><dt>API style</dt><dd>{m.fingerprint.apiStyle}</dd></div>
        </dl>
      </section>

      <section>
        <h5>Schema inference</h5>
        <dl className="runtime-kv-grid">
          <div><dt>Prompt field</dt><dd>{m.inference.promptField ?? "—"}</dd></div>
          <div><dt>History field</dt><dd>{m.inference.historyField ?? "—"}</dd></div>
          <div><dt>Conversation field</dt><dd>{m.inference.conversationField ?? "—"}</dd></div>
          <div><dt>Model field</dt><dd>{m.inference.modelField ?? "—"}</dd></div>
          <div><dt>Stream field</dt><dd>{m.inference.streamField ?? "—"}</dd></div>
        </dl>
      </section>

      <section>
        <h5>Classification</h5>
        <dl className="runtime-kv-grid">
          <div><dt>Type</dt><dd>{endpointTypeLabel(endpoint.endpoint_type ?? endpoint.metadata?.classification.endpointType)}</dd></div>
          <div><dt>Framework</dt><dd>{endpointPlatformLabel(endpoint)}</dd></div>
          <div><dt>Risk score</dt><dd>{endpoint.risk_score}</dd></div>
          <div><dt>Discovery source</dt><dd>{endpoint.discovery_source}</dd></div>
        </dl>
      </section>

      <section>
        <h5>Capabilities</h5>
        <ul className="endpoint-details-panel__caps">
          <li>Chat: {caps.supportsChat ? "yes" : "no"}</li>
          <li>Streaming: {caps.supportsStreaming ? "yes" : "no"}</li>
          <li>Tools: {caps.supportsTools ? "yes" : "no"}</li>
          <li>Agent: {caps.supportsAgent ? "yes" : "no"}</li>
          <li>Memory: {caps.supportsMemory ? "yes" : "no"}</li>
        </ul>
      </section>

      {m.raw && (
        <section>
          <h5>Raw observation</h5>
          {m.raw.requestBody && (
            <pre className="endpoint-details-panel__raw">{m.raw.requestBody}</pre>
          )}
          {m.raw.responseBody && (
            <pre className="endpoint-details-panel__raw">{m.raw.responseBody.slice(0, 2000)}</pre>
          )}
        </section>
      )}
    </div>
  );
}
