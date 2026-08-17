import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import {
  ActionsDropdown,
  Badge,
  Button,
  Card,
  EmptyState,
  FindingStatusBadge,
  IconStatus,
  IconTrash,
  PageHeader,
  PageLoadingSkeleton,
  SeverityBadge,
  YazgBadge,
  type ActionsDropdownItem,
} from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import { rejudgeFinding } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Finding } from "@/shared/types";

import { FindingDetailPanel } from "./FindingDetailPanel";
import { FindingRecommendationsPanel } from "./FindingRecommendationsPanel";
import { UpdateFindingStatusModal } from "./UpdateFindingStatusModal";
import { complianceRefsFor } from "./complianceRefs";
import { parseFindingEvidence } from "./findingEvidence";

export function FindingDetailsPage() {
  const { findingId = "" } = useParams();
  const navigate = useNavigate();
  const { findings, projects, scans, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [statusModalOpen, setStatusModalOpen] = useState(false);
  const [busy, setBusy] = useState<"status" | "delete" | "rejudge" | null>(null);

  const finding = findings.find((item) => item.id === findingId);
  const project = projects.find((item) => item.id === finding?.projectId);
  const scan = scans.find((item) => item.id === finding?.scanId);
  const judgedAt = useMemo(
    () => (finding ? parseFindingEvidence(finding).judgedAt : null),
    [finding],
  );

  const handleUpdateStatus = useCallback(
    async (status: Finding["status"]) => {
      if (!finding) return;
      setBusy("status");
      try {
        await actions.updateFindingStatus(finding.id, status);
        notify("Finding status updated", "success");
        setStatusModalOpen(false);
      } catch (error) {
        notify(error instanceof Error ? error.message : "Failed to update status", "error");
      } finally {
        setBusy(null);
      }
    },
    [actions, finding, notify],
  );

  const handleDelete = useCallback(async () => {
    if (!finding) return;
    const confirmed = window.confirm(
      `Delete finding "${finding.title}"? This cannot be undone.`,
    );
    if (!confirmed) return;

    setBusy("delete");
    try {
      await actions.deleteFinding(finding.id);
      notify("Finding deleted", "success");
      navigate("/findings");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Failed to delete finding", "error");
      setBusy(null);
    }
  }, [actions, finding, navigate, notify]);

  const handleRejudge = useCallback(async () => {
    if (!finding) return;
    setBusy("rejudge");
    try {
      await rejudgeFinding(finding.id);
      await actions.refresh();
      notify("Finding re-judged", "success");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Failed to re-judge finding", "error");
    } finally {
      setBusy(null);
    }
  }, [actions, finding, notify]);

  const actionItems = useMemo((): ActionsDropdownItem[] => {
    if (!finding) return [];
    return [
      {
        id: "update-status",
        label: "Update status",
        icon: <IconStatus />,
        disabled: busy !== null,
        onClick: () => setStatusModalOpen(true),
      },
      {
        id: "delete",
        label: "Delete finding",
        icon: <IconTrash />,
        tone: "danger",
        disabled: busy !== null,
        onClick: () => void handleDelete(),
      },
    ];
  }, [busy, finding, handleDelete]);

  if (!finding && !loading) {
    return (
      <div className="page">
        <PageHeader title="Finding details" backTo="/findings" />
        <EmptyState
          title="Finding not found"
          description="This finding may have been deleted or is no longer available."
        />
      </div>
    );
  }

  if (!finding) {
    return (
      <div className="page finding-details">
        <PageHeader title="Finding details" backTo="/findings" />
        <PageLoadingSkeleton />
      </div>
    );
  }

  const confidencePct = Math.round(finding.confidence * 100);
  const targetLabel = finding.targetName || finding.targetUrl || "—";
  const complianceRefs = complianceRefsFor(finding.category);

  return (
    <div className="page finding-details">
      <PageHeader
        backTo="/findings"
        backOnly
        title="Finding details"
        actions={
          <div className="page-actions">
            <ActionsDropdown
              label="Finding actions"
              disabled={busy !== null}
              items={actionItems}
            />
          </div>
        }
      />

      <UpdateFindingStatusModal
        open={statusModalOpen}
        currentStatus={finding.status}
        submitting={busy === "status"}
        onClose={() => {
          if (busy !== "status") setStatusModalOpen(false);
        }}
        onSubmit={handleUpdateStatus}
      />

      <section className="finding-details__overview" aria-label="Finding overview">
        <Card className="detail-section finding-details__context">
          <h2 className="detail-section__title">Finding Information</h2>
          <dl className="finding-details__meta">
            <div className="finding-details__meta-row finding-details__meta-row--wide">
              <dt>Title</dt>
              <dd>{finding.title}</dd>
            </div>
            <div className="finding-details__meta-row">
              <dt>Project</dt>
              <dd>
                {project ? (
                  <Link to={`/projects/${project.id}`} className="link">
                    {project.name}
                  </Link>
                ) : (
                  "—"
                )}
              </dd>
            </div>
            <div className="finding-details__meta-row">
              <dt>Scan ID</dt>
              <dd>
                {scan ? (
                  <Link to={`/scans/${scan.id}`} className="link mono text-sm">
                    {scan.id}
                  </Link>
                ) : (
                  <span className="mono text-sm">{finding.scanId}</span>
                )}
              </dd>
            </div>
            <div className="finding-details__meta-row">
              <dt>Finding ID</dt>
              <dd>
                <span className="mono text-sm">{finding.id}</span>
              </dd>
            </div>
            <div className="finding-details__meta-row">
              <dt>Target</dt>
              <dd>
                {finding.targetId ? (
                  <Link to={`/targets/${finding.targetId}`} className="link">
                    {targetLabel}
                  </Link>
                ) : (
                  targetLabel
                )}
              </dd>
            </div>
            <div className="finding-details__meta-row finding-details__meta-row--wide">
              <dt>Endpoint</dt>
              <dd>
                {finding.targetUrl ? (
                  <span className="mono text-sm" title={finding.targetUrl}>
                    {finding.targetUrl}
                  </span>
                ) : (
                  "—"
                )}
              </dd>
            </div>
          </dl>
        </Card>

        <Card className="detail-section finding-details__signal">
          <h2 className="detail-section__title">Assessment</h2>
          <div className="finding-details__signal-grid">
            <div className="finding-details__signal-item">
              <span className="finding-details__signal-label">Severity</span>
              <SeverityBadge severity={finding.severity} />
            </div>
            <div className="finding-details__signal-item">
              <span className="finding-details__signal-label">Verdict</span>
              {finding.verdict ? (
                <Badge variant={finding.verdict === "vulnerable" ? "danger" : "muted"}>
                  {finding.verdict === "vulnerable" ? "Vulnerable" : "Not vulnerable"}
                </Badge>
              ) : (
                <span className="text-muted">—</span>
              )}
            </div>
            <div className="finding-details__signal-item">
              <span className="finding-details__signal-label">Status</span>
              <FindingStatusBadge status={finding.status} />
            </div>
            <div className="finding-details__signal-item finding-details__signal-item--compliance">
              <span className="finding-details__signal-label">Compliance</span>
              <div className="finding-details__compliance-list">
                {complianceRefs.map((ref) => (
                  <Badge key={ref} variant="info">
                    {ref}
                  </Badge>
                ))}
              </div>
            </div>
            <div className="finding-details__signal-item finding-details__signal-item--confidence">
              <div className="finding-details__confidence-head">
                <span className="finding-details__signal-label">Confidence</span>
                <span className="finding-details__confidence-value">{confidencePct}%</span>
              </div>
              <div
                className="finding-details__confidence-track"
                role="meter"
                aria-label="Confidence"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={confidencePct}
              >
                <div
                  className="finding-details__confidence-fill"
                  style={{ width: `${confidencePct}%` }}
                />
              </div>
            </div>
          </div>
        </Card>
      </section>

      <section className="finding-details__poc" aria-label="Proof of Concept">
        <Card className="detail-section finding-details__panel">
          <h2 className="detail-section__title">Proof of Concept (PoC)</h2>
          <FindingDetailPanel finding={finding} embedded mode="poc" />
        </Card>
      </section>

      <section className="finding-details__judge" aria-label="Judging Analysis">
        <Card className="detail-section finding-details__panel">
          <div className="detail-section__header">
            <h2 className="detail-section__title">Judging Analysis</h2>
            <div className="detail-section__header-actions">
              <YazgBadge pulsing={busy === "rejudge"} />
            </div>
          </div>
          <FindingDetailPanel finding={finding} embedded mode="judge" />
          <div className="project-summary__footer finding-details__judge-footer">
            {judgedAt || finding.discoveredAt ? (
              <p className="project-summary__generated">
                Generated {formatTimestamp(judgedAt ?? finding.discoveredAt)}
              </p>
            ) : (
              <span />
            )}
            <Button
              variant="primary"
              size="sm"
              type="button"
              className="project-summary__action"
              onClick={() => void handleRejudge()}
              disabled={busy !== null}
            >
              <span className="btn__content">
                <IconAi className="btn__icon" aria-hidden />
                {busy === "rejudge" ? "Re-judging…" : "Re-judge"}
              </span>
            </Button>
          </div>
        </Card>
      </section>

      <section className="finding-details__recommendations" aria-label="Recommendations">
        <Card className="detail-section finding-details__panel">
          <FindingRecommendationsPanel finding={finding} variant="section" />
        </Card>
      </section>
    </div>
  );
}
