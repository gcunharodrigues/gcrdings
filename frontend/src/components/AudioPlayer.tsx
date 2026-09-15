"use client";

import { Pause, Play } from "lucide-react";
import { PLAYBACK_RATES, type PlaybackRate, type SessionAudioPlayer } from "@/hooks/useAudioPlayer";

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  return `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;
}

/**
 * The scrub bar draws the recording's own shape, so silence and speech are
 * distinguishable before playing anything. The range input stays on top and
 * unstyled-but-transparent: it remains the keyboard and screen-reader control,
 * while the bars are decoration behind it.
 */
function WaveformScrubber({ player }: { player: SessionAudioPlayer }) {
  const disabled = player.status !== "ready";
  const progress = player.duration > 0 ? player.currentTime / player.duration : 0;

  return (
    <div className="relative min-w-0 flex-1">
      <div aria-hidden="true" className="flex h-8 items-center gap-px overflow-hidden">
        {player.waveform.map((peak, index) => {
          const played = index / player.waveform.length <= progress;
          return (
            <span
              key={index}
              className={`min-w-px flex-1 rounded-sm ${played ? "bg-blue-600" : "bg-border"}`}
              style={{ height: `${Math.max(8, peak * 100)}%` }}
            />
          );
        })}
        {player.waveform.length === 0 && <span className="h-px w-full bg-border" />}
      </div>
      <input
        type="range"
        min={0}
        max={Math.max(player.duration, 0)}
        step={0.1}
        value={Math.min(player.currentTime, player.duration)}
        onChange={(event) => player.seek(Number(event.target.value))}
        disabled={disabled}
        aria-label="Session audio position"
        aria-valuetext={`${formatTime(player.currentTime)} of ${formatTime(player.duration)}`}
        className="absolute inset-0 h-full w-full cursor-pointer opacity-0 focus-visible:opacity-100 focus-visible:accent-blue-600"
      />
    </div>
  );
}

export function AudioPlayer({ player }: { player: SessionAudioPlayer }) {
  const disabled = player.status !== "ready";

  return (
    <section aria-label="Session mixed-track player" className="border-b border-border bg-muted/40 px-4 py-3">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={player.isPlaying ? player.pause : () => void player.play()}
          disabled={disabled}
          aria-label={player.isPlaying ? "Pause Session audio" : "Play Session audio"}
          className="rounded-full border border-border bg-card p-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
        >
          {player.isPlaying ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
        </button>
        <span className="w-11 font-mono text-xs tabular-nums">{formatTime(player.currentTime)}</span>

        <WaveformScrubber player={player} />

        <span className="w-11 font-mono text-xs tabular-nums">{formatTime(player.duration)}</span>

        <label className="flex items-center gap-1 text-xs text-muted-foreground">
          <span className="sr-only">Playback speed</span>
          <select
            value={player.rate}
            onChange={(event) => player.setRate(Number(event.target.value) as PlaybackRate)}
            disabled={disabled}
            aria-label="Playback speed"
            className="rounded border border-border bg-card px-1.5 py-1 font-mono text-xs tabular-nums focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
          >
            {PLAYBACK_RATES.map((rate) => (
              <option key={rate} value={rate}>{rate}×</option>
            ))}
          </select>
        </label>
      </div>

      {player.status === "loading" && <p aria-live="polite" className="mt-2 text-xs text-muted-foreground">Loading Session audio…</p>}
      {player.status === "unavailable" && <p className="mt-2 text-xs text-muted-foreground">Session audio is unavailable.</p>}
      {player.error && <p role="alert" className="mt-2 text-xs text-red-800">{player.error}</p>}
    </section>
  );
}
