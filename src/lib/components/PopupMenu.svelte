<script lang="ts" module>
  export type MenuItem =
    | { label: string; hint?: string; checked?: boolean; onSelect: () => void }
    | "separator";
</script>

<script lang="ts">
  // A small menu at a point: the card's right-click menu, the folder menu
  // and the "…" menu. The arrows move through it, Enter picks, and Esc,
  // clicking elsewhere or leaving the window closes it.
  let {
    x,
    y,
    items,
    alignRight = false,
    onClose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    /** Open leftwards from `x`, for a button at the right of the bar. */
    alignRight?: boolean;
    onClose: () => void;
  } = $props();

  let menu: HTMLDivElement;
  let position = $state({ left: 0, top: 0 });

  $effect(() => {
    // Kept inside the window when opened near an edge.
    const { width, height } = menu.getBoundingClientRect();
    const left = alignRight ? x - width : x;
    position = {
      left: Math.max(8, Math.min(left, window.innerWidth - width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
    };
    menu.querySelector("button")?.focus();
  });

  function choose(item: Exclude<MenuItem, "separator">) {
    onClose();
    item.onSelect();
  }

  function onkeydown(e: KeyboardEvent) {
    const buttons = [...menu.querySelectorAll("button")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      buttons[(at + step + buttons.length) % buttons.length].focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
    // The menu has the keyboard while it's open.
    e.stopPropagation();
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (!menu.contains(e.target as Node)) onClose();
  }}
  onblur={onClose}
  onresize={onClose}
/>

<div
  class="menu"
  role="menu"
  tabindex="-1"
  bind:this={menu}
  style:left="{position.left}px"
  style:top="{position.top}px"
  {onkeydown}
>
  {#each items as item}
    {#if item === "separator"}
      <hr />
    {:else}
      <button role="menuitem" class:checked={item.checked} onclick={() => choose(item)}>
        <span class="label">{item.label}</span>
        {#if item.hint}<span class="hint">{item.hint}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 10;
    min-width: 200px;
    max-width: min(420px, calc(100vw - 16px));
    padding: var(--space-unit);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--card-surface);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
    outline: none;
  }

  button {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-xlarge);
    width: 100%;
    padding: 6px 10px 6px 24px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    outline: none;
  }

  button:hover,
  button:focus {
    background: var(--fill-hover);
  }

  .checked {
    position: relative;
  }

  .checked::before {
    content: "✓";
    position: absolute;
    left: 8px;
    font-size: 12px;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    flex-shrink: 0;
    color: var(--text-tertiary);
    font-size: 12px;
  }

  hr {
    margin: var(--space-unit) 6px;
    border: 0;
    border-top: 1px solid var(--hairline);
  }
</style>
