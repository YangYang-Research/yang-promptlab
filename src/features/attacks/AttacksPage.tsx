import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  DataTable,
  PageHeader,
  ProgressBar,
  StatusBadge,
} from "@/shared/components";
import type { AttackRun } from "@/shared/types";

const attackCategories = [
  { id: "prompt_injection", label: "Prompt Injection", desc: "Direct and indirect instruction override" },
  { id: "jailbreak", label: "Jailbreak", desc: "Safety filter bypass via persona/role-play" },
  { id: "data_exfiltration", label: "Data Exfiltration", desc: "Extract training data, PII, secrets" },
  { id: "system_prompt_leak", label: "System Prompt Leak", desc: "Recover hidden system instructions" },
  { id: "rag_poisoning", label: "RAG Poisoning", desc: "Indirect injection via retrieved context" },
  { id: "model_dos", label: "Model DoS", desc: "Resource exhaustion and token flooding" },
  { id: "insecure_output", label: "Insecure Output", desc: "XSS, code injection in completions" },
  { id: "excessive_agency", label: "Excessive Agency", desc: "Unauthorized tool/action invocation" },
  { id: "supply_chain", label: "Supply Chain", desc: "Plugin and dependency tampering" },
] as const;

export function AttacksPage() {
  const { attackRuns } = useAppStore();

  const columns = [
    {
      key: "target",
      header: "Target",
      render: (a: AttackRun) => (
        <div>
          <strong>{a.targetName}</strong>
          <div className="text-muted text-sm">{a.category.replace(/_/g, " ")}</div>
        </div>
      ),
    },
    {
      key: "progress",
      header: "Progress",
      width: "160px",
      render: (a: AttackRun) => (
        <ProgressBar
          value={a.payloadsRun}
          max={a.payloadsTotal}
          size="sm"
        />
      ),
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (a: AttackRun) => a.findingsCount,
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (a: AttackRun) => <StatusBadge status={a.status} />,
    },
    {
      key: "started",
      header: "Started",
      width: "160px",
      render: (a: AttackRun) => new Date(a.startedAt).toLocaleString(),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Attacks"
        description="OWASP LLM Top 10 aligned attack orchestration"
        actions={
          <>
            <Button variant="ghost">Playbook</Button>
            <Button variant="primary">Launch Attack</Button>
          </>
        }
      />

      <section className="attack-categories">
        <h3 className="section-title">Attack Categories</h3>
        <div className="attack-category-grid">
          {attackCategories.map((cat) => (
            <Card key={cat.id} className="attack-category-card" padding="sm">
              <h4>{cat.label}</h4>
              <p className="text-muted text-sm">{cat.desc}</p>
              <Button size="sm" variant="ghost">Configure</Button>
            </Card>
          ))}
        </div>
      </section>

      <section>
        <h3 className="section-title">Recent Runs</h3>
        <Card padding="none">
          <DataTable
            columns={columns}
            rows={attackRuns}
            keyField="id"
            emptyMessage="No attack runs yet"
          />
        </Card>
      </section>
    </div>
  );
}
