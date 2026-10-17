<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    title,
    icon,
    key,
    hint,
    consequential = false,
    disabled = false,
    onclick,
  }: {
    title: string;
    icon: IconName;
    /** The key shown on the button. */
    key: string;
    /** Tooltip; defaults to the title and key. */
    hint?: string;
    /** Tinted with the trash colour. Stage only. */
    consequential?: boolean;
    disabled?: boolean;
    onclick: () => void;
  } = $props();
</script>

<!-- Never takes focus: the keyboard drives decisions directly, and a
     focused button would also fire on Space. -->
<button
  class:consequential
  {disabled}
  tabindex="-1"
  onmousedown={(e) => e.preventDefault()}
  {onclick}
  title={hint ?? `${title}  (${key})`}
  aria-label={consequential ? "Stage for trash" : title}
>
  <Icon name={icon} size={15} />
  <span class="title">{title}</span>
  <span class="key" aria-hidden="true">{key}</span>
</button>

<style>
  button {
    --tint: var(--text-primary);
    display: flex;
    flex: 1;
    align-items: center;
    gap: 6px;
    min-width: 0;
    min-height: 48px;
    padding: 0 10px;
    border: 1px solid color-mix(in srgb, var(--tint) 12%, transparent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--tint) 4.5%, transparent);
    color: var(--tint);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--tint) 11%, transparent);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .consequential {
    --tint: var(--consequence);
    border-color: color-mix(in srgb, var(--tint) 22%, transparent);
  }

  .title {
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .key {
    display: grid;
    flex-shrink: 0;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 3px;
    box-sizing: border-box;
    border-radius: 4px;
    background: color-mix(in srgb, var(--tint) 7%, transparent);
    font-family: var(--font-mono);
    font-size: 11px;
  }
</style>
