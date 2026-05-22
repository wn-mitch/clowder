<script lang="ts">
  // Reusable atlas-grid component (ticket 449 Phase 4 + 5).
  // Renders a sprite-sheet PNG as a clickable grid with hover-highlight,
  // optional named-cell outlines, and an optional ring on a currently-
  // selected index. Lifted from tools/sprite_catalog.html and made
  // reactive.

  interface Props {
    src: string
    cols: number
    rows: number
    tile: number
    /** Display zoom factor; final cell size in CSS pixels is `tile * scale`. */
    scale?: number
    /** Cell index drawn with a thick accent ring. */
    highlightedIndex?: number | null
    /** Cell-index → label; labeled cells get a green outline + tooltip. */
    namedCells?: Record<number, string>
    /** Fires with the cell index when the user clicks a cell. */
    onCellClick?: (index: number) => void
  }

  let {
    src,
    cols,
    rows,
    tile,
    scale = 3,
    highlightedIndex = null,
    namedCells = {},
    onCellClick,
  }: Props = $props()

  let hoverIdx = $state<number | null>(null)
  let displayTile = $derived(tile * scale)
  let totalCells = $derived(cols * rows)
</script>

<div class="inline-block border border-border rounded bg-bg-deep p-2">
  <div
    class="relative"
    style="width: {cols * displayTile}px; height: {rows * displayTile}px;"
  >
    <img
      {src}
      alt=""
      class="absolute inset-0 select-none pointer-events-none"
      style="width: {cols * displayTile}px; height: {rows * displayTile}px; image-rendering: pixelated;"
      draggable="false"
    />
    {#each Array.from({ length: totalCells }) as _, idx}
      {@const col = idx % cols}
      {@const row = Math.floor(idx / cols)}
      {@const isNamed = namedCells[idx] !== undefined}
      {@const isHighlighted = idx === highlightedIndex}
      {@const isHover = idx === hoverIdx}
      <button
        type="button"
        class="absolute m-0 p-0 border-none bg-transparent cursor-pointer"
        style="
          left: {col * displayTile}px;
          top: {row * displayTile}px;
          width: {displayTile}px;
          height: {displayTile}px;
          outline: {isHighlighted
            ? '3px solid #d4943a'
            : isHover
              ? '2px solid #d4943a'
              : isNamed
                ? '1px solid rgba(107, 158, 58, 0.5)'
                : 'none'};
          outline-offset: -1px;
          z-index: {isHighlighted ? 3 : isHover ? 2 : 1};
        "
        title={namedCells[idx] ?? `#${idx} · col ${col} row ${row}`}
        aria-label={namedCells[idx] ?? `cell ${idx}`}
        onmouseenter={() => (hoverIdx = idx)}
        onmouseleave={() => (hoverIdx = null)}
        onclick={() => onCellClick?.(idx)}
      ></button>
    {/each}
  </div>
</div>
