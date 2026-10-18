<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { cubicIn, cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import Card from "$lib/components/Card.svelte";
  import ConfirmDialog, { type DialogAction } from "$lib/components/ConfirmDialog.svelte";
  import DecisionButton from "$lib/components/DecisionButton.svelte";
  import DestinationShelf from "$lib/components/DestinationShelf.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import PopupMenu, { type MenuItem } from "$lib/components/PopupMenu.svelte";
  import { IMAGE_EXTENSIONS } from "$lib/components/Preview.svelte";
  import RenameField from "$lib/components/RenameField.svelte";
  import ReviewTrash from "$lib/components/ReviewTrash.svelte";
  import SettingsPanel, { type SettingsSection } from "$lib/components/SettingsPanel.svelte";
  import { HeldDecision } from "$lib/held";
  import { loadHoldSpeed, loadSwipeReversed, saveHoldSpeed, saveSwipeReversed } from "$lib/settings";
  import { SWIPE, SwipeTracker, isMouseWheel, type Intent } from "$lib/swipe";
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

  /** The open pop-up menu, if any, and where. */
  let popup = $state<{ kind: "card" | "folder" | "more"; x: number; y: number } | null>(null);
  let folderChoices = $state<session.FolderMenu | null>(null);
  /** An open question: Start Over, or switching with files staged. */
  let confirm = $state<{ title: string; message: string; actions: DialogAction[] } | null>(null);

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
  let swipeReversed = $state(loadSwipeReversed());
  $effect(() => saveSwipeReversed(swipeReversed));

  // Where the card sits while a swipe or drag carries it: -1 to 1 of a full
  // swipe. `settling` animates it back to rest when a gesture stops short.
  let swipe = $state(0);
  let settling = $state(false);
  /** Where the outgoing card was when its decision was made, so it leaves
   *  from there instead of jumping back to the middle first. */
  let exitFrom = $state(0);
  /** Pixels the card travels at a full swipe. */
  const SWIPE_TRAVEL = 200;

  let settingsOpen = $state(false);
  let settingsSection = $state<SettingsSection>("general");

  function openSettings(section: SettingsSection) {
    held.stop();
    shelfOpen = false;
    popup = null;
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
  /** Returns false when there's no card to decide on, ending a hold.
   *  `fromSwipe` keeps the card where the gesture left it as it goes. */
  function makeDecision(kind: DecisionKind, fromSwipe = false): boolean {
    if (!view?.current || review || shelfOpen || renaming) return false;
    notice = null;
    status = null;
    const from = fromSwipe ? swipe : 0;
    enqueue(async () => {
      const next = await session.decide(kind);
      exit = exitFor(kind);
      exitFrom = from;
      entrance = "rise";
      settling = false;
      swipe = 0;
      view = next;
    });
    return true;
  }

  // Touchpad swipes and dragging the card.

  const tracker = new SwipeTracker(() => swipeReversed, () => shelfOpen);

  // The app reads finger contacts from the touchpad itself, so a gesture
  // starts on touch and ends on lift instead of after a quiet moment.
  $effect(() => {
    const stop = listen<number>("touchpad", (e) => act(tracker.fingers(e.payload)));
    return () => {
      stop.then((unlisten) => unlisten());
    };
  });
  let quietTimer: ReturnType<typeof setTimeout> | undefined;

  /** Something else has the input: a dialog, a menu, a text field. */
  const gesturesBlocked = () => !view || !!review || renaming || !!popup || settingsOpen || !!confirm;

  function settleBack() {
    if (swipe === 0) return;
    settling = true;
    swipe = 0;
  }

  function act(intents: Intent[]) {
    for (const intent of intents) {
      switch (intent.type) {
        case "progress":
          if (view?.current && !shelfOpen) {
            settling = false;
            swipe = intent.value;
          }
          break;
        case "keep":
        case "stage":
          if (!makeDecision(intent.type, true)) settleBack();
          break;
        case "cancel":
          settleBack();
          break;
        case "up":
          if (shelfOpen) confirmShelf();
          else openShelf();
          break;
        case "down":
          if (shelfOpen) shelfOpen = false;
          break;
        case "left":
        case "right":
          if (shelfOpen) stepBin(intent.type === "right" ? 1 : -1);
          break;
        case "release":
          // Let go on the shelf: file into the highlighted bin.
          if (shelfOpen) confirmShelf();
          break;
      }
    }
  }

  function onwheel(e: WheelEvent) {
    // Mouse wheels, line-based scrolling and pinch-to-zoom aren't swipes.
    if (e.deltaMode !== 0 || e.ctrlKey || isMouseWheel(e as WheelEvent & { wheelDeltaX?: number; wheelDeltaY?: number })) return;
    if (gesturesBlocked()) {
      act(tracker.reset());
      return;
    }
    held.stop();
    act(tracker.wheel({ dx: e.deltaX, dy: e.deltaY, t: performance.now() }));
    // Windows never says the fingers lifted; a quiet moment ends a gesture.
    clearTimeout(quietTimer);
    quietTimer = setTimeout(() => act(tracker.tick(performance.now())), SWIPE.quietMs + 20);
  }

  /** A mouse or finger press on the card, until it's clear it's a drag. */
  let drag: { id: number; x: number; active: boolean } | null = null;

  function onCardPointerDown(e: PointerEvent) {
    if (e.button !== 0 || gesturesBlocked() || shelfOpen) return;
    if ((e.target as HTMLElement).closest("button, input, a")) return;
    drag = { id: e.pointerId, x: e.clientX, active: false };
  }

  function onCardPointerMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = e.clientX - drag.x;
    if (!drag.active) {
      // A few pixels of slack, so a click or double-click stays a click.
      if (Math.abs(dx) < 6) return;
      drag.active = true;
      // Keeps the drag going if the pointer leaves the card. If the browser
      // won't capture this pointer, the drag still works inside the card.
      try {
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
      } catch {}
      held.stop();
    }
    settling = false;
    swipe = Math.max(-1, Math.min(1, dx / SWIPE_TRAVEL));
  }

  function onCardPointerUp(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const dragged = drag.active;
    drag = null;
    if (!dragged) return;
    if (Math.abs(swipe) >= SWIPE.commitAt) {
      if (!makeDecision(swipe > 0 ? "keep" : "stage", true)) settleBack();
    } else {
      settleBack();
    }
  }

  function onCardPointerCancel() {
    drag = null;
    settleBack();
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

  const sameFolder = (a: string, b: string) =>
    a.replace(/[\\/]+$/, "").toLowerCase() === b.replace(/[\\/]+$/, "").toLowerCase();
  const filesWord = (n: number) => (n === 1 ? "1 file" : `${n} files`);

  const cardItems = (): MenuItem[] => [
    { label: "Open", hint: "Enter", onSelect: open },
    { label: "Show in Explorer", onSelect: reveal },
    "separator",
    { label: "Rename…", hint: "F2", onSelect: startRename },
    { label: "Copy name", onSelect: copyName },
  ];

  function folderItems(): MenuItem[] {
    const choices = folderChoices;
    if (!choices || !view) return [];
    const here = view.folder;
    const items: MenuItem[] = [
      { label: "Downloads", checked: sameFolder(here, choices.downloads), onSelect: () => requestSwitch(choices.downloads) },
    ];
    if (choices.recents.length > 0) {
      items.push("separator");
      for (const folder of choices.recents) {
        items.push({ label: folder, checked: sameFolder(here, folder), onSelect: () => requestSwitch(folder) });
      }
    }
    items.push("separator", { label: "Choose Folder…", onSelect: chooseFolder });
    return items;
  }

  const moreItems = (): MenuItem[] => [
    { label: "Start Over…", onSelect: confirmStartOver },
    { label: "Controls & keys", hint: "?", onSelect: () => openSettings("controls") },
  ];

  async function openFolderMenu(e: MouseEvent) {
    const anchor = (e.currentTarget as HTMLElement).getBoundingClientRect();
    held.stop();
    folderChoices = await session.folderMenu().catch(() => null);
    popup = { kind: "folder", x: anchor.left, y: anchor.bottom + 4 };
  }

  function openMoreMenu(e: MouseEvent) {
    const anchor = (e.currentTarget as HTMLElement).getBoundingClientRect();
    held.stop();
    popup = { kind: "more", x: anchor.right, y: anchor.bottom + 4 };
  }

  /** Switches to `folder`, first asking about any files still staged. */
  function requestSwitch(folder: string) {
    if (!view || sameFolder(folder, view.folder)) return;
    const staged = view.staged;
    if (staged === 0) {
      switchTo(folder);
      return;
    }
    confirm = {
      title: "Switch folders?",
      message: `${staged === 1 ? "1 file is" : `${staged} files are`} staged for trash in this folder. Files you already filed into folders stay where they are either way.`,
      actions: [
        {
          label: "Leave them staged",
          kind: "secondary",
          onSelect: () => {
            confirm = null;
            switchTo(folder);
          },
        },
        {
          label: `Trash ${filesWord(staged)}`,
          onSelect: () => {
            confirm = null;
            trashThenSwitch(folder);
          },
        },
      ],
    };
  }

  function switchTo(folder: string) {
    held.stop();
    shelfOpen = false;
    renaming = false;
    notice = null;
    status = null;
    enqueue(async () => {
      // Each folder has its own saved session, so this picks up where the
      // last visit to it left off.
      const next = await session.openQueue(folder);
      exit = "fade";
      entrance = "rise";
      view = next;
    });
  }

  /** Trashes everything staged, then switches. If anything couldn't be
   *  trashed, stays and shows the review list with the error instead, so
   *  a partial failure is never mistaken for success. */
  function trashThenSwitch(folder: string) {
    enqueue(async () => {
      const outcome = await session.commitTrash([]);
      view = outcome.view;
      if (outcome.error) {
        reviewError = outcome.error;
        review = await session.stagedFiles();
        if (review.length === 0) review = null;
      } else {
        switchTo(folder);
      }
    });
  }

  function chooseFolder() {
    enqueue(async () => {
      const folder = await session.pickSourceFolder();
      if (folder) requestSwitch(folder);
    });
  }

  /** Says what Start Over will do to files on disk: nobody can eyeball
   *  twenty files coming back. */
  function confirmStartOver() {
    if (!view) return;
    const lines = ["Every keep, trash and skip decision in this folder becomes undecided again."];
    const filed = view.filedThisSession;
    if (filed > 0) {
      lines.push(`${filesWord(filed)} you filed into ${filed === 1 ? "a folder moves" : "folders move"} back to ${folderName}.`);
    }
    if (view.trashed > 0) lines.push("Files already in the Recycle Bin stay there.");
    confirm = {
      title: "Start over?",
      message: lines.join(" "),
      actions: [
        {
          label: "Start over",
          onSelect: () => {
            confirm = null;
            startOver();
          },
        },
      ],
    };
  }

  function startOver() {
    held.stop();
    notice = null;
    status = null;
    enqueue(async () => {
      const outcome = await session.startOver();
      exit = "fade";
      entrance = "rise";
      view = outcome.view;
      status = outcome.movedBack > 0 ? `Started over. ${filesWord(outcome.movedBack)} moved back.` : "Started over.";
      if (outcome.error) notice = outcome.error;
    });
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
    if (!view || review || renaming || popup || settingsOpen || confirm) return;

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

  function cardOut(_node: Element, { exit, from }: { exit: Exit; from: number }) {
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
    // Leave from wherever a swipe or drag had carried the card.
    const start = from * SWIPE_TRAVEL;
    const tilt = from * 4;
    return {
      duration: 140,
      easing: cubicIn,
      css: (t: number, u: number) =>
        `transform: translateX(${start + u * (dx - start)}px) rotate(${tilt}deg); opacity: ${t}`,
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

<svelte:window
  {onkeydown}
  {onwheel}
  onkeyup={(e) => held.stop(e.key)}
  onblur={() => {
    held.stop();
    act(tracker.reset());
  }}
/>

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
        <!-- Reads as a location, but it's a menu: click to switch folders. -->
        <button
          class="folder-name"
          tabindex="-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={openFolderMenu}
          title="Sorting {view.folder}. Click to switch folders"
        >
          {folderName}
          <Icon name="chevron-down" size={12} />
        </button>
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

      <IconButton title="More" icon="more" onclick={openMoreMenu} />
      {#if settingsOpen}
        <SettingsPanel
          bind:section={settingsSection}
          bind:holdSpeed
          bind:swipeReversed
          onClose={() => (settingsOpen = false)}
        />
      {/if}
    </header>

    <section class="stage">
      {#if view.current}
        {#key view.current.id}
          <div
            class="slot"
            role="group"
            aria-label={view.current.name}
            class:behind-shelf={shelfOpen}
            class:settling
            style:transform={swipe ? `translateX(${swipe * SWIPE_TRAVEL}px) rotate(${swipe * 4}deg)` : null}
            style:opacity={swipe ? 1 - Math.min(Math.abs(swipe) * 0.35, 0.35) : null}
            onpointerdown={onCardPointerDown}
            onpointermove={onCardPointerMove}
            onpointerup={onCardPointerUp}
            onpointercancel={onCardPointerCancel}
            in:cardIn|global={{ from: entrance }}
            out:cardOut|global={{ exit, from: exitFrom }}
          >
            <Card item={view.current} onOpen={open} onMenu={(x, y) => (popup = { kind: "card", x, y })} />
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

  {#if popup && (popup.kind !== "card" || view.current)}
    <PopupMenu
      x={popup.x}
      y={popup.y}
      alignRight={popup.kind === "more"}
      items={popup.kind === "card" ? cardItems() : popup.kind === "folder" ? folderItems() : moreItems()}
      onClose={() => (popup = null)}
    />
  {/if}

  {#if confirm}
    <ConfirmDialog
      title={confirm.title}
      message={confirm.message}
      actions={confirm.actions}
      onCancel={() => (confirm = null)}
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
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    gap: var(--space-unit);
    max-width: 100%;
    margin: -2px -6px;
    padding: 2px 6px;
    border: 0;
    border-radius: var(--radius-chip);
    background: none;
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .folder-name:hover {
    background: var(--fill-hover);
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
    /* The card's own drag handling owns touches; the page doesn't scroll. */
    touch-action: none;
  }

  /* Back to rest after a swipe or drag that stopped short. While a gesture
     carries the card it follows exactly, unanimated. */
  .slot.settling {
    transition: transform 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.2), opacity 0.22s ease-out, scale 0.2s ease-out;
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
