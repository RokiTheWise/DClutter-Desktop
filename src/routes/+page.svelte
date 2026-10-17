<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { cubicIn, cubicOut } from "svelte/easing";
  import Card from "$lib/components/Card.svelte";
  import DecisionButton from "$lib/components/DecisionButton.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import { IMAGE_EXTENSIONS } from "$lib/components/Preview.svelte";
  import { HeldDecision } from "$lib/held";
  import * as session from "$lib/session";
  import type { DecisionKind, SessionView } from "$lib/session";

  type Exit = "left" | "right" | "fade";
  type Entrance = "rise" | "left" | "right";

  let view = $state<SessionView | null>(null);
  /** Couldn't open the folder at all. */
  let loadError = $state<string | null>(null);
  /** Something went wrong mid-session; shown in the footer. */
  let notice = $state<string | null>(null);

  // How the outgoing card leaves and the incoming one arrives. Set just
  // before `view` changes; Svelte reads them when the transitions start.
  let exit = $state<Exit>("fade");
  let entrance = $state<Entrance>("rise");

  const held = new HeldDecision();
  const folderName = $derived(view?.folder.split(/[\\/]/).filter(Boolean).pop() ?? "");

  async function load() {
    loadError = null;
    try {
      view = await session.openQueue();
    } catch (e) {
      loadError = String(e);
    }
  }

  load();

  // Preload the next image so it's already decoded when its card arrives.
  $effect(() => {
    const next = view?.upNext;
    if (next && !next.isDirectory && IMAGE_EXTENSIONS.has(next.extension)) {
      new Image().src = convertFileSrc(next.path);
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
    kind === "keep" ? "right" : kind === "stage" ? "left" : "fade";

  /** Returns false when there's no card to decide on, ending a hold. */
  function makeDecision(kind: DecisionKind): boolean {
    if (!view?.current) return false;
    notice = null;
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
      entrance = change.kind === "keep" ? "right" : change.kind === "stage" ? "left" : "rise";
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

  function open() {
    if (!view?.current) return;
    session.openCurrent().catch((e) => {
      notice = `Couldn't open this file: ${e}`;
    });
  }

  function onkeydown(e: KeyboardEvent) {
    if (!view) return;

    if ((e.ctrlKey || e.metaKey) && !e.altKey) {
      const key = e.key.toLowerCase();
      if (key === "z") {
        e.preventDefault();
        e.shiftKey ? redo() : undo();
      } else if (key === "y") {
        e.preventDefault();
        redo();
      }
      return;
    }
    if (e.altKey || e.ctrlKey || e.metaKey) return;

    switch (e.key) {
      case "ArrowRight":
        e.preventDefault();
        // The held-decision timer paces repeats, not the system.
        if (!e.repeat) held.start(e.key, () => makeDecision("keep"));
        break;
      case "ArrowLeft":
        e.preventDefault();
        if (!e.repeat) held.start(e.key, () => makeDecision("stage"));
        break;
      case " ":
        e.preventDefault();
        if (!e.repeat) makeDecision("skip");
        break;
      case "Enter":
        e.preventDefault();
        if (!e.repeat) open();
        break;
    }
  }

  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const TRAVEL = 760;

  function cardOut(_node: Element, { exit }: { exit: Exit }) {
    if (reduceMotion || exit === "fade") {
      return { duration: 120, css: (t: number) => `opacity: ${t}` };
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
      <div class="folder">
        <span class="folder-name">{folderName}</span>
        <span class="folder-path" title={view.folder}>{view.folder}</span>
      </div>

      <div class="history">
        <IconButton title="Undo" icon="undo" shortcut="Ctrl+Z" disabled={!view.canUndo} onclick={undo} />
        <IconButton title="Redo" icon="redo" shortcut="Ctrl+Shift+Z or Ctrl+Y" disabled={!view.canRedo} onclick={redo} />
      </div>

      <button class="review" class:has-staged={view.staged > 0} disabled tabindex="-1" title="Reviewing and trashing staged files comes with file actions">
        <Icon name="tray" size={14} />
        Review trash
        <span class="count">{view.staged}</span>
      </button>
    </header>

    <section class="stage">
      {#if view.current}
        {#key view.current.id}
          <div class="slot" in:cardIn|global={{ from: entrance }} out:cardOut|global={{ exit }}>
            <Card item={view.current} onOpen={open} />
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
    </section>

    <nav class="decisions" aria-label="Decide on this file">
      <DecisionButton title="Stage" icon="trash" key="←" consequential
        hint="Stage for trash. You review before anything is deleted.  (←)"
        disabled={!view.current} onclick={() => makeDecision("stage")} />
      <DecisionButton title="Skip" icon="skip" key="␣" hint="Skip. It comes back at the end.  (Space)"
        disabled={!view.current} onclick={() => makeDecision("skip")} />
      <DecisionButton title="File" icon="folder" key="↑" hint="Filing into folders comes with file actions"
        disabled onclick={() => {}} />
      <DecisionButton title="Keep" icon="check" key="→" disabled={!view.current} onclick={() => makeDecision("keep")} />
    </nav>

    <footer>
      {#if notice}
        <p class="notice">{notice}</p>
      {/if}
      <p class="counts">
        <span class="strong">{view.remaining}</span> left · {view.sorted} sorted
      </p>
    </footer>
  </main>
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
    display: grid;
    flex: 1;
    align-items: center;
    min-height: 0;
  }

  .slot {
    grid-area: 1 / 1;
    min-width: 0;
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
