import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import {
  Button,
  PageHeader,
  RefreshButton,
  YazgBadge,
} from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import { useAppStore } from "@/app/store/AppStore";
import { toAppError } from "@/shared/errors";
import {
  listAttackCatalog,
  listAttackCatalogCategories,
  generateAttackCatalogPrompt,
  resetAttackCatalogTechnique,
  updateAttackCatalogTechnique,
  type AttackCatalogCategoryDto,
  type AttackCatalogTechniqueDto,
} from "@/shared/ipc/attackCatalog";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import { paginateItems } from "@/shared/utils/pagination";
import {
  matchesNistBrowse,
  NIST_BROWSE,
  nistRefsForTechnique,
  type NistBrowseId,
} from "./nistAiRmf";

const TECHNIQUES_PAGE_SIZE = 7;

type OwaspBrowseId = "all" | "llm" | "asi" | "mcp";

const OWASP_BROWSE: { id: OwaspBrowseId; label: string }[] = [
  { id: "all", label: "All" },
  { id: "llm", label: "LLM" },
  { id: "asi", label: "Agentic" },
  { id: "mcp", label: "MCP" },
];

function owaspFamily(owasp: string): "llm" | "asi" | "mcp" | "other" {
  const id = owasp.trim().toUpperCase();
  if (id.startsWith("LLM")) return "llm";
  if (id.startsWith("ASI")) return "asi";
  if (id.startsWith("MCP")) return "mcp";
  return "other";
}

/** Supports single or comma-separated OWASP IDs (e.g. `ASI02, LLM06`). */
function owaspFlags(owasp: string | null): string[] {
  if (!owasp) return [];
  return owasp
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
}

function matchesOwaspBrowse(row: AttackCatalogTechniqueDto, browse: OwaspBrowseId): boolean {
  if (browse === "all") return true;
  return owaspFlags(row.owasp).some((tag) => owaspFamily(tag) === browse);
}

export function AttackCategoriesPage() {
  const { techniqueId } = useParams<{ techniqueId?: string }>();
  const { backendConnected } = useAppStore();
  const appliedTechniqueId = useRef<string | null>(null);
  const [categories, setCategories] = useState<AttackCatalogCategoryDto[]>([]);
  const [techniques, setTechniques] = useState<AttackCatalogTechniqueDto[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedOwasp, setSelectedOwasp] = useState<OwaspBrowseId>("all");
  const [selectedNist, setSelectedNist] = useState<NistBrowseId>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [yazgGenerated, setYazgGenerated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedHint, setSavedHint] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!backendConnected) {
      setCategories([]);
      setTechniques([]);
      return;
    }
    const [cats, rows] = await Promise.all([
      listAttackCatalogCategories(),
      listAttackCatalog(),
    ]);
    setCategories(cats);
    setTechniques(rows);
  }, [backendConnected]);

  useEffect(() => {
    void load().catch(() => {
      setCategories([]);
      setTechniques([]);
    });
  }, [load]);

  const nistCounts = useMemo(() => {
    const inFamily = (row: AttackCatalogTechniqueDto) =>
      !selectedCategory || row.categoryId === selectedCategory;
    const counts: Record<NistBrowseId, number> = {
      all: 0,
      govern: 0,
      map: 0,
      measure: 0,
      manage: 0,
    };
    for (const row of techniques) {
      if (!inFamily(row)) continue;
      if (!matchesOwaspBrowse(row, selectedOwasp)) continue;
      const functions = new Set(
        nistRefsForTechnique(row).map((ref) => ref.functionId),
      );
      if (functions.has("govern")) counts.govern += 1;
      if (functions.has("map")) counts.map += 1;
      if (functions.has("measure")) counts.measure += 1;
      if (functions.has("manage")) counts.manage += 1;
      counts.all += 1;
    }
    return counts;
  }, [techniques, selectedOwasp, selectedCategory]);

  const owaspCounts = useMemo(() => {
    const inFamily = (row: AttackCatalogTechniqueDto) =>
      !selectedCategory || row.categoryId === selectedCategory;
    const counts: Record<OwaspBrowseId, number> = {
      all: 0,
      llm: 0,
      asi: 0,
      mcp: 0,
    };
    for (const row of techniques) {
      if (!inFamily(row)) continue;
      if (!matchesNistBrowse(row, selectedNist)) continue;
      const families = new Set(
        owaspFlags(row.owasp)
          .map(owaspFamily)
          .filter((family): family is "llm" | "asi" | "mcp" => family !== "other"),
      );
      if (families.has("llm")) counts.llm += 1;
      if (families.has("asi")) counts.asi += 1;
      if (families.has("mcp")) counts.mcp += 1;
      counts.all += 1;
    }
    return counts;
  }, [techniques, selectedNist, selectedCategory]);

  const activeCategory = useMemo(
    () => categories.find((cat) => cat.id === selectedCategory) ?? null,
    [categories, selectedCategory],
  );

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return techniques.filter((row) => {
      if (!matchesOwaspBrowse(row, selectedOwasp)) return false;
      if (!matchesNistBrowse(row, selectedNist)) return false;
      if (selectedCategory && row.categoryId !== selectedCategory) return false;
      if (!needle) return true;
      const nistLabels = nistRefsForTechnique(row)
        .map((ref) => ref.label.toLowerCase())
        .join(" ");
      return (
        row.name.toLowerCase().includes(needle) ||
        row.id.toLowerCase().includes(needle) ||
        (row.owasp?.toLowerCase().includes(needle) ?? false) ||
        nistLabels.includes(needle) ||
        (row.description?.toLowerCase().includes(needle) ?? false)
      );
    });
  }, [techniques, selectedCategory, selectedOwasp, selectedNist, query]);

  const pagination = useMemo(
    () => paginateItems(filtered, page, TECHNIQUES_PAGE_SIZE),
    [filtered, page],
  );

  useEffect(() => {
    setPage(1);
  }, [selectedCategory, selectedOwasp, selectedNist, query]);

  useEffect(() => {
    if (!techniqueId) {
      appliedTechniqueId.current = null;
      return;
    }
    if (techniques.length === 0) return;
    if (appliedTechniqueId.current === techniqueId) return;
    const row = techniques.find((item) => item.id === techniqueId);
    if (!row) return;
    appliedTechniqueId.current = techniqueId;
    setSelectedOwasp("all");
    setSelectedNist("all");
    setQuery("");
    setSelectedCategory(row.categoryId);
    setSelectedId(row.id);
  }, [techniqueId, techniques]);

  useEffect(() => {
    if (!selectedId) return;
    const index = filtered.findIndex((row) => row.id === selectedId);
    if (index < 0) return;
    const nextPage = Math.floor(index / TECHNIQUES_PAGE_SIZE) + 1;
    setPage((current) => (current === nextPage ? current : nextPage));
  }, [selectedId, filtered]);

  useEffect(() => {
    if (page > pagination.totalPages) {
      setPage(pagination.totalPages);
    }
  }, [page, pagination.totalPages]);

  const selected = useMemo(
    () => techniques.find((row) => row.id === selectedId) ?? null,
    [techniques, selectedId],
  );

  useEffect(() => {
    if (!selected) {
      setDraft("");
      setYazgGenerated(false);
      return;
    }
    setDraft(selected.content);
    setSavedHint(null);
    setYazgGenerated(false);
  }, [selected]);

  useEffect(() => {
    if (filtered.length === 0) {
      if (!techniqueId) setSelectedId(null);
      return;
    }
    if (!selectedId || !filtered.some((row) => row.id === selectedId)) {
      setSelectedId(pagination.items[0]?.id ?? filtered[0].id);
    }
  }, [filtered, selectedId, pagination.items, techniqueId]);

  async function handleSave() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await updateAttackCatalogTechnique(selected.id, {
        content: draft,
      });
      setTechniques((current) =>
        current.map((row) => (row.id === updated.id ? updated : row)),
      );
      setYazgGenerated(false);
      setSavedHint("Prompt saved");
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  async function handleReset() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await resetAttackCatalogTechnique(selected.id);
      setTechniques((current) =>
        current.map((row) => (row.id === updated.id ? updated : row)),
      );
      setDraft(updated.content);
      setYazgGenerated(false);
      setSavedHint("Reset to factory default");
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleEnabled() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await updateAttackCatalogTechnique(selected.id, {
        enabled: !selected.enabled,
      });
      setTechniques((current) =>
        current.map((row) => (row.id === updated.id ? updated : row)),
      );
      setCategories(await listAttackCatalogCategories());
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  async function handleGeneratePrompt() {
    if (!selected) return;
    setGenerating(true);
    setError(null);
    setSavedHint(null);
    try {
      const yazg = await assertYazgAgentLive(true);
      if (!yazg.live) {
        setError(yazg.message);
        return;
      }
      const generated = await generateAttackCatalogPrompt(selected.id);
      setDraft(generated.content);
      setYazgGenerated(true);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setGenerating(false);
    }
  }

  const dirty = selected ? draft !== selected.content : false;
  const actionsDisabled = busy || generating;

  const hasActiveFilters =
    selectedCategory !== null || selectedOwasp !== "all" || selectedNist !== "all";

  const owaspLabel =
    OWASP_BROWSE.find((item) => item.id === selectedOwasp)?.label ?? "All";
  const nistLabel =
    NIST_BROWSE.find((item) => item.id === selectedNist)?.label ?? "All";

  function clearBrowseFilters() {
    setSelectedCategory(null);
    setSelectedOwasp("all");
    setSelectedNist("all");
    setQuery("");
    setPage(1);
  }

  const attackAllCount = techniques.filter(
    (row) =>
      matchesOwaspBrowse(row, selectedOwasp) && matchesNistBrowse(row, selectedNist),
  ).length;

  return (
    <div className="page attack-categories-page">
      <PageHeader
        title="Attack Factory"
        description="Factory prompts for static pack and mutation generation."
        actions={
          <RefreshButton
            loading={busy}
            error={error}
            disabled={!backendConnected}
            onClick={() => void load().catch((err) => setError(toAppError(err).message))}
          />
        }
      />

      {!backendConnected ? (
        <section className="attack-catalog-empty">
          <p className="attack-catalog-empty__title">Backend required</p>
          <p className="text-muted">
            Connect to the Tauri backend to load and edit attack techniques.
          </p>
        </section>
      ) : (
        <div className="attack-catalog">
          <section className="attack-catalog-filter" aria-label="Browse techniques">
            <div className="attack-catalog-filter__head">
              <div className="attack-catalog-filter__intro">
                <h2 className="attack-catalog-filter__title">Find techniques</h2>
              </div>
              <p className="attack-catalog-filter__meta" aria-live="polite">
                Showing{" "}
                <strong className="attack-catalog-filter__meta-count">{filtered.length}</strong>
                {filtered.length === 1 ? " technique" : " techniques"}
              </p>
            </div>

            {hasActiveFilters ? (
              <div className="attack-catalog-filter__active" aria-label="Active filters">
                <div className="attack-catalog-filter__active-tags">
                  {activeCategory ? (
                    <button
                      type="button"
                      className="attack-catalog-filter__tag"
                      onClick={() => {
                        setSelectedCategory(null);
                        setPage(1);
                      }}
                    >
                      {activeCategory.label}
                      <span aria-hidden="true">×</span>
                      <span className="sr-only">Remove category filter</span>
                    </button>
                  ) : null}
                  {selectedOwasp !== "all" ? (
                    <button
                      type="button"
                      className="attack-catalog-filter__tag"
                      onClick={() => {
                        setSelectedOwasp("all");
                        setPage(1);
                      }}
                    >
                      OWASP · {owaspLabel}
                      <span aria-hidden="true">×</span>
                      <span className="sr-only">Remove OWASP filter</span>
                    </button>
                  ) : null}
                  {selectedNist !== "all" ? (
                    <button
                      type="button"
                      className="attack-catalog-filter__tag"
                      onClick={() => {
                        setSelectedNist("all");
                        setPage(1);
                      }}
                    >
                      NIST · {nistLabel}
                      <span aria-hidden="true">×</span>
                      <span className="sr-only">Remove NIST filter</span>
                    </button>
                  ) : null}
                </div>
                <button
                  type="button"
                  className="attack-catalog-filter__clear"
                  onClick={clearBrowseFilters}
                >
                  Clear filters
                </button>
              </div>
            ) : null}

            <div className="attack-catalog-filter__group attack-catalog-filter__group--primary">
              <div className="attack-catalog-filter__group-head">
                <p className="attack-catalog-filter__group-label">Attack family</p>
                <p className="attack-catalog-filter__group-help">Primary browse</p>
              </div>
              <div
                className="attack-catalog-filter__chips attack-catalog-filter__chips--primary"
                role="tablist"
                aria-label="Attack categories"
              >
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedCategory === null}
                  className={
                    selectedCategory === null
                      ? "filter-chip filter-chip--active attack-catalog-filter__chip"
                      : "filter-chip attack-catalog-filter__chip"
                  }
                  onClick={() => {
                    setSelectedCategory(null);
                    setQuery("");
                    setPage(1);
                  }}
                >
                  All families
                  <span className="attack-catalog-filter__count">{attackAllCount}</span>
                </button>
                {categories.map((cat) => {
                  const count = techniques.filter(
                    (row) =>
                      row.categoryId === cat.id &&
                      matchesOwaspBrowse(row, selectedOwasp) &&
                      matchesNistBrowse(row, selectedNist),
                  ).length;
                  return (
                    <button
                      key={cat.id}
                      type="button"
                      role="tab"
                      aria-selected={cat.id === selectedCategory}
                      className={
                        cat.id === selectedCategory
                          ? "filter-chip filter-chip--active attack-catalog-filter__chip"
                          : "filter-chip attack-catalog-filter__chip"
                      }
                      onClick={() => {
                        setSelectedCategory(cat.id);
                        setQuery("");
                        setPage(1);
                      }}
                    >
                      {cat.label}
                      <span className="attack-catalog-filter__count">{count}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="attack-catalog-filter__standards">
              <div className="attack-catalog-filter__group">
                <div className="attack-catalog-filter__group-head">
                  <p className="attack-catalog-filter__group-label">OWASP</p>
                  <p className="attack-catalog-filter__group-help">Risk taxonomy</p>
                </div>
                <div
                  className="attack-catalog-filter__chips"
                  role="tablist"
                  aria-label="OWASP families"
                >
                  {OWASP_BROWSE.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      role="tab"
                      aria-selected={item.id === selectedOwasp}
                      className={
                        item.id === selectedOwasp
                          ? `filter-chip filter-chip--active filter-chip--owasp-${item.id} attack-catalog-filter__chip attack-catalog-filter__chip--compact`
                          : `filter-chip filter-chip--owasp-${item.id} attack-catalog-filter__chip attack-catalog-filter__chip--compact`
                      }
                      onClick={() => {
                        setSelectedOwasp(item.id);
                        setQuery("");
                        setPage(1);
                      }}
                    >
                      {item.label}
                      <span className="attack-catalog-filter__count">{owaspCounts[item.id]}</span>
                    </button>
                  ))}
                </div>
              </div>

              <div className="attack-catalog-filter__group">
                <div className="attack-catalog-filter__group-head">
                  <p className="attack-catalog-filter__group-label">NIST AI RMF</p>
                  <p className="attack-catalog-filter__group-help">Governance functions</p>
                </div>
                <div
                  className="attack-catalog-filter__chips"
                  role="tablist"
                  aria-label="NIST AI RMF functions"
                >
                  {NIST_BROWSE.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      role="tab"
                      aria-selected={item.id === selectedNist}
                      className={
                        item.id === selectedNist
                          ? `filter-chip filter-chip--active filter-chip--nist-${item.id} attack-catalog-filter__chip attack-catalog-filter__chip--compact`
                          : `filter-chip filter-chip--nist-${item.id} attack-catalog-filter__chip attack-catalog-filter__chip--compact`
                      }
                      onClick={() => {
                        setSelectedNist(item.id);
                        setQuery("");
                        setPage(1);
                      }}
                    >
                      {item.label}
                      <span className="attack-catalog-filter__count">{nistCounts[item.id]}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </section>

          <section className="attack-catalog-workspace" aria-label="Technique editor">
            <aside className="attack-catalog-rail">
              <div className="attack-catalog-rail__head">
                <h2 className="attack-catalog-rail__title">Techniques</h2>
                <span className="attack-catalog-rail__count">{filtered.length}</span>
              </div>
              <label className="attack-catalog-rail__search">
                <span className="sr-only">Search techniques</span>
                <input
                  type="search"
                  value={query}
                  placeholder="Search name, id, OWASP, NIST…"
                  onChange={(event) => setQuery(event.target.value)}
                />
              </label>
              {filtered.length === 0 ? (
                <p className="attack-catalog-rail__empty text-muted">No techniques match.</p>
              ) : (
                <>
                  <ul className="attack-catalog-rail__list">
                    {pagination.items.map((row) => (
                      <li key={row.id}>
                        <button
                          type="button"
                          className={
                            row.id === selectedId
                              ? "attack-catalog-rail__item is-active"
                              : "attack-catalog-rail__item"
                          }
                          onClick={() => setSelectedId(row.id)}
                        >
                          <span className="attack-catalog-rail__item-name">{row.name}</span>
                          <span className="attack-catalog-rail__item-meta">
                            {owaspFlags(row.owasp).map((tag) => (
                              <span
                                key={tag}
                                className={`attack-catalog-flag attack-catalog-flag--owasp attack-catalog-flag--${owaspFamily(tag)}`}
                              >
                                {tag}
                              </span>
                            ))}
                            {nistRefsForTechnique(row).map((ref) => (
                              <span
                                key={ref.label}
                                className={`attack-catalog-flag attack-catalog-flag--nist attack-catalog-flag--nist-${ref.functionId}`}
                                title={`NIST AI RMF: ${ref.label}`}
                              >
                                {ref.label}
                              </span>
                            ))}
                            {!row.enabled ? (
                              <span className="attack-catalog-flag attack-catalog-flag--off">
                                Off
                              </span>
                            ) : null}
                            {row.userModified ? (
                              <span className="attack-catalog-flag attack-catalog-flag--edited">
                                Edited
                              </span>
                            ) : null}
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                  {pagination.totalPages > 1 ? (
                    <div className="attack-catalog-rail__pager">
                      <button
                        type="button"
                        className="attack-catalog-rail__pager-btn"
                        disabled={pagination.page <= 1}
                        onClick={() => setPage((current) => Math.max(1, current - 1))}
                      >
                        Prev
                      </button>
                      <span className="attack-catalog-rail__pager-meta">
                        {pagination.page}/{pagination.totalPages}
                      </span>
                      <button
                        type="button"
                        className="attack-catalog-rail__pager-btn"
                        disabled={pagination.page >= pagination.totalPages}
                        onClick={() =>
                          setPage((current) =>
                            Math.min(pagination.totalPages, current + 1),
                          )
                        }
                      >
                        Next
                      </button>
                    </div>
                  ) : null}
                </>
              )}
            </aside>

            <div className="attack-catalog-editor">
              {selected ? (
                <>
                  <header className="attack-catalog-editor__identity">
                    <div className="attack-catalog-editor__identity-text">
                      <p className="attack-catalog-editor__eyebrow">
                        {activeCategory?.label ?? selected.categoryId}
                      </p>
                      <h2 className="attack-catalog-editor__title">{selected.name}</h2>
                      <p className="attack-catalog-editor__id">{selected.id}</p>
                      {selected.description ? (
                        <p className="attack-catalog-editor__desc">{selected.description}</p>
                      ) : null}
                    </div>
                    <div className="attack-catalog-editor__status">
                      {owaspFlags(selected.owasp).map((tag) => (
                        <span
                          key={tag}
                          className={`attack-catalog-flag attack-catalog-flag--owasp attack-catalog-flag--${owaspFamily(tag)}`}
                        >
                          {tag}
                        </span>
                      ))}
                      {nistRefsForTechnique(selected).map((ref) => (
                        <span
                          key={ref.label}
                          className={`attack-catalog-flag attack-catalog-flag--nist attack-catalog-flag--nist-${ref.functionId}`}
                          title={`NIST AI RMF: ${ref.label}`}
                        >
                          {ref.label}
                        </span>
                      ))}
                      <span
                        className={
                          selected.enabled
                            ? "attack-catalog-flag attack-catalog-flag--on"
                            : "attack-catalog-flag attack-catalog-flag--off"
                        }
                      >
                        {selected.enabled ? "Enabled" : "Disabled"}
                      </span>
                      {selected.userModified ? (
                        <span className="attack-catalog-flag attack-catalog-flag--edited">
                          Custom prompt
                        </span>
                      ) : (
                        <span className="attack-catalog-flag attack-catalog-flag--factory">
                          Factory default
                        </span>
                      )}
                    </div>
                  </header>

                  <div className="attack-catalog-editor__prompt">
                    <div className="attack-catalog-editor__prompt-head">
                      <label htmlFor="attack-prompt">
                        Default prompt
                        {yazgGenerated ? (
                          <YazgBadge className="attack-catalog-editor__yazg-badge" />
                        ) : null}
                      </label>
                      <div className="attack-catalog-editor__prompt-head-actions">
                        {!generating && dirty ? (
                          <span className="attack-catalog-editor__dirty">Unsaved changes</span>
                        ) : null}
                        <Button
                          variant="primary"
                          className="attack-catalog-generate-btn"
                          disabled={!backendConnected || actionsDisabled}
                          onClick={() => void handleGeneratePrompt()}
                        >
                          <span className="btn__content">
                            <IconAi className="btn__icon" aria-hidden />
                            {generating ? "Yazg is generating…" : "Generate new prompt"}
                          </span>
                        </Button>
                      </div>
                    </div>
                    <textarea
                      id="attack-prompt"
                      className="attack-catalog-editor__textarea"
                      value={draft}
                      disabled={generating}
                      onChange={(event) => {
                        setDraft(event.target.value);
                        setSavedHint(null);
                      }}
                      rows={18}
                      spellCheck={false}
                    />
                  </div>

                  <footer className="attack-catalog-editor__footer">
                    <div className="attack-catalog-editor__feedback">
                      {savedHint ? <p className="text-muted">{savedHint}</p> : null}
                      {error ? <p className="form-error">{error}</p> : null}
                    </div>
                    <div className="attack-catalog-editor__actions">
                      <Button
                        variant="ghost"
                        disabled={actionsDisabled}
                        onClick={() => void handleToggleEnabled()}
                      >
                        {selected.enabled ? "Disable" : "Enable"}
                      </Button>
                      <Button
                        variant="secondary"
                        disabled={actionsDisabled || !selected.userModified}
                        onClick={() => void handleReset()}
                      >
                        Reset default
                      </Button>
                      <Button
                        variant="primary"
                        disabled={actionsDisabled || !dirty}
                        onClick={() => void handleSave()}
                      >
                        Save prompt
                      </Button>
                    </div>
                  </footer>
                </>
              ) : (
                <div className="attack-catalog-editor__placeholder">
                  <p className="attack-catalog-empty__title">Select a technique</p>
                  <p className="text-muted">
                    Choose a category above, then pick a technique to edit its prompt.
                  </p>
                </div>
              )}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
