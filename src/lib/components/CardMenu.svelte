<script lang="ts">
  // The right-click menu on the card: the File Explorer vocabulary people
  // already know, as the macOS app offers Finder's.
  let {
    x,
    y,
    onOpen,
    onReveal,
    onRename,
    onCopyName,
    onClose,
  }: {
    x: number;
    y: number;
    onOpen: () => void;
    onReveal: () => void;
    onRename: () => void;
    onCopyName: () => void;
    onClose: () => void;
  } = $props();

  let menu: HTMLDivElement;
  // Kept on screen when opened near the right or bottom edge.
  let position = $state({ left: 0, top: 0 });

  $effect(() => {
    const { width, height } = menu.getBoundingClientRect();
    position = {
      left: Math.min(x, window.innerWidth - width - 8),
      top: Math.min(y, window.innerHeight - height - 8),
    };
    menu.querySelector("button")?.focus();
  });

  function choose(action: () => void) {
    onClose();
    action();
  }

  function onkeydown(e: KeyboardEvent) {
    const items = [...menu.querySelectorAll("button")];
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      items[(at + step + items.length) % items.length].focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
    // Arrows, Enter and Escape belong to the menu while it's open.
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
  <button role="menuitem" onclick={() => choose(onOpen)}>Open <span>Enter</span></button>
  <button role="menuitem" onclick={() => choose(onReveal)}>Show in Explorer</button>
  <hr />
  <button role="menuitem" onclick={() => choose(onRename)}>Rename… <span>F2</span></button>
  <button role="menuitem" onclick={() => choose(onCopyName)}>Copy name</button>
</div>

<style>
  .menu {
    position: fixed;
    z-index: 10;
    min-width: 200px;
    padding: var(--space-unit);
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--card-surface);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
    outline: none;
  }

  button {
    display: flex;
    justify-content: space-between;
    gap: var(--space-xlarge);
    width: 100%;
    padding: 6px 10px;
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
  button:focus-visible,
  button:focus {
    background: var(--fill-hover);
  }

  span {
    color: var(--text-tertiary);
    font-size: 12px;
  }

  hr {
    margin: var(--space-unit) 6px;
    border: 0;
    border-top: 1px solid var(--hairline);
  }
</style>
