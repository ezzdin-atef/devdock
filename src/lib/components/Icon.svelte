<script lang="ts" module>
  // Line icons on a 16x16 grid, round caps (DESIGN.md section 7). Stroke is 1.75 at
  // 13-14 px and 1.6 above, so small icons keep their contrast.
  const PATHS = {
    folder:
      "M2 4.5A1.5 1.5 0 0 1 3.5 3h3l1.5 1.5h4.5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z",
    code: "M5.5 4.5 2 8l3.5 3.5M10.5 4.5 14 8l-3.5 3.5",
    plus: "M8 3v10M3 8h10",
    layers: "M8 2.5 14 5.5 8 8.5 2 5.5zM2 8.5l6 3 6-3M2 11l6 3 6-3",
    play: "M5 3.5v9l7-4.5z",
    plug: "M6 2v3M10 2v3M4 5h8v2.5a4 4 0 0 1-8 0zM8 11.5V14",
    terminal: "M3 4.5 6.5 8 3 11.5M8 12h5",
    sliders: "M3 4.5h10M3 8h10M3 11.5h10M6 3v3M10 6.5v3M5 10v3",
    alert: "M8 2.5 14 13H2zM8 6.5v3M8 11.2v.1",
    x: "M4 4l8 8M12 4l-8 8",
    search: "M7 12A5 5 0 1 0 7 2a5 5 0 0 0 0 10zM10.6 10.6 14 14",
    star: "M8 2.2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.4l-3.6 1.9.7-4L2.2 6.5l4-.6z",
    refresh: "M13 8a5 5 0 1 1-1.5-3.6M13 2.5v3h-3",
    kebab: "M8 3.5v.1M8 8v.1M8 12.5v.1",
    check: "M3 8.5 6.5 12 13 4.5",
    sun: "M8 10.7a2.7 2.7 0 1 0 0-5.4 2.7 2.7 0 0 0 0 5.4zM8 1.8v1.3M8 12.9v1.3M1.8 8h1.3M12.9 8h1.3M3.6 3.6l.9.9M11.5 11.5l.9.9M3.6 12.4l.9-.9M11.5 4.5l.9-.9",
    moon: "M13.3 9.7A5.6 5.6 0 0 1 6.3 2.7a5.6 5.6 0 1 0 7 7z",
    monitor: "M2.5 3.5h11v7.5h-11zM6 14h4M8 11v3",
    stop: "M4.5 4.5h7v7h-7z",
    chevron: "M4.5 6.2 8 9.7l3.5-3.5",
    download: "M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13.5h10",
    copy: "M5.5 5.5h7v7h-7zM10.5 5.5v-2h-7v7h2",
  } as const;
  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    filled = false,
  }: { name: IconName; size?: number; filled?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width={name === "kebab" ? 2.6 : size <= 14 ? 1.75 : 1.6}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name]} fill={filled || name === "play" || name === "stop" ? "currentColor" : "none"} />
</svg>
