import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
  ProgressBar,
  StatusBadge,
} from "@/shared/components";

export function DiscoveryPage() {
  const { discoveryJobs } = useAppStore();

  return (
    <div className="page">
      <PageHeader
        title="Discovery"
        description="Attack surface enumeration — crawl, API, OpenAPI, AI fingerprinting"
        actions={
          <>
            <Button variant="ghost">Configure Modules</Button>
            <Button variant="primary">Start Scan</Button>
          </>
        }
      />

      <div className="discovery-grid">
        {discoveryJobs.map((job) => (
          <Card key={job.id} className="discovery-card">
            <div className="discovery-card__header">
              <div>
                <h3 className="discovery-card__title">{job.targetName}</h3>
                <p className="text-muted text-sm">
                  Started {new Date(job.startedAt).toLocaleString()}
                </p>
              </div>
              <StatusBadge status={job.status} />
            </div>

            <ProgressBar value={job.progress} label="Progress" />

            <div className="discovery-card__stats">
              <div>
                <span className="discovery-card__stat-value">{job.endpointsFound}</span>
                <span className="discovery-card__stat-label">Endpoints</span>
              </div>
              <div>
                <span className="discovery-card__stat-value">{job.modules.length}</span>
                <span className="discovery-card__stat-label">Modules</span>
              </div>
            </div>

            <div className="tag-list">
              {job.modules.map((mod) => (
                <span key={mod} className="chip">{mod}</span>
              ))}
            </div>

            <div className="discovery-card__actions">
              {job.status === "running" && <Button size="sm" variant="ghost">Pause</Button>}
              {job.status === "completed" && (
                <Button size="sm" variant="primary">View Results</Button>
              )}
              {job.status === "pending" && (
                <Button size="sm" variant="primary">Run Now</Button>
              )}
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
