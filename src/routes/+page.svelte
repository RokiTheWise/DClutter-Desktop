<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { cubicIn, cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import Card from "$lib/components/Card.svelte";
  import CardMenu from "$lib/components/CardMenu.svelte";
  import DecisionButton from "$lib/components/DecisionButton.svelte";
  import DestinationShelf from "$lib/components/DestinationShelf.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import { IMAGE_EXTENSIONS } from "$lib/components/Preview.svelte";
  import RenameField from "$lib/components/RenameField.svelte";
  import ReviewTrash from "$lib/components/ReviewTrash.svelte";
  import SettingsPanel, { type SettingsSection } from "$lib/components/SettingsPanel.svelte";
  import { HeldDecision } from "$lib/held";
  import { loadHoldSpeed, saveHoldSpeed } from "$lib/settings";
  import * as session from "$lib/session";
  import type { DecisionKind, DestinationItem, SessionView, StagedItem } from "$lib/session";

  type Exit = "left" | "right" | "up" | "fade";
  type Entrance = "rise" | "left" | "right" | "top" | "fade";

  let view = $state<SessionView | null>(null);
  /** Couldn't open the folder at all. */
  let loadError = $state<string | null>(null);
  /** Something went wrong mid-session; shown in the footer. */
  let notice = $state<string | null>(null);
  /** What the last trash commit did; shown in the footer. */
  let status = $state<string | null>(null);

  /** The staged files while the review-trash list is open. */
  let review = $state<StagedItem[] | null>(null);
  let reviewBusy = $state(false);
  let reviewError = $state<string | null>(null);

  /** Where the card's right-click menu is open, if it is. */
  let menu = $state<{ x: number; y: number } | null>(null);

  let renaming = $state(false);
  let renameBusy = $state(false);
  let renameError = $state<string | null>(null);

  let destinations = $state<DestinationItem[]>([]);
  let shelfOpen = $state(false);
  /** The highlighted bin; `destinations.length` is the add bin. */
  let selectedBin = $state(0);
  // The row ends with an add bin whenever there's room for another folder.
  const binCount = $derived(destinations.length + (destinations.length < session.MAX_DESTINATIONS ? 1 : 0));

  // How the outgoing card leaves and the incoming one arrives. Set just
  // before `view` changes; Svelte reads them when the transitions start.
  let exit = $state<Exit>("fade");
  let entrance = $state<Entrance>("rise");

  const held = new HeldDecision();
  let holdSpeed = $state(loadHoldSpeed());
  $effect(() => saveHoldSpeed(holdSpeed));

  let settingsOpen = $state(false);
  let settingsSection = $state<SettingsSection>("general");

  function openSettings(section: SettingsSection) {
    held.stop();
    shelfOpen = false;
    menu = null;
    settingsSection = section;
    settingsOpen = true;
  }
  const folderName = $derived(view?.folder.split(/[\\/]/).filter(Boolean).pop() ?? "");

  async function load() {
    loadError = null;
    try {
      [view, destinations] = await Promise.all([session.openQueue(), session.listDestinations()]);
    } catch (e) {
      loadError = String(e);
    }
  }

  load();

  // Get the next card's preview ready before it arrives: decode its image,
  // or have the shell make its thumbnail, which the app then caches.
  $effect(() => {
    const next = view?.upNext;
    if (!next) return;
    if (!next.isDirectory && IMAGE_EXTENSIONS.has(next.extension)) {
      new Image().src = convertFileSrc(next.path);
    } else {
      session.thumbnail(next.id).catch(() => {});
    }
  });

  // Commands run one at a time, in order, so a fast run of key presses
  // can never apply out of sequence.
  let pending: Promise<void> = Promise.resolve();
  function enqueue(work: () => Promise<void>) {
    pending = pending.then(work).catch((e) => {
      notice = String(e);
    });
  }

  const exitFor = (kind: DecisionKind): Exit =>
    kind === "keep" ? "right" : kind === "stage" ? "left" : kind === "file" ? "up" : "fade";

  // A rename changes the card's name, not its place: a quick cross-fade.

  const returnFor = (kind: DecisionKind): Entrance =>
    kind === "keep"
      ? "right"
      : kind === "stage"
        ? "left"
        : kind === "file"
          ? "top"
          : kind === "rename"
            ? "fade"
            : "rise";

  /** Returns false when there's no card to decide on, ending a hold. */
  function makeDecision(kind: DecisionKind): boolean {
    if (!view?.current || review || shelfOpen || renaming) return false;
    notice = null;
    status = null;
    enqueue(async () => {
      const next = await session.decide(kind);
      exit = exitFor(kind);
      entrance = "rise";
      view = next;
    });
    return true;
  }

  function undo() {
    notice = null;
    enqueue(async () => {
      const change = await session.undo();
      if (change.kind === null) return;
      // The card comes back in from the side it left by, so an undo reads
      // as a rewind rather than as another decision.
      exit = "fade";
      entrance = returnFor(change.kind);
      view = change.view;
    });
  }

  function redo() {
    notice = null;
    enqueue(async () => {
      const change = await session.redo();
      if (change.kind === null) return;
      exit = exitFor(change.kind);
      entrance = "rise";
      view = change.view;
    });
  }

  /** The name to start editing from: without the extension, which is
   *  kept automatically and isn't the user's to delete by accident. */
  function stemOf(item: { name: string; extension: string; isDirectory: boolean }) {
    return item.isDirectory || !item.extension ? item.name : item.name.slice(0, -(item.extension.length + 1));
  }

  function startRename() {
    if (!view?.current || review || shelfOpen) return;
    held.stop();
    notice = null;
    renameError = null;
    renaming = true;
  }

  function submitRename(name: string) {
    renameBusy = true;
    enqueue(async () => {
      try {
        const next = await session.renameCurrent(name);
        exit = "fade";
        entrance = "fade";
        view = next;
        renaming = false;
      } catch (e) {
        // Keep the field open so the name can be fixed.
        renameError = String(e);
      } finally {
        renameBusy = false;
      }
    });
  }

  function openShelf() {
    if (!view?.current || review || renaming) return;
    held.stop();
    notice = null;
    selectedBin = Math.min(selectedBin, binCount - 1);
    shelfOpen = true;
  }

  function stepBin(step: number) {
    selectedBin = Math.min(Math.max(selectedBin + step, 0), binCount - 1);
  }

  function confirmShelf() {
    if (selectedBin < destinations.length) {
      fileInto(selectedBin);
    } else {
      addFolder();
    }
  }

  /** Files the current file into bin `index`. The move happens at once
   *  and can be undone. */
  function fileInto(index: number) {
    const destination = destinations[index];
    if (!destination || !view?.current || review) return;
    held.stop();
    shelfOpen = false;
    notice = null;
    status = null;
    enqueue(async () => {
      const next = await session.fileCurrent(destination.id);
      // Up, toward the shelf: the file visibly goes the way you reached.
      exit = "up";
      entrance = "rise";
      view = next;
    });
  }

  function addFolder() {
    held.stop();
    shelfOpen = false;
    notice = null;
    enqueue(async () => {
      destinations = await session.addDestination();
    });
  }

  function removeFolder(id: string) {
    enqueue(async () => {
      destinations = await session.removeDestination(id);
      selectedBin = Math.min(selectedBin, binCount - 1);
    });
  }

  function openReview() {
    if (!view || view.staged === 0 || review) return;
    held.stop();
    notice = null;
    enqueue(async () => {
      reviewError = null;
      review = await session.stagedFiles();
    });
  }

  function confirmReview(excluded: string[]) {
    reviewBusy = true;
    enqueue(async () => {
      try {
        const outcome = await session.commitTrash(excluded);
        view = outcome.view;
        if (outcome.trashed > 0) {
          status = outcome.trashed === 1
            ? "Trashed 1 file."
            : `Trashed ${outcome.trashed} files.`;
        }
        if (outcome.error) {
          // Keep the list open on what's left, which is what failed.
          reviewError = outcome.error;
          review = await session.stagedFiles();
          if (review.length === 0) review = null;
        } else {
          review = null;
        }
      } catch (e) {
        // Show it in the list, not in the footer hidden behind it.
        reviewError = String(e);
      } finally {
        reviewBusy = false;
      }
    });
  }

  function reveal() {
    session.revealCurrent().catch((e) => {
      notice = `Couldn't show this file in File Explorer: ${e}`;
    });
  }

  async function copyName() {
    const name = view?.current?.name;
    if (!name) return;
    try {
      await navigator.clipboard.writeText(name);
      notice = null;
      status = "Copied the file name.";
    } catch (e) {
      notice = `Couldn't copy the name: ${e}`;
    }
  }

  function open() {
    if (!view?.current) return;
    session.openCurrent().catch((e) => {
      notice = `Couldn't open this file: ${e}`;
    });
  }

  function onkeydown(e: KeyboardEvent) {
    // The review list, the rename field, the menu and the settings handle
    // their own keys.
    if (!view || review || renaming || menu || settingsOpen) return;

    // While the shelf is open the arrows steer it instead of deciding.
    if (shelfOpen && !e.ctrlKey && !e.metaKey && !e.altKey) {
      switch (e.key) {
        case "ArrowLeft":
          e.preventDefault();
          stepBin(-1);
          break;
        case "ArrowRight":
          e.preventDefault();
          stepBin(1);
          break;
        case "Enter":
          e.preventDefault();
          if (!e.repeat) confirmShelf();
          break;
        case "Escape":
        case "ArrowUp":
        case "ArrowDown":
          e.preventDefault();
          shelfOpen = false;
          break;
        case "Delete":
        case "Backspace":
          // Frees a slot for another folder. Only the shelf forgets it:
          // the folder and anything already filed there stay put.
          e.preventDefault();
          if (selectedBin < destinations.length) removeFolder(destinations[selectedBin].id);
          break;
      }
      return;
    }

    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const key = e.key.toLowerCase();
      if (key === "n") {
        e.preventDefault();
        addFolder();
      } else if (key === ",") {
        e.preventDefault();
        openSettings("general");
      } else if (key === "z") {
        e.preventDefault();
        e.shiftKey ? redo() : undo();
      } else if (key === "y") {
        e.preventDefault();
        redo();
      } else if (e.key === "Enter") {
        e.preventDefault();
        openReview();
      }
      return;
    }
    if (e.altKey || e.ctrlKey || e.metaKey) return;

    switch (e.key) {
      case "ArrowRight":
        e.preventDefault();
        // The held-decision timer paces repeats, not the system.
        if (!e.repeat) held.start(e.key, () => makeDecision("keep"), holdSpeed);
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (!e.repeat) held.start(e.key, () => makeDecision("stage"), holdSpeed);
        break;
      case " ":
        e.preventDefault();
        if (!e.repeat) makeDecision("skip");
        break;
      case "ArrowUp":
        e.preventDefault();
        openShelf();
        break;
      // Down, as on macOS, and F2, the Windows rename key.
      case "ArrowDown":
      case "F2":
        e.preventDefault();
        startRename();
        break;
      case "Enter":
        e.preventDefault();
        if (!e.repeat) open();
        break;
      case "?":
        e.preventDefault();
        openSettings("controls");
        break;
      case "1":
      case "2":
      case "3":
        if (!e.repeat) fileInto(Number(e.key) - 1);
        break;
    }
  }

  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const TRAVEL = 760;

  function cardOut(_node: Element, { exit }: { exit: Exit }) {
    if (reduceMotion || exit === "fade") {
      return { duration: 120, css: (t: number) => `opacity: ${t}` };
    }
    if (exit === "up") {
      return {
        duration: 140,
        easing: cubicIn,
        css: (t: number, u: number) => `transform: translateY(${-u * TRAVEL}px); opacity: ${t}`,
      };
    }
    const dx = exit === "right" ? TRAVEL : -TRAVEL;
    return {
      duration: 140,
      easing: cubicIn,
      css: (t: number, u: number) => `transform: translateX(${u * dx}px); opacity: ${t}`,
    };
  }

  function cardIn(_node: Element, { from }: { from: Entrance }) {
    if (reduceMotion) {
      return { duration: 150, css: (t: number) => `opacity: ${t}` };
    }
    if (from === "rise") {
      return {
        duration: 280,
        easing: cubicOut,
        css: (t: number, u: number) => `transform: translateY(${u * 40}px); opacity: ${t}`,
      };
    }
    if (from === "fade") {
      return { duration: 120, css: (t: number) => `opacity: ${t}` };
    }
    if (from === "top") {
      return {
        duration: 220,
        easing: cubicOut,
        css: (t: number, u: number) => `transform: translateY(${-u * TRAVEL}px); opacity: ${t}`,
      };
    }
    const dx = from === "right" ? TRAVEL : -TRAVEL;
    return {
      duration: 220,
      easing: cubicOut,
      css: (t: number, u: number) => `transform: translateX(${u * dx}px); opacity: ${t}`,
    };
  }
</script>

<svelte:window {onkeydown} onkeyup={(e) => held.stop(e.key)} onblur={() => held.stop()} />

{#if loadError}
  <div class="centered">
    <h1>Couldn't read the folder</h1>
    <p>{loadError}</p>
    <button class="text-button" onclick={load}>Try again</button>
  </div>
{:else if !view}
  <div class="centered"><p>Reading folder…</p></div>
{:else}
  <main>
    <header>
      <span class="settings-toggle" data-settings-toggle>
        <IconButton
          title="Settings"
          icon="settings"
          shortcut="Ctrl+,"
          onclick={() => (settingsOpen ? (settingsOpen = false) : openSettings("general"))}
        />
      </span>

      <div class="folder">
        <span class="folder-name">{folderName}</span>
        <span class="folder-path" title={view.folder}>{view.folder}</span>
      </div>

      <div class="history">
        <IconButton title="Undo" icon="undo" shortcut="Ctrl+Z" disabled={!view.canUndo} onclick={undo} />
        <IconButton title="Redo" icon="redo" shortcut="Ctrl+Shift+Z or Ctrl+Y" disabled={!view.canRedo} onclick={redo} />
      </div>

      <button
        class="review"
        class:has-staged={view.staged > 0}
        disabled={view.staged === 0}
        tabindex="-1"
        onmousedown={(e) => e.preventDefault()}
        onclick={openReview}
        title="Review staged files before trashing them  (Ctrl+Enter)"
      >
        <Icon name="tray" size={14} />
        Review trash
        <span class="count">{view.staged}</span>
      </button>
      {#if settingsOpen}
        <SettingsPanel bind:section={settingsSection} bind:holdSpeed onClose={() => (settingsOpen = false)} />
      {/if}
    </header>

    <section class="stage">
      {#if view.current}
        {#key view.current.id}
          <div class="slot" class:behind-shelf={shelfOpen} in:cardIn|global={{ from: entrance }} out:cardOut|global={{ exit }}>
            <Card item={view.current} onOpen={open} onMenu={(x, y) => (menu = { x, y })} />
          </div>
        {/key}
      {:else}
        <div class="slot done">
          <Icon name="done" size={36} />
          <h1>All caught up</h1>
          <p>
            {view.staged === 0
              ? "Every file in this folder has a decision."
              : "Review your staged files when you're ready."}
          </p>
        </div>
      {/if}

      {#if shelfOpen}
        <div class="shelf" transition:fly={{ y: reduceMotion ? 0 : -16, duration: 200 }}>
          <DestinationShelf
            {destinations}
            highlighted={selectedBin}
            onSelect={fileInto}
            onAdd={addFolder}
            onRemove={removeFolder}
            onClose={() => (shelfOpen = false)}
          />
        </div>
      {/if}
    </section>

    <nav class="decisions" class:dimmed={shelfOpen} aria-label="Decide on this file">
      <DecisionButton title="Stage" icon="trash" key="←" consequential
        hint="Stage for trash. You review before anything is deleted.  (←)"
        disabled={!view.current} onclick={() => makeDecision("stage")} />
      <DecisionButton title="Skip" icon="skip" key="␣" hint="Skip. It comes back at the end.  (Space)"
        disabled={!view.current} onclick={() => makeDecision("skip")} />
      <DecisionButton title="File" icon="folder" key="↑" hint="File into a folder. It moves right away, and Undo moves it back.  (↑)"
        disabled={!view.current} onclick={openShelf} />
      <DecisionButton title="Keep" icon="check" key="→" disabled={!view.current} onclick={() => makeDecision("keep")} />
    </nav>

    <footer>
      {#if renaming && view.current}
        <RenameField
          initial={stemOf(view.current)}
          busy={renameBusy}
          error={renameError}
          onSubmit={submitRename}
          onCancel={() => (renaming = false)}
        />
      {:else}
      {#if notice}
        <p class="notice">{notice}</p>
      {/if}
      {#if status}
        <p class="status">{status}</p>
      {/if}
      <p class="counts">
        <span class="strong">{view.remaining}</span> left · {view.sorted} sorted{#if view.trashed > 0}{` · ${view.trashed} trashed`}{/if}
      </p>
      {/if}
    </footer>
  </main>

  {#if menu && view.current}
    <CardMenu
      x={menu.x}
      y={menu.y}
      onOpen={open}
      onReveal={reveal}
      onRename={startRename}
      onCopyName={copyName}
      onClose={() => (menu = null)}
    />
  {/if}

  {#if review}
    <ReviewTrash
      items={review}
      busy={reviewBusy}
      error={reviewError}
      onCancel={() => (review = null)}
      onConfirm={confirmReview}
    />
  {/if}
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: var(--space-large);
    box-sizing: border-box;
    max-width: 808px;
    height: 100%;
    margin: 0 auto;
    padding: var(--space-large) var(--space-xlarge) var(--space-medium);
  }

  header {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-medium);
  }

  .folder {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .folder-name {
    font-size: 13px;
    font-weight: 600;
  }

  .folder-path {
    overflow: hidden;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history {
    display: flex;
    gap: 2px;
  }

  .review {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
  }

  .review:not(:disabled) {
    cursor: pointer;
  }

  .review:hover:not(:disabled) {
    background: color-mix(in srgb, var(--consequence) 16%, transparent);
  }

  .review.has-staged {
    background: color-mix(in srgb, var(--consequence) 10%, transparent);
    color: var(--consequence);
  }

  .count {
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--fill-subtle);
    font-variant-numeric: tabular-nums;
  }

  /* Outgoing and incoming cards share one grid cell so they overlap
     while they animate. */
  .stage {
    position: relative;
    display: grid;
    flex: 1;
    align-items: center;
    min-height: 0;
  }

  .slot {
    grid-area: 1 / 1;
    min-width: 0;
    transition: scale 0.2s ease-out, opacity 0.2s ease-out;
  }

  /* The card steps back while the shelf is open, so the bins are clearly
     what the keys now act on. */
  .behind-shelf {
    scale: 0.85;
  }

  .behind-shelf,
  .dimmed {
    opacity: 0.35;
    pointer-events: none;
  }

  .shelf {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
    z-index: 1;
  }

  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-medium);
    color: var(--text-secondary);
    text-align: center;
  }

  .done h1 {
    margin: 0;
    color: var(--text-primary);
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 500;
  }

  .done p {
    margin: 0;
    font-size: 13px;
  }

  .decisions {
    display: flex;
    gap: var(--space-small);
    transition: opacity 0.2s ease-out;
  }

  footer p {
    margin: 0;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .counts {
    color: var(--text-secondary);
  }

  .strong {
    color: var(--text-primary);
  }

  .status {
    margin-bottom: var(--space-small) !important;
    color: var(--text-secondary);
    font-size: 12px !important;
  }

  .notice {
    margin-bottom: var(--space-small) !important;
    color: var(--consequence);
    font-size: 12px !important;
  }

  .centered {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-small);
    height: 100%;
    padding: var(--space-xlarge);
    box-sizing: border-box;
    color: var(--text-secondary);
    text-align: center;
  }

  .centered h1 {
    margin: 0;
    color: var(--text-primary);
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 500;
  }

  .centered p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .text-button {
    margin-top: var(--space-small);
    padding: var(--space-small) var(--space-large);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-chip);
    background: var(--card-surface);
    color: var(--text-primary);
    font: inherit;
    cursor: pointer;
  }
</style>
