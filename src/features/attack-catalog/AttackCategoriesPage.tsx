import { useCallback, useEffect, useMemo, useState } from "react";

import {
  Button,
  PageHeader,
  RefreshButton,
} from "@/shared/components";
import { useAppStore } from "@/app/store/AppStore";
import { toAppError } from "@/shared/errors";
import {
  listAttackCatalog,
  listAttackCatalogCategories,
  resetAttackCatalogTechnique,
  updateAttackCatalogTechnique,
  type AttackCatalogCategoryDto,
  type AttackCatalogTechniqueDto,
} from "@/shared/ipc/attackCatalog";
import { paginateItems } from "@/shared/utils/pagination";

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
  const { backendConnected } = useAppStore();
  const [categories, setCategories] = useState<AttackCatalogCategoryDto[]>([]);
  const [techniques, setTechniques] = useState<AttackCatalogTechniqueDto[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedOwasp, setSelectedOwasp] = useState<OwaspBrowseId>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
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

  const stats = useMemo(() => {
    const enabled = techniques.filter((row) => row.enabled).length;
    const modified = techniques.filter((row) => row.userModified).length;
    return {
      total: techniques.length,
      enabled,
      modified,
      categories: categories.length,
    };
  }, [techniques, categories.length]);

  const owaspCounts = useMemo(() => {
    const counts: Record<OwaspBrowseId, number> = {
      all: techniques.length,
      llm: 0,
      asi: 0,
      mcp: 0,
    };
    for (const row of techniques) {
      const families = new Set(
        owaspFlags(row.owasp)
          .map(owaspFamily)
          .filter((family): family is "llm" | "asi" | "mcp" => family !== "other"),
      );
      if (families.has("llm")) counts.llm += 1;
      if (families.has("asi")) counts.asi += 1;
      if (families.has("mcp")) counts.mcp += 1;
    }
    return counts;
  }, [techniques]);

  const activeCategory = useMemo(
    () => categories.find((cat) => cat.id === selectedCategory) ?? null,
    [categories, selectedCategory],
  );

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return techniques.filter((row) => {
      if (!matchesOwaspBrowse(row, selectedOwasp)) return false;
      if (selectedCategory && row.categoryId !== selectedCategory) return false;
      if (!needle) return true;
      return (
        row.name.toLowerCase().includes(needle) ||
        row.id.toLowerCase().includes(needle) ||
        (row.owasp?.toLowerCase().includes(needle) ?? false) ||
        (row.description?.toLowerCase().includes(needle) ?? false)
      );
    });
  }, [techniques, selectedCategory, selectedOwasp, query]);

  const pagination = useMemo(
    () => paginateItems(filtered, page, TECHNIQUES_PAGE_SIZE),
    [filtered, page],
  );

  useEffect(() => {
    setPage(1);
  }, [selectedCategory, selectedOwasp, query]);

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
      return;
    }
    setDraft(selected.content);
    setSavedHint(null);
  }, [selected]);

  useEffect(() => {
    if (filtered.length === 0) {
      setSelectedId(null);
      return;
    }
    if (!selectedId || !filtered.some((row) => row.id === selectedId)) {
      setSelectedId(pagination.items[0]?.id ?? filtered[0].id);
    }
  }, [filtered, selectedId, pagination.items]);

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

  const dirty = selected ? draft !== selected.content : false;

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
          <section className="attack-catalog-summary" aria-label="Catalog summary">
            <div className="attack-catalog-summary__stat">
              <span className="attack-catalog-summary__label">Techniques</span>
              <span className="attack-catalog-summary__value">{stats.total}</span>
            </div>
            <div className="attack-catalog-summary__stat">
              <span className="attack-catalog-summary__label">Enabled</span>
              <span className="attack-catalog-summary__value">{stats.enabled}</span>
            </div>
            <div className="attack-catalog-summary__stat">
              <span className="attack-catalog-summary__label">Modified</span>
              <span className="attack-catalog-summary__value">{stats.modified}</span>
            </div>
            <div className="attack-catalog-summary__stat">
              <span className="attack-catalog-summary__label">Categories</span>
              <span className="attack-catalog-summary__value">{stats.categories}</span>
            </div>
          </section>

          <section className="attack-catalog-filter" aria-label="Category filter">
            <div className="attack-catalog-filter__head">
              <h2 className="attack-catalog-filter__title">Browse by category</h2>
              <p className="attack-catalog-filter__meta">
                {filtered.length} shown
                {activeCategory ? ` · ${activeCategory.label}` : ""}
                {selectedOwasp !== "all"
                  ? ` · ${OWASP_BROWSE.find((item) => item.id === selectedOwasp)?.label}`
                  : ""}
              </p>
            </div>

            <div className="attack-catalog-filter__group">
              <p className="attack-catalog-filter__group-label">OWASP</p>
              <div className="attack-catalog-filter__chips" role="tablist" aria-label="OWASP families">
                {OWASP_BROWSE.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    role="tab"
                    aria-selected={item.id === selectedOwasp}
                    className={
                      item.id === selectedOwasp
                        ? `filter-chip filter-chip--active filter-chip--owasp-${item.id}`
                        : `filter-chip filter-chip--owasp-${item.id}`
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
              <p className="attack-catalog-filter__group-label">Attack</p>
              <div className="attack-catalog-filter__chips" role="tablist" aria-label="Attack categories">
                <button
                  type="button"
                  role="tab"
                  aria-selected={selectedCategory === null}
                  className={
                    selectedCategory === null
                      ? "filter-chip filter-chip--active"
                      : "filter-chip"
                  }
                  onClick={() => {
                    setSelectedCategory(null);
                    setQuery("");
                    setPage(1);
                  }}
                >
                  All
                  <span className="attack-catalog-filter__count">
                    {selectedOwasp === "all"
                      ? techniques.length
                      : techniques.filter((row) => matchesOwaspBrowse(row, selectedOwasp)).length}
                  </span>
                </button>
                {categories.map((cat) => {
                  const count =
                    selectedOwasp === "all"
                      ? cat.techniqueCount
                      : techniques.filter(
                          (row) =>
                            row.categoryId === cat.id &&
                            matchesOwaspBrowse(row, selectedOwasp),
                        ).length;
                  return (
                    <button
                      key={cat.id}
                      type="button"
                      role="tab"
                      aria-selected={cat.id === selectedCategory}
                      className={
                        cat.id === selectedCategory
                          ? "filter-chip filter-chip--active"
                          : "filter-chip"
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
                  placeholder="Search name, id, OWASP…"
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
                        {owaspFlags(selected.owasp).length > 0
                          ? ` · ${owaspFlags(selected.owasp).join(" · ")}`
                          : ""}
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
                      <label htmlFor="attack-prompt">Default prompt</label>
                      {dirty ? (
                        <span className="attack-catalog-editor__dirty">Unsaved changes</span>
                      ) : null}
                    </div>
                    <textarea
                      id="attack-prompt"
                      className="attack-catalog-editor__textarea"
                      value={draft}
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
                        disabled={busy}
                        onClick={() => void handleToggleEnabled()}
                      >
                        {selected.enabled ? "Disable" : "Enable"}
                      </Button>
                      <Button
                        variant="secondary"
                        disabled={busy || !selected.userModified}
                        onClick={() => void handleReset()}
                      >
                        Reset default
                      </Button>
                      <Button
                        variant="primary"
                        disabled={busy || !dirty}
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
