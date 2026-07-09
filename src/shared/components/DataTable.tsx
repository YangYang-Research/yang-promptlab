type Column<T> = {
  key: string;
  header: string;
  render: (row: T) => React.ReactNode;
  width?: string;
  align?: "left" | "right";
};

function cellClassName(align?: "left" | "right") {
  return align === "right" ? "data-table__cell--align-right" : undefined;
}

type DataTableProps<T> = {
  columns: Column<T>[];
  rows: T[];
  keyField: keyof T & string;
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
  loading?: boolean;
};

export function DataTable<T extends { [key: string]: unknown }>({
  columns,
  rows,
  keyField,
  onRowClick,
  emptyMessage = "No data",
  loading = false,
}: DataTableProps<T>) {
  if (rows.length === 0) {
    return (
      <div className={`data-table__empty ${loading ? "data-table__empty--loading" : ""}`}>
        {loading ? <span className="data-table__empty-spinner" aria-hidden="true" /> : null}
        <p>{emptyMessage}</p>
      </div>
    );
  }

  return (
    <div className="data-table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            {columns.map((col) => (
              <th
                key={col.key}
                className={cellClassName(col.align)}
                style={col.width ? { width: col.width } : undefined}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={String(row[keyField])}
              className={onRowClick ? "data-table__row--clickable" : undefined}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
            >
              {columns.map((col) => (
                <td key={col.key} className={cellClassName(col.align)}>
                  {col.render(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
