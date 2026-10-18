<script lang="ts" module>
  export type SettingsSection = "general" | "controls";
</script>

<script lang="ts">
  import { HOLD_SPEED, boundedSpeed } from "$lib/settings";
  import IconButton from "./IconButton.svelte";

  // Port of TriageSettingsView, with the controls guide in its second tab.
  let {
    section = $bindable(),
    holdSpeed = $bindable(),
    swipeReversed = $bindable(),
    onClose,
  }: {
    section: SettingsSection;
    holdSpeed: number;
    swipeReversed: boolean;
    onClose: () => void;
  } = $props();

  let panel: HTMLDivElement;

  $effect(() => {
    panel.focus();
  });

  const speedLabel = $derived.by(() => {
    const speed = boundedSpeed(holdSpeed);
    return `${Number.isInteger(speed) ? speed : speed.toFixed(1)} ${speed === 1 ? "file" : "files"} / second`;
  });

  const keyboard: [string, string][] = [
    ["→", "Keep, next card"],
    ["←", "Stage for trash, next card"],
    ["Hold ←  →", "Repeat at the speed set in General"],
    ["Space", "Skip. It comes back at the end"],
    ["1 – 3", "File into a destination folder"],
    ["↑", "Open the destination shelf"],
    ["← →", "On the shelf: choose a folder, then Enter to file"],
    ["Delete", "On the shelf: remove the highlighted folder"],
    ["Esc  ↓", "Close the shelf"],
    ["Enter", "Open this file"],
    ["↓  F2", "Rename this file"],
    ["Ctrl+Z", "Undo"],
    ["Ctrl+Shift+Z  Ctrl+Y", "Redo"],
    ["Ctrl+Enter", "Review and trash staged files"],
    ["Ctrl+N", "Add a destination folder"],
    ["Ctrl+,", "Settings"],
    ["?", "Show this list"],
  ];

  const touchpad: [string, string][] = [
    ["Two fingers right", "Keep"],
    ["Two fingers left", "Stage for trash"],
    ["Two fingers up", "Open the destination shelf"],
    ["On the shelf", "Swipe left or right to choose, up to file, down to close"],
  ];

  const mouse: [string, string][] = [
    ["Drag the card", "Right to keep, left to stage; works with a finger too"],
    ["Click Stage, Skip, Keep", "Decide on the current file"],
    ["Click File", "Open the destination shelf"],
    ["Click a folder", "File into that folder"],
    ["Double-click the card", "Open this file"],
    ["Right-click the card", "Open, show in File Explorer, rename, copy name"],
  ];

  function onkeydown(e: KeyboardEvent) {
    // The settings take the keyboard while they're open, so arrows can
    // adjust the slider without deciding on files.
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
    e.stopPropagation();
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    const target = e.target as HTMLElement;
    if (!panel.contains(target) && !target.closest("[data-settings-toggle]")) onClose();
  }}
/>

<div class="panel" role="dialog" aria-label="Settings" tabindex="-1" bind:this={panel} {onkeydown}>
  <header>
    <h2>Settings</h2>
    <IconButton title="Close" icon="close" shortcut="Esc" onclick={onClose} />
  </header>

  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={section === "general"} onclick={() => (section = "general")}>General</button>
    <button role="tab" aria-selected={section === "controls"} onclick={() => (section = "controls")}>
      Controls
    </button>
  </div>

  {#if section === "general"}
    <section class="general">
      <div class="label-row">
        <label for="hold-speed">Hold speed</label>
        <span class="value">{speedLabel}</span>
      </div>
      <input
        id="hold-speed"
        type="range"
        min={HOLD_SPEED.min}
        max={HOLD_SPEED.max}
        step={HOLD_SPEED.step}
        bind:value={holdSpeed}
      />
      <div class="ends"><span>Slow</span><span>Fast</span></div>
      <p class="help">
        {holdSpeed < 1
          ? "One file every two seconds while holding ← or →. Single presses respond immediately."
          : "Hold ← or → to move through files at this pace. Single presses respond immediately."}
      </p>

      <hr />

      <label class="toggle">
        <input type="checkbox" bind:checked={swipeReversed} />
        Reverse swipe direction
      </label>
      <p class="help">
        Turn this on if swiping right stages instead of keeps. That happens when Windows is set to
        reverse touchpad scrolling.
      </p>
    </section>
  {:else}
    <section class="controls">
      <h3>Keyboard</h3>
      <dl>
        {#each keyboard as [key, action]}
          <dt>{key}</dt>
          <dd>{action}</dd>
        {/each}
      </dl>
      <h3>Touchpad</h3>
      <dl>
        {#each touchpad as [key, action]}
          <dt>{key}</dt>
          <dd>{action}</dd>
        {/each}
      </dl>
      <h3>Mouse</h3>
      <dl>
        {#each mouse as [key, action]}
          <dt>{key}</dt>
          <dd>{action}</dd>
        {/each}
      </dl>
    </section>
  {/if}
</div>

<style>
  .panel {
    position: absolute;
    top: 44px;
    left: 0;
    z-index: 5;
    box-sizing: border-box;
    width: min(420px, calc(100vw - 48px));
    padding: var(--space-large) var(--space-xlarge) var(--space-xlarge);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sheet);
    background: var(--card-surface);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.25);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-medium);
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .tabs {
    display: flex;
    gap: 2px;
    margin-bottom: var(--space-large);
    padding: 2px;
    border-radius: 8px;
    background: var(--fill-subtle);
  }

  .tabs button {
    flex: 1;
    height: 26px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .tabs button[aria-selected="true"] {
    background: var(--card-surface);
    color: var(--text-primary);
    box-shadow: 0 0 0 1px var(--hairline);
  }

  .label-row,
  .ends {
    display: flex;
    justify-content: space-between;
  }

  label {
    font-size: 13px;
    font-weight: 500;
  }

  .value {
    color: var(--text-secondary);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }

  input[type="range"] {
    width: 100%;
    margin: var(--space-small) 0 2px;
    accent-color: var(--text-primary);
  }

  .ends {
    color: var(--text-tertiary);
    font-size: 11px;
  }

  .help {
    margin: var(--space-medium) 0 0;
    color: var(--text-secondary);
    font-size: 12px;
  }

  hr {
    margin: var(--space-large) 0;
    border: 0;
    border-top: 1px solid var(--hairline);
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-small);
    cursor: pointer;
  }

  .toggle input {
    margin: 0;
    accent-color: var(--text-primary);
  }

  .controls {
    max-height: min(460px, 60vh);
    overflow-y: auto;
  }

  h3 {
    margin: 0 0 var(--space-small);
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 400;
    letter-spacing: 0.5px;
    text-transform: uppercase;
  }

  h3 + dl {
    margin-bottom: var(--space-large);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 7px var(--space-large);
    margin: 0;
  }

  dt {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: nowrap;
  }

  dd {
    margin: 0;
    font-size: 12px;
  }
</style>
