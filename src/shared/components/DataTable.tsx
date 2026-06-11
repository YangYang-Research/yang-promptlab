type Column<T> = {
  key: string;
  header: string;
  render: (row: T) => React.ReactNode;
  width?: string;
};

type DataTableProps<T> = {
  columns: Column<T>[];
  rows: T[];
  keyField: keyof T & string;
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
};

export function DataTable<T extends { [key: string]: unknown }>({
  columns,
  rows,
  keyField,
  onRowClick,
  emptyMessage = "No data",
}: DataTableProps<T>) {
  if (rows.length === 0) {
    return <p className="data-table__empty">{emptyMessage}</p>;
  }

  return (
    <div className="data-table-wrap">
      <table className="data-table">
        <thead>
          <tr>
            {columns.map((col) => (
              <th key={col.key} style={col.width ? { width: col.width } : undefined}>
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
                <td key={col.key}>{col.render(row)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
