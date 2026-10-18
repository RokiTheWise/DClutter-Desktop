<script lang="ts">
  import "$lib/styles/tokens.css";

  let { children } = $props();

  // The WebView's own menu (Back, Reload, Print, Inspect) makes the app feel
  // like a web page. Keep it only in text fields, for copy and paste, and in
  // development builds, for Inspect.
  function oncontextmenu(e: MouseEvent) {
    if (import.meta.env.DEV) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea")) return;
    e.preventDefault();
  }
</script>

<svelte:window {oncontextmenu} />

{@render children()}
