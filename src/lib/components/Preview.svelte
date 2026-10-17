<script lang="ts" module>
  /** Formats WebView2 can draw itself. Everything else gets a type tile
   *  until Windows shell thumbnails are wired up. */
  export const IMAGE_EXTENSIONS = new Set([
    "apng", "avif", "bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp",
  ]);
</script>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { CardItem } from "$lib/session";
  import Icon from "./Icon.svelte";

  let { item }: { item: CardItem } = $props();

  let failed = $state(false);
  const isImage = $derived(!item.isDirectory && IMAGE_EXTENSIONS.has(item.extension) && !failed);
</script>

<div class="well">
  {#if isImage}
    <img src={convertFileSrc(item.path)} alt="" draggable="false" onerror={() => (failed = true)} />
  {:else if item.isDirectory}
    <div class="tile folder"><Icon name="folder" size={72} /></div>
  {:else}
    <div class="tile document">
      <span>{item.extension ? item.extension.toUpperCase().slice(0, 5) : "FILE"}</span>
    </div>
  {/if}
</div>

<style>
  .well {
    display: grid;
    place-items: center;
    height: var(--preview-height);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-preview);
    background: var(--card-surface);
    overflow: hidden;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    user-select: none;
  }

  .tile {
    display: grid;
    place-items: center;
    color: var(--text-tertiary);
  }

  /* A page with a folded corner, sized like the macOS generic file icon. */
  .document {
    position: relative;
    width: 104px;
    height: 132px;
    border: 1.5px solid var(--hairline);
    border-radius: 10px 28px 10px 10px;
    background: var(--surface);
  }

  .document span {
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 15px;
    letter-spacing: 0.06em;
  }
</style>
