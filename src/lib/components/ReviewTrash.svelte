<script lang="ts">
  import { formatBytes } from "$lib/format";
  import type { StagedItem } from "$lib/session";

  // Lists every staged file and needs an explicit confirm. It reports a
  // count, never bytes reclaimed: DClutter is about decisions, not space.
  //
  // A checklist, not a receipt. By the end of a session people have
  // forgotten what they staged twenty cards ago, and the moment before
  // deleting is when a second thought needs somewhere to go. Unticking
  // keeps the file; it doesn't put it back in the queue.
  let {
    items,
    busy,
    error,
    onCancel,
    onConfirm,
  }: {
    items: StagedItem[];
    busy: boolean;
    error: string | null;
    onCancel: () => void;
    onConfirm: (excluded: string[]) => void;
  } = $props();

  let excluded = $state(new Set<string>());
  let dialog: HTMLDialogElement;

  const count = $derived(items.filter((item) => !excluded.has(item.id)).length);
  const unticked = $derived(items.length - count);
  const files = (n: number) => (n === 1 ? "1 file" : `${n} files`);

  $effect(() => {
    dialog.showModal();
  });

  function toggle(id: string) {
    const next = new Set(excluded);
    if (!next.delete(id)) next.add(id);
    excluded = next;
  }

  function toggleAll() {
    excluded = excluded.size === 0 ? new Set(items.map((item) => item.id)) : new Set();
  }

  function confirm() {
    if (!busy && count > 0) onConfirm([...excluded]);
  }

  function onkeydown(e: KeyboardEvent) {
    // Enter confirms. A destructive default is acceptable only here:
    // getting to this list is itself deliberate, every file is shown, and
    // each one can be unticked.
    if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) {
      e.preventDefault();
      confirm();
    }
  }
</script>

<dialog
  bind:this={dialog}
  {onkeydown}
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onCancel();
  }}
  aria-labelledby="review-title"
>
  <h2 id="review-title">Trash {files(count)}?</h2>
  <p class="subtitle">
    {#if unticked > 0}
      {unticked} unticked. Those stay where they are.
    {:else}
      You can restore them from the Recycle Bin later.
    {/if}
  </p>

  <ul>
    {#each items as item (item.id)}
      <li>
        <label>
          <input type="checkbox" checked={!excluded.has(item.id)} onchange={() => toggle(item.id)} disabled={busy} />
          <span class="name" title={item.name}>{item.name}</span>
          <span class="size">{item.isDirectory ? "Folder" : formatBytes(item.bytes)}</span>
        </label>
      </li>
    {/each}
  </ul>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <div class="actions">
    <button class="link" onclick={toggleAll} disabled={busy}>
      {excluded.size === 0 ? "Untick all" : "Tick all"}
    </button>
    <span class="spacer"></span>
    <button class="secondary" onclick={onCancel} disabled={busy}>Cancel</button>
    <button class="primary" onclick={confirm} disabled={busy || count === 0}>
      {busy ? "Trashing…" : `Trash ${files(count)}`}
    </button>
  </div>
</dialog>

<style>
  dialog {
    width: min(460px, calc(100vw - 48px));
    box-sizing: border-box;
    padding: var(--space-xlarge);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sheet);
    background: var(--surface);
    color: var(--text-primary);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 600;
  }

  .subtitle {
    margin: var(--space-small) 0 var(--space-large);
    color: var(--text-secondary);
    font-size: 12px;
  }

  ul {
    max-height: min(320px, 50vh);
    min-height: 120px;
    margin: 0;
    padding: var(--space-unit) 0;
    overflow-y: auto;
    list-style: none;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-preview);
    background: var(--card-surface);
  }

  label {
    display: flex;
    align-items: center;
    gap: var(--space-small);
    padding: 6px var(--space-medium);
    cursor: pointer;
  }

  input {
    flex-shrink: 0;
    margin: 0;
    accent-color: var(--consequence);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  .size {
    flex-shrink: 0;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .error {
    margin: var(--space-medium) 0 0;
    color: var(--consequence);
    font-size: 12px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-small);
    margin-top: var(--space-large);
  }

  .spacer {
    flex: 1;
  }

  button {
    height: 30px;
    padding: 0 var(--space-medium);
    border-radius: var(--radius-chip);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-secondary);
  }

  .link:hover:not(:disabled) {
    color: var(--text-primary);
  }

  .secondary {
    border: 1px solid var(--hairline);
    background: var(--card-surface);
    color: var(--text-primary);
  }

  .primary {
    border: 1px solid transparent;
    background: var(--consequence);
    color: white;
    font-weight: 500;
  }
</style>
