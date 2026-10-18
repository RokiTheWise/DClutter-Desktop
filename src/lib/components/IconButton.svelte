<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    title,
    icon,
    shortcut,
    disabled = false,
    onclick,
  }: {
    title: string;
    icon: IconName;
    shortcut?: string;
    disabled?: boolean;
    onclick: (e: MouseEvent) => void;
  } = $props();
</script>

<button
  {disabled}
  tabindex="-1"
  onmousedown={(e) => e.preventDefault()}
  {onclick}
  title={shortcut ? `${title}  (${shortcut})` : title}
  aria-label={title}
>
  <Icon name={icon} size={15} />
</button>

<style>
  button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: var(--fill-subtle);
    color: var(--text-secondary);
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: var(--fill-hover);
  }

  button:disabled {
    opacity: 0.65;
    cursor: default;
  }
</style>
