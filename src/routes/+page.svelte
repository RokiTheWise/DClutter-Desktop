<script lang="ts">
  import { formatBytes, scanFolder, sourceHost, type Queue } from "$lib/queue";

  // Temporary view: lists the ranked queue so scoring can be checked
  // against real folders. The one-card-at-a-time triage view replaces it.
  let queue = $state<Queue | null>(null);
  let error = $state<string | null>(null);

  async function load() {
    error = null;
    try {
      queue = await scanFolder();
    } catch (e) {
      error = String(e);
    }
  }

  load();
</script>

<main>
  <header>
    <img src="/logo.png" width="32" height="32" alt="" />
    <div class="heading">
      <h1>DClutter</h1>
      {#if queue}
        <p class="folder" title={queue.folder}>{queue.folder}</p>
      {/if}
    </div>
    <button onclick={load}>Rescan</button>
  </header>

  {#if error}
    <p class="message">{error}</p>
  {:else if !queue}
    <p class="message">Reading folder…</p>
  {:else if queue.items.length === 0}
    <p class="message">Nothing here. This folder is already clear.</p>
  {:else}
    <p class="summary">
      {queue.items.length} {queue.items.length === 1 ? "item" : "items"}, highest priority first
    </p>
    <ol>
      {#each queue.items as item (item.id)}
        {@const host = sourceHost(item.sourceUrl)}
        <li>
          <span class="score">{item.score.toFixed(1)}</span>
          <div class="details">
            <span class="name">{item.name}{item.isDirectory ? "\\" : ""}</span>
            <span class="meta">
              {item.isDirectory ? "Folder" : formatBytes(item.bytes)}
              {#if host}· from {host}{/if}
            </span>
          </div>
          <div class="chips">
            {#if item.redundantCopy}
              <span class="chip">Copy · {item.duplicateCount} total</span>
            {/if}
            {#if item.extractedArchive}
              <span class="chip">Already extracted</span>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</main>

<style>
  main {
    max-width: 760px;
    margin: 0 auto;
    padding: var(--space-xlarge);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-medium);
    margin-bottom: var(--space-xlarge);
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }

  .folder {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: 12px;
  }

  button {
    padding: var(--space-small) var(--space-large);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-chip);
    background: var(--card-surface);
    color: var(--text-primary);
    font: inherit;
    cursor: pointer;
  }

  .message,
  .summary {
    color: var(--text-secondary);
  }

  .summary {
    margin: 0 0 var(--space-small);
    font-size: 12px;
  }

  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sheet);
    background: var(--card-surface);
    overflow: hidden;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-medium);
    padding: var(--space-small) var(--space-large);
  }

  li + li {
    border-top: 1px solid var(--hairline);
  }

  .score {
    width: 3.5em;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .details {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    color: var(--text-secondary);
    font-size: 12px;
  }

  .chips {
    display: flex;
    gap: var(--space-unit);
  }

  .chip {
    padding: 2px var(--space-small);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-chip);
    color: var(--text-secondary);
    font-size: 11px;
    white-space: nowrap;
  }
</style>
