<script lang="ts" module>
  export interface DialogAction {
    label: string;
    /** "consequential" is tinted with the trash colour and is the default. */
    kind?: "consequential" | "secondary";
    onSelect: () => void;
  }
</script>

<script lang="ts">
  // A question with a few answers, for Start Over and switching folders
  // with files still staged. Esc cancels. Nothing is pre-selected that
  // would act on files: Enter only presses the button that has focus.
  let {
    title,
    message,
    actions,
    onCancel,
  }: {
    title: string;
    message: string;
    actions: DialogAction[];
    onCancel: () => void;
  } = $props();

  let dialog: HTMLDialogElement;

  $effect(() => {
    dialog.showModal();
    // Focus Cancel, not an action that changes files.
    dialog.querySelector<HTMLButtonElement>("button.cancel")?.focus();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="confirm-title"
  oncancel={(e) => {
    e.preventDefault();
    onCancel();
  }}
  onkeydown={(e) => e.stopPropagation()}
>
  <h2 id="confirm-title">{title}</h2>
  <p>{message}</p>
  <div class="actions">
    <button class="cancel secondary" onclick={onCancel}>Cancel</button>
    {#each actions as action}
      <button class={action.kind ?? "consequential"} onclick={action.onSelect}>{action.label}</button>
    {/each}
  </div>
</dialog>

<style>
  dialog {
    width: min(420px, calc(100vw - 48px));
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

  p {
    margin: var(--space-small) 0 var(--space-xlarge);
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.45;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-small);
  }

  button {
    height: 30px;
    padding: 0 var(--space-medium);
    border-radius: var(--radius-chip);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }

  .secondary {
    border: 1px solid var(--hairline);
    background: var(--card-surface);
    color: var(--text-primary);
  }

  .consequential {
    border: 1px solid transparent;
    background: var(--consequence);
    color: white;
    font-weight: 500;
  }

  .cancel {
    margin-right: auto;
  }
</style>
