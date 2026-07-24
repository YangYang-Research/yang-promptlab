import { useCallback, useEffect, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  ALL_ATTACK_CATEGORY_IDS,
  getCategory,
  type AttackCategoryId,
} from "@/features/scans/attackProfiles";
import {
  ALL_ATTACK_MUTATOR_IDS,
  ATTACK_MUTATORS,
  type AttackMutatorId,
} from "@/features/scans/payloadStrategy";
import { Button, IconButton, PageHeader, RefreshButton } from "@/shared/components";
import { IconCheck, IconCopy } from "@/shared/components/Icons";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import { paginateItems } from "@/shared/utils/pagination";

import {
  categoriesForMutator,
  emptyCategoryMutatorMap,
  hydrateMutatorSettings,
  isMutatorAtDefaultCategories,
  persistMutatorSettings,
  resetMutatorToDefaultCategories,
  toggleMutatorCategory,
  type CategoryMutatorMap,
} from "./mutatorDefaults";

const LIBRARY_PAGE_SIZE = 8;

function ExampleCopyCard({ label, text }: { label: string; text: string }) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      notify(`${label} copied`, "success");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      notify(error instanceof Error ? error.message : `Failed to copy ${label}`, "error");
    }
  }

  return (
    <div className="mutators-page__example-block">
      <span className="mutators-page__example-label">{label}</span>
      <div className="mutators-page__example-card">
        <pre className="mutators-page__example-pre">{text}</pre>
        <IconButton
          ariaLabel={copied ? `${label} copied` : `Copy ${label}`}
          size="sm"
          active={copied}
          onClick={() => void handleCopy()}
        >
          {copied ? <IconCheck /> : <IconCopy />}
        </IconButton>
      </div>
    </div>
  );
}

export function MutatorsPage() {
  const { backendConnected } = useAppStore();
  const [categoryMap, setCategoryMap] = useState<CategoryMutatorMap>(() =>
    emptyCategoryMutatorMap(),
  );
  const [selectedId, setSelectedId] = useState<AttackMutatorId>(ATTACK_MUTATORS[0].id);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const selected = ATTACK_MUTATORS.find((item) => item.id === selectedId) ?? ATTACK_MUTATORS[0];
  const selectedCategories = categoriesForMutator(categoryMap, selected.id);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return ATTACK_MUTATORS;
    return ATTACK_MUTATORS.filter(
      (item) =>
        item.id.includes(needle) ||
        item.label.toLowerCase().includes(needle) ||
        item.description.toLowerCase().includes(needle),
    );
  }, [query]);

  const pagination = useMemo(
    () => paginateItems(filtered, page, LIBRARY_PAGE_SIZE),
    [filtered, page],
  );

  const load = useCallback(async () => {
    if (!backendConnected) {
      setCategoryMap(emptyCategoryMutatorMap());
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    const settings = await hydrateMutatorSettings();
    setCategoryMap(settings.categoryMutators);
    // Keep every mutator available; category assignment is the only gate.
    if (
      settings.enabledMutators.length !== ALL_ATTACK_MUTATOR_IDS.length ||
      ALL_ATTACK_MUTATOR_IDS.some((id) => !settings.enabledMutators.includes(id))
    ) {
      await persistMutatorSettings({
        enabledMutators: [...ALL_ATTACK_MUTATOR_IDS],
        categoryMutators: settings.categoryMutators,
      });
    }
    setLoading(false);
  }, [backendConnected]);

  useEffect(() => {
    void load().catch((err) => {
      setError(toAppError(err).message);
      setCategoryMap(emptyCategoryMutatorMap());
      setLoading(false);
    });
  }, [load]);

  useEffect(() => {
    setPage(1);
  }, [query]);

  useEffect(() => {
    if (page > pagination.totalPages) {
      setPage(pagination.totalPages);
    }
  }, [page, pagination.totalPages]);

  useEffect(() => {
    if (filtered.length === 0) return;
    if (!pagination.items.some((item) => item.id === selectedId)) {
      setSelectedId(pagination.items[0]?.id ?? filtered[0].id);
    }
  }, [filtered, selectedId, pagination.items]);

  async function handleRefresh() {
    try {
      await load();
    } catch (err) {
      setError(toAppError(err).message);
      setLoading(false);
    }
  }

  async function persistCategoryMap(nextMap: CategoryMutatorMap) {
    if (!backendConnected) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await persistMutatorSettings({
        enabledMutators: [...ALL_ATTACK_MUTATOR_IDS],
        categoryMutators: nextMap,
      });
      setCategoryMap(saved.categoryMutators);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  function toggleCategory(mutatorId: AttackMutatorId, categoryId: AttackCategoryId) {
    const assigned = categoriesForMutator(categoryMap, mutatorId).includes(categoryId);
    const nextMap = toggleMutatorCategory(categoryMap, mutatorId, categoryId, !assigned);
    void persistCategoryMap(nextMap);
  }

  function handleResetDefault(mutatorId: AttackMutatorId) {
    const nextMap = resetMutatorToDefaultCategories(categoryMap, mutatorId);
    void persistCategoryMap(nextMap);
  }

  const atDefault = isMutatorAtDefaultCategories(categoryMap, selected.id);

  return (
    <div className="page mutators-page">
      <PageHeader
        title="Mutators"
        description="Transform prompts into alternate forms before they are sent."
        actions={
          <RefreshButton
            loading={loading}
            error={error}
            disabled={!backendConnected || busy}
            onClick={() => void handleRefresh()}
          />
        }
      />

      {!backendConnected ? (
        <section className="mutators-page__empty" aria-live="polite">
          <p className="mutators-page__empty-title">Backend offline</p>
          <p className="text-muted text-sm">
            Connect to the Tauri backend to load and edit mutator settings.
          </p>
        </section>
      ) : loading ? (
        <section className="mutators-page__workspace mutators-page__workspace--loading" aria-busy="true">
          <div className="mutators-page__rail">
            <div className="mutators-page__skeleton mutators-page__skeleton--rail" />
          </div>
          <div className="mutators-page__detail">
            <div className="mutators-page__skeleton mutators-page__skeleton--detail" />
          </div>
        </section>
      ) : (
        <>
          {error ? (
            <p className="mutators-page__error" role="alert">
              {error}
            </p>
          ) : null}

          <section className="mutators-page__workspace" aria-label="Mutator editor">
            <aside className="mutators-page__rail">
              <div className="mutators-page__rail-head">
                <h2 className="mutators-page__rail-title">Library</h2>
                <span className="mutators-page__rail-count">{filtered.length}</span>
              </div>
              <label className="mutators-page__search">
                <span className="sr-only">Search mutators</span>
                <input
                  type="search"
                  value={query}
                  placeholder="Search name or id…"
                  onChange={(event) => setQuery(event.target.value)}
                />
              </label>
              {filtered.length === 0 ? (
                <p className="mutators-page__rail-empty text-muted">No mutators match.</p>
              ) : (
                <>
                  <ul className="mutators-page__rail-list">
                    {pagination.items.map((item) => {
                      const cats = categoriesForMutator(categoryMap, item.id).length;
                      return (
                        <li key={item.id}>
                          <button
                            type="button"
                            className={
                              item.id === selected.id
                                ? "mutators-page__rail-item is-active"
                                : "mutators-page__rail-item"
                            }
                            onClick={() => setSelectedId(item.id)}
                          >
                            <span className="mutators-page__rail-item-name">{item.label}</span>
                            <span className="mutators-page__rail-item-cats">
                              {cats} categor{cats === 1 ? "y" : "ies"}
                            </span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                  {pagination.totalPages > 1 ? (
                    <div className="mutators-page__rail-pager">
                      <button
                        type="button"
                        className="mutators-page__rail-pager-btn"
                        disabled={pagination.page <= 1}
                        onClick={() => setPage((current) => Math.max(1, current - 1))}
                      >
                        Prev
                      </button>
                      <span className="mutators-page__rail-pager-meta">
                        {pagination.page}/{pagination.totalPages}
                      </span>
                      <button
                        type="button"
                        className="mutators-page__rail-pager-btn"
                        disabled={pagination.page >= pagination.totalPages}
                        onClick={() =>
                          setPage((current) => Math.min(pagination.totalPages, current + 1))
                        }
                      >
                        Next
                      </button>
                    </div>
                  ) : null}
                </>
              )}
            </aside>

            <div className="mutators-page__detail">
              <header className="mutators-page__detail-head">
                <div>
                  <p className="mutators-page__detail-id">{selected.id}</p>
                  <h2 className="mutators-page__detail-title">{selected.label}</h2>
                  <p className="mutators-page__detail-desc">{selected.description}</p>
                </div>
                <Button
                  variant="secondary"
                  disabled={busy || atDefault}
                  onClick={() => handleResetDefault(selected.id)}
                >
                  Reset default
                </Button>
              </header>

              <div className="mutators-page__detail-section">
                <div className="mutators-page__example">
                  <ExampleCopyCard
                    key={`${selected.id}-before`}
                    label="Before"
                    text={selected.exampleSeed}
                  />
                  <div className="mutators-page__example-arrow" aria-hidden="true">
                    <span>&gt;</span>
                  </div>
                  <ExampleCopyCard
                    key={`${selected.id}-after`}
                    label="After"
                    text={selected.example}
                  />
                </div>
              </div>

              <div className="mutators-page__detail-section">
                <div className="mutators-page__detail-section-head">
                  <h3 className="mutators-page__detail-section-title">Categories</h3>
                  <p className="mutators-page__detail-section-help">
                    Add or remove this mutator from each attack category plan.
                  </p>
                </div>
                <div className="mutators-page__category-grid" role="group" aria-label="Categories">
                  {ALL_ATTACK_CATEGORY_IDS.map((categoryId) => {
                    const assigned = selectedCategories.includes(categoryId);
                    const category = getCategory(categoryId);
                    return (
                      <button
                        key={categoryId}
                        type="button"
                        className={
                          assigned
                            ? "mutators-page__category-tile is-on"
                            : "mutators-page__category-tile"
                        }
                        aria-pressed={assigned}
                        disabled={busy}
                        onClick={() => toggleCategory(selected.id, categoryId)}
                      >
                        <span className="mutators-page__category-tile-name">{category.label}</span>
                        <span className="mutators-page__category-tile-state">
                          {assigned ? "Assigned" : "Not used"}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          </section>
        </>
      )}
    </div>
  );
}
