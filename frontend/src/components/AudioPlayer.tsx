"use client";

import { Pause, Play } from "lucide-react";
import type { SessionAudioPlayer } from "@/hooks/useAudioPlayer";

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  return `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;
}

export function AudioPlayer({ player }: { player: SessionAudioPlayer }) {
  const disabled = player.status !== "ready";
  return (
    <section aria-label="Session mixed-track player" className="border-b border-gray-200 bg-gray-50 px-4 py-3">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={player.isPlaying ? player.pause : () => void player.play()}
          disabled={disabled}
          aria-label={player.isPlaying ? "Pause Session audio" : "Play Session audio"}
          className="rounded-full border border-gray-300 bg-white p-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
        >
          {player.isPlaying ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
        </button>
        <span className="w-11 font-mono text-xs">{formatTime(player.currentTime)}</span>
        <input
          type="range"
          min={0}
          max={Math.max(player.duration, 0)}
          step={0.1}
          value={Math.min(player.currentTime, player.duration)}
          onChange={(event) => player.seek(Number(event.target.value))}
          disabled={disabled}
          aria-label="Session audio position"
          className="min-w-0 flex-1 accent-blue-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
        />
        <span className="w-11 font-mono text-xs">{formatTime(player.duration)}</span>
      </div>
      {player.status === "loading" && <p aria-live="polite" className="mt-2 text-xs text-gray-600">Loading Session audio…</p>}
      {player.status === "unavailable" && <p className="mt-2 text-xs text-gray-600">Session audio is unavailable.</p>}
      {player.error && <p role="alert" className="mt-2 text-xs text-red-800">{player.error}</p>}
    </section>
  );
}
