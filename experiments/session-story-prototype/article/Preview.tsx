// THROWAWAY: three Kinograph compositions in an isolated copy of the actual
// article, switched via ?prototype=sessions&variant=A|B|C. No live mutations.
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react"
import "./preview.css"

const variants = [
  { id: "A", name: "Paired workspaces", note: "Both workspaces stay in view. The conversation accumulates; the browser follows the work." },
  { id: "B", name: "Focus handoff", note: "The browser takes more space when it becomes useful. The same session stays anchored at the left." },
  { id: "C", name: "Tool bridge", note: "A condensed turn above, a browser result below. The attached connection makes the causal relationship explicit." },
] as const
const beats = [
  [0, "Ask"], [3, "Configure"], [6, "Connect"], [9, "Browse"],
  [13, "Inspect"], [17, "Code Mode"], [21, "Continue"],
] as const
const assets = "/kinograph-session-prototype"

export function KinographPrototype({ original }: { original: ReactNode }) {
  if (!import.meta.env.DEV || new URLSearchParams(location.search).get("prototype") !== "sessions") return original
  return <Comparison original={original} />
}

function Comparison({ original }: { original: ReactNode }) {
  const [index, setIndex] = useState(() => Math.max(0, variants.findIndex(v => v.id === new URLSearchParams(location.search).get("variant"))))
  const [showOriginal, setShowOriginal] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [error, setError] = useState(false)
  const video = useRef<HTMLVideoElement>(null)
  const resume = useRef({ time: 0, playing: false, speed: 1 })
  const selected = variants[index]
  const select = useCallback((index: number) => {
    const current = video.current
    if (current) resume.current = { time: current.currentTime, playing: !current.paused, speed: current.playbackRate }
    setIndex(index)
    setError(false)
    const url = new URL(location.href)
    url.searchParams.set("variant", variants[index].id)
    history.replaceState(null, "", url)
    console.info("Kinograph comparison", { variant: variants[index].id, ...resume.current, delivery: "native-rendered video", fixture: true })
  }, [elapsed])
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || event.target instanceof HTMLElement && event.target.closest("input,textarea,select,video,[contenteditable]")) return
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return
      event.preventDefault()
      select((index + (event.key === "ArrowRight" ? 1 : variants.length - 1)) % variants.length)
    }
    window.addEventListener("keydown", key)
    return () => window.removeEventListener("keydown", key)
  }, [index, select])
  const seek = (time: number) => {
    if (!video.current) return
    video.current.pause()
    video.current.currentTime = time
  }
  return <section id="kinograph-options" className="kg-comparison" data-kinograph-prototype>
    <div className="kg-prototype-label"><span>Kinograph · visual prototype</span><button onClick={() => {
      if (video.current) {
        resume.current = { time: video.current.currentTime, playing: false, speed: video.current.playbackRate }
        video.current.pause()
      }
      setShowOriginal(v => !v)
    }}>{showOriginal ? "Back to Kinograph" : "Compare current version"}</button></div>
    {showOriginal ? original : <>
      <video key={selected.id} ref={video} className="kg-prototype-video" src={`${assets}/${selected.id}.mp4`} poster={`${assets}/${selected.id}.png`} controls playsInline preload="metadata"
        aria-label={`${selected.name}: illustrated OpenCode and browser session`}
        onLoadedMetadata={event => {
          const element = event.currentTarget
          element.currentTime = resume.current.time
          element.playbackRate = resume.current.speed
          if (resume.current.playing) void element.play().catch(() => setPlaying(false))
        }}
        onTimeUpdate={event => setElapsed(event.currentTarget.currentTime)}
        onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onError={() => setError(true)} />
      {error && <p role="alert">The rendered clip is missing. Run the prototype’s render command, then restart its isolated preview.</p>}
      <div className="kg-beats" role="group" aria-label="Story beats">
        {beats.map(([at, label], i) => <button key={label} aria-pressed={elapsed >= at && elapsed < (beats[i + 1]?.[0] ?? 26)} onClick={() => seek(at + .8)}>{label}</button>)}
        <select aria-label="Playback speed" defaultValue="1" onChange={event => { if (video.current) video.current.playbackRate = Number(event.target.value) }}><option value="1">1×</option><option value="0.5">½×</option><option value="0.25">¼×</option></select>
      </div>
      <p className="kg-option-note">{selected.note}</p>
      <p className="kg-disclosure">Illustrated session; fictional apartments and prices. Native Rust-rendered pixels, not a live OpenCode connection or a browser recording. Silent, 1920 × 1080, 60 fps. The OpenCode session stays open; the Code Mode change recreates the browser context.</p>
      <details className="kg-transcript"><summary>Read the illustrated sequence</summary><ol>
        <li>Ask OpenCode to connect Chrome DevTools and search near the Panhandle.</li>
        <li>OpenCode edits the project configuration. New browser tools become available without reopening the session.</li>
        <li>The browser shows three fictional apartments. The Oak Street listing is $48,000 a month.</li>
        <li>A later Code Mode change causes the old native call to fail. New instructions arrive.</li>
        <li>OpenCode discovers the browser tools through Code Mode and reopens the search in browser context 02. The original OpenCode session continues.</li>
      </ol></details>
    </>}
    <nav className="kg-switcher" aria-label="Prototype variants">
      <button aria-label="Previous variant" onClick={() => select((index + variants.length - 1) % variants.length)}>←</button>
      <span><strong>{selected.id} — {selected.name}</strong><small>{showOriginal ? "Current article simulation" : `${playing ? "Playing" : "Paused"} · ${elapsed.toFixed(1)}s · switch at the same time`}</small></span>
      <button aria-label="Next variant" onClick={() => select((index + 1) % variants.length)}>→</button>
    </nav>
  </section>
}
