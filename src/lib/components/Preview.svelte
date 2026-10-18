<script lang="ts" module>
  /** Formats WebView2 draws itself, straight from disk at full quality.
   *  Everything else gets the Windows shell's preview. */
  export const IMAGE_EXTENSIONS = new Set([
    "apng", "avif", "bmp", "gif", "ico", "jpeg", "jpg", "png", "svg", "webp",
  ]);
</script>

<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { fade } from "svelte/transition";
  import * as session from "$lib/session";
  import type { CardItem, PreviewImage } from "$lib/session";
  import Icon from "./Icon.svelte";

  let { item }: { item: CardItem } = $props();

  let imageFailed = $state(false);
  const isImage = $derived(!item.isDirectory && IMAGE_EXTENSIONS.has(item.extension) && !imageFailed);

  /** undefined while asking the shell; null when it has nothing. */
  let preview = $state<PreviewImage | null | undefined>(undefined);

  $effect(() => {
    if (isImage) return;
    const id = item.id;
    preview = undefined;
    session
      .thumbnail(id)
      .then((result) => {
        // A slow answer for a card that has already gone is dropped, so the
        // well never shows the previous file's preview.
        if (item.id === id) preview = result;
      })
      .catch(() => {
        if (item.id === id) preview = null;
      });
  });
</script>

<div class="well">
  {#if isImage}
    <img src={convertFileSrc(item.path)} alt="" draggable="false" onerror={() => (imageFailed = true)} />
  {:else if preview}
    <img
      class:icon={preview.kind === "icon"}
      src={preview.src}
      alt=""
      draggable="false"
      in:fade={{ duration: 120 }}
    />
  {:else if preview === null}
    {#if item.isDirectory}
      <div class="tile folder"><Icon name="folder" size={72} /></div>
    {:else}
      <div class="tile document">
        <span>{item.extension ? item.extension.toUpperCase().slice(0, 5) : "FILE"}</span>
      </div>
    {/if}
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

  /* Type icons top out at 256 px and blur when stretched; show them at a
     comfortable fixed size, like the macOS app's 160-point icon. */
  img.icon {
    width: min(128px, 60%);
    height: min(128px, 60%);
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
