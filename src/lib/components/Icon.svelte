<script lang="ts" module>
  /** Line icons on a 24-unit grid, drawn with the current text colour. */
  const PATHS = {
    check: ["M20 6 9 17l-5-5"],
    trash: [
      "M3 6h18",
      "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6",
      "M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2",
    ],
    skip: ["m15 14 5-5-5-5", "M4 20v-7a4 4 0 0 1 4-4h12"],
    folder: [
      "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
    ],
    undo: ["M9 14 4 9l5-5", "M4 9h10.5a5.5 5.5 0 0 1 0 11H11"],
    redo: ["m15 14 5-5-5-5", "M20 9H9.5a5.5 5.5 0 0 0 0 11H13"],
    tray: [
      "M22 12h-6l-2 3h-4l-2-3H2",
      "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11Z",
    ],
    open: ["M7 7h10v10", "M7 17 17 7"],
    close: ["M18 6 6 18", "m6 6 12 12"],
    "folder-plus": [
      "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
      "M12 10v6",
      "M9 13h6",
    ],
    done: ["M22 11.08V12a10 10 0 1 1-5.93-9.14", "m9 11 3 3L22 4"],
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each PATHS[name] as d}
    <path {d} />
  {/each}
</svg>
