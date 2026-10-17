<script lang="ts">
  import { fileKind, formatBytes, formatDate, sourceHost } from "$lib/format";
  import type { CardItem } from "$lib/session";
  import Icon from "./Icon.svelte";
  import Preview from "./Preview.svelte";

  let { item, onOpen }: { item: CardItem; onOpen: () => void } = $props();

  // Grey, informational, at most three — per the design spec. The macOS
  // app's "never opened" chip is left out: Windows can't tell us.
  const chips = $derived(
    [
      item.duplicateCount > 1 ? `${item.duplicateCount} copies` : null,
      item.extractedArchive ? "already extracted" : null,
    ].filter((chip): chip is string => chip !== null),
  );

  // Always the same four fields, so nothing moves between cards. MODIFIED
  // stands in for the macOS app's LAST OPENED, which Windows doesn't track.
  const rows = $derived([
    ["Size", item.isDirectory ? "—" : formatBytes(item.bytes)],
    ["Modified", formatDate(item.modified)],
    ["From", sourceHost(item.sourceUrl) ?? "—"],
    ["Added", formatDate(item.created)],
  ]);
</script>

<article class="card" ondblclick={onOpen} title="Double-click to open this file">
  <div class="preview">
    <Preview {item} />
  </div>

  <div class="body">
    <div class="title-row">
      <h2 title={item.name}>{item.name}</h2>
      <button class="open" tabindex="-1" onclick={onOpen} title="Open this file  (Enter)">
        Open <Icon name="open" size={12} />
      </button>
    </div>

    <div class="kind-row">
      <span class="kind">{fileKind(item.isDirectory, item.extension)}</span>
      {#each chips as chip}
        <span class="chip">{chip}</span>
      {/each}
    </div>

    <hr />

    <dl>
      {#each rows as [label, value]}
        <div>
          <dt>{label}</dt>
          <dd title={value}>{value}</dd>
        </div>
      {/each}
    </dl>
  </div>
</article>

<style>
  /* Flat, 20px radius, no shadow: depth comes from the surface step and
     the hairline. */
  .card {
    display: flex;
    flex-direction: column;
    height: calc(var(--preview-height) + 250px);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-card);
    background: var(--card-surface);
    overflow: hidden;
    user-select: none;
  }

  .preview {
    padding: 20px;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-medium);
    padding: 0 20px 20px;
  }

  .title-row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-medium);
  }

  h2 {
    flex: 1;
    min-width: 0;
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 500;
    letter-spacing: -0.4px;
    line-height: 1.25;
  }

  .open {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: var(--space-unit);
    margin-top: 3px;
    padding: 3px var(--space-small);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-chip);
    background: var(--fill-subtle);
    color: var(--text-primary);
    font: inherit;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
  }

  .open:hover {
    background: var(--fill-hover);
  }

  .kind-row {
    display: flex;
    align-items: center;
    gap: var(--space-small);
    height: 22px;
    margin-top: calc(-1 * var(--space-small));
  }

  .kind {
    color: var(--text-secondary);
    font-size: 12px;
    white-space: nowrap;
  }

  .chip {
    padding: 3px var(--space-small);
    border-radius: var(--radius-chip);
    background: var(--fill-chip);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.1px;
    white-space: nowrap;
  }

  hr {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--hairline);
  }

  dl {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin: 0;
  }

  dl div {
    min-width: 0;
  }

  dt {
    margin-bottom: var(--space-unit);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.7px;
    text-transform: uppercase;
  }

  dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
</style>
