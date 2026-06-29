"use client";

import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import {
  ChevronDown,
  ChevronsUpDown,
  ChevronUp,
  Search,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export type { ColumnDef };

export interface DataTableProps<T> {
  columns: ColumnDef<T, unknown>[];
  data: T[];
  isLoading?: boolean;
  emptyState?: React.ReactNode;
  onRowClick?: (row: T) => void;
  searchPlaceholder?: string;
  enableSearch?: boolean;
  pageSize?: number;
}

/**
 * Generic, reusable table built on @tanstack/react-table. Global text filter,
 * sortable headers, and client-side pagination. Markup mirrors the dashboard
 * table so it slots into any console surface.
 */
export function DataTable<T>({
  columns,
  data,
  isLoading = false,
  emptyState,
  onRowClick,
  searchPlaceholder = "Search…",
  enableSearch = true,
  pageSize = 10,
}: DataTableProps<T>) {
  const [sorting, setSorting] = React.useState<SortingState>([]);
  const [globalFilter, setGlobalFilter] = React.useState("");

  const table = useReactTable({
    data,
    columns,
    state: { sorting, globalFilter },
    onSortingChange: setSorting,
    onGlobalFilterChange: setGlobalFilter,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageSize } },
  });

  const rows = table.getRowModel().rows;
  const totalRows = table.getFilteredRowModel().rows.length;
  const { pageIndex, pageSize: currentPageSize } = table.getState().pagination;
  const firstRow = totalRows === 0 ? 0 : pageIndex * currentPageSize + 1;
  const lastRow = Math.min((pageIndex + 1) * currentPageSize, totalRows);
  const colCount = table.getAllLeafColumns().length;

  return (
    <div className="space-y-3">
      {enableSearch && (
        <div className="relative max-w-xs">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-3" />
          <input
            value={globalFilter}
            onChange={(e) => setGlobalFilter(e.target.value)}
            placeholder={searchPlaceholder}
            aria-label="Search table"
            className="h-9 w-full rounded-lg border border-line bg-surface pl-9 pr-3 text-sm text-ink placeholder:text-ink-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background"
          />
        </div>
      )}

      <div className="overflow-x-auto rounded-xl border border-line bg-surface">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id} className="border-b border-line">
                {headerGroup.headers.map((header) => {
                  const canSort = header.column.getCanSort();
                  const sorted = header.column.getIsSorted();
                  return (
                    <th
                      key={header.id}
                      className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wide text-ink-3"
                    >
                      {header.isPlaceholder ? null : canSort ? (
                        <button
                          type="button"
                          onClick={header.column.getToggleSortingHandler()}
                          className="inline-flex items-center gap-1.5 hover:text-ink-2"
                        >
                          {flexRender(
                            header.column.columnDef.header,
                            header.getContext()
                          )}
                          {sorted === "asc" ? (
                            <ChevronUp className="h-3.5 w-3.5" />
                          ) : sorted === "desc" ? (
                            <ChevronDown className="h-3.5 w-3.5" />
                          ) : (
                            <ChevronsUpDown className="h-3.5 w-3.5 text-ink-3" />
                          )}
                        </button>
                      ) : (
                        flexRender(
                          header.column.columnDef.header,
                          header.getContext()
                        )
                      )}
                    </th>
                  );
                })}
              </tr>
            ))}
          </thead>
          <tbody>
            {isLoading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <tr key={i} className="border-b border-line last:border-0">
                  {Array.from({ length: colCount }).map((__, j) => (
                    <td key={j} className="px-4 py-3">
                      <div className="skeleton h-5 w-full max-w-[180px]" />
                    </td>
                  ))}
                </tr>
              ))
            ) : rows.length === 0 ? (
              <tr>
                <td colSpan={colCount} className="px-4 py-3">
                  {emptyState ?? (
                    <div className="py-10 text-center text-sm text-ink-3">
                      No results
                    </div>
                  )}
                </td>
              </tr>
            ) : (
              rows.map((row) => (
                <tr
                  key={row.id}
                  onClick={
                    onRowClick ? () => onRowClick(row.original) : undefined
                  }
                  className={cn(
                    "border-b border-line last:border-0 hover:bg-surface-2/50",
                    onRowClick && "cursor-pointer"
                  )}
                >
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="px-4 py-3 text-sm">
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext()
                      )}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {!isLoading && totalRows > 0 && (
        <div className="flex items-center justify-between gap-3">
          <p data-numeric className="text-xs text-ink-3">
            Showing {firstRow}–{lastRow} of {totalRows}
          </p>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
            >
              Prev
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
            >
              Next
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
