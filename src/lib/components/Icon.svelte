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
    // Project tabs.
    grid: "M2.5 2.5h4.5v4.5H2.5zM9 2.5h4.5v4.5H9zM2.5 9h4.5v4.5H2.5zM9 9h4.5v4.5H9z",
    branch:
      "M5 2.5v8M5 13.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zM11 6.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zM11 6.5c0 2.5-6 2-6 4",
    lines: "M3 4h10M3 8h10M3 12h6",
    // Files tree.
    folderOpen:
      "M2 12.5v-8A1.5 1.5 0 0 1 3.5 3h3L8 4.5h3.5A1.5 1.5 0 0 1 13 6v1M2 12.5l1.7-4.5A1.5 1.5 0 0 1 5.1 7h8.2a.7.7 0 0 1 .7 1l-1.5 4.5z",
    file: "M4 2h5l3 3v9H4zM9 2v3h3",
    fileCode: "M4 2h5l3 3v9H4zM9 2v3h3M7 8.5 5.8 10 7 11.5M9 8.5l1.2 1.5L9 11.5",
    fileText: "M4 2h5l3 3v9H4zM9 2v3h3M6 8.5h4M6 11h4",
    braces:
      "M6 3c-1.4 0-1.6.8-1.6 2s-.4 2.4-1.4 3c1 .6 1.4 1.8 1.4 3s.2 2 1.6 2M10 3c1.4 0 1.6.8 1.6 2s.4 2.4 1.4 3c-1 .6-1.4 1.8-1.4 3s-.2 2-1.6 2",
    image: "M2.5 3.5h11v9h-11zM2.5 10.5 6 7l3 3 1.5-1.5 3 3M10.5 6v.1",
    lock: "M4 7.5h8v6H4zM5.5 7.5v-2a2.5 2.5 0 0 1 5 0v2",
    eye: "M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
    eyeOff:
      "M2.5 2.5l11 11M6.6 3.7A6.6 6.6 0 0 1 8 3.5c4 0 6.5 4.5 6.5 4.5a11.6 11.6 0 0 1-1.8 2.3M10.4 11.9A6.3 6.3 0 0 1 8 12.5C4 12.5 1.5 8 1.5 8a11.5 11.5 0 0 1 2.4-2.9M6.6 6.6a2 2 0 0 0 2.8 2.8",
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
