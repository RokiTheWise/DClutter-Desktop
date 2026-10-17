<script lang="ts">
  import { MAX_DESTINATIONS, type DestinationItem } from "$lib/session";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  // Port of DestinationShelf: the up-to-three bins a file can be filed
  // into. Exactly one bin is highlighted at a time; there's never an
  // ambiguous middle state. With no destinations yet, a single "Choose a
  // folder" bin shows instead of nothing, so the feature explains itself.
  let {
    destinations,
    highlighted,
    onSelect,
    onAdd,
    onRemove,
    onClose,
  }: {
    destinations: DestinationItem[];
    /** Index into the bins; `destinations.length` is the add bin. */
    highlighted: number;
    onSelect: (index: number) => void;
    onAdd: () => void;
    onRemove: (id: string) => void;
    onClose: () => void;
  } = $props();

  // Adding sits in the same row rather than in settings, so filling the
  // bins is something you do while sorting, not before it.
  const canAdd = $derived(destinations.length < MAX_DESTINATIONS);
</script>

<section class="shelf" aria-label="File into a folder">
  <header>
    <div>
      <h2>File into a folder</h2>
      <p>← → to choose · Enter to file · Esc to cancel</p>
    </div>
    <IconButton title="Close" icon="close" shortcut="Esc" onclick={onClose} />
  </header>

  <div class="bins">
    {#each destinations as destination, index (destination.id)}
      <div class="bin-wrap">
        <button
          class="bin"
          class:highlighted={highlighted === index}
          tabindex="-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => onSelect(index)}
          title="File into {destination.path}  ({index + 1})"
        >
          <Icon name="folder" size={18} />
          <span class="label">{destination.name}</span>
          <span class="key">{index + 1}</span>
        </button>
        <button
          class="remove"
          tabindex="-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => onRemove(destination.id)}
          title="Remove {destination.name} from the shelf. The folder itself is left alone."
          aria-label="Remove {destination.name} from the shelf"
        >
          <Icon name="close" size={11} />
        </button>
      </div>
    {/each}

    {#if canAdd}
      <button
        class="bin"
        class:highlighted={highlighted === destinations.length}
        tabindex="-1"
        onmousedown={(e) => e.preventDefault()}
        onclick={onAdd}
        title="Pick or create a folder to file into  (Ctrl+N)"
      >
        <Icon name="folder-plus" size={18} />
        <span class="label">{destinations.length === 0 ? "Choose a folder…" : "Add folder…"}</span>
        <span class="key">Ctrl+N</span>
      </button>
    {/if}
  </div>
</section>

<style>
  .shelf {
    border: 1px solid var(--hairline);
    border-radius: 16px;
    background: var(--card-surface);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: var(--space-large) var(--space-large) 0;
  }

  h2 {
    margin: 0 0 var(--space-unit);
    font-size: 14px;
    font-weight: 500;
  }

  header p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .bins {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-medium);
    padding: var(--space-large);
  }

  .bin-wrap {
    position: relative;
  }

  .bin {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-unit);
    width: 132px;
    height: 76px;
    padding: 0 var(--space-small);
    box-sizing: border-box;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-bin);
    background: var(--card-surface);
    color: var(--text-secondary);
    font: inherit;
    cursor: pointer;
  }

  .bin:hover {
    background: var(--fill-subtle);
  }

  .bin.highlighted {
    border: 2px solid var(--text-primary);
    color: var(--text-primary);
  }

  .label {
    max-width: 100%;
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .key {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .remove {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--fill-chip);
    color: var(--text-secondary);
    cursor: pointer;
  }

  .remove:hover {
    background: var(--fill-hover);
    color: var(--text-primary);
  }
</style>
