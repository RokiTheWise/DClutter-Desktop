<script lang="ts">
  import { untrack } from "svelte";

  // A text field mid-session breaks the loop, so this one is cheap to
  // leave: Enter renames, Esc cancels, and it takes the footer's place
  // instead of moving the card.
  let {
    initial,
    busy,
    error,
    onSubmit,
    onCancel,
  }: {
    /** The name without its extension, which is kept automatically. */
    initial: string;
    busy: boolean;
    error: string | null;
    onSubmit: (name: string) => void;
    onCancel: () => void;
  } = $props();

  // Starts from the current name once; later changes mustn't overwrite
  // what's being typed.
  let value = $state(untrack(() => initial));
  let input: HTMLInputElement;

  $effect(() => {
    input.focus();
    input.select();
  });

  function onkeydown(e: KeyboardEvent) {
    // The triage keys mean nothing while typing a name.
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      if (!busy) onSubmit(value);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div class="rename">
  <div class="row">
    <label for="rename-input">Rename</label>
    <input
      id="rename-input"
      bind:this={input}
      bind:value
      {onkeydown}
      onblur={() => {
        if (!busy) onCancel();
      }}
      readonly={busy}
      spellcheck="false"
      autocomplete="off"
    />
    <span class="hint">Enter rename · Esc cancel</span>
  </div>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-small);
  }

  label,
  .hint {
    flex-shrink: 0;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  label {
    letter-spacing: 0.5px;
    text-transform: uppercase;
  }

  input {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 var(--space-small);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-chip);
    background: var(--card-surface);
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    outline: none;
  }

  input:focus {
    border-color: var(--text-secondary);
  }

  .error {
    margin: var(--space-unit) 0 0;
    color: var(--consequence);
    font-size: 11px;
  }
</style>
