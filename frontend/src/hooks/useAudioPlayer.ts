import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { validateSessionSeek } from "@/lib/session-audio";

export type AudioPlayerStatus = "loading" | "ready" | "unavailable" | "error";

export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 1.75, 2] as const;
export type PlaybackRate = (typeof PLAYBACK_RATES)[number];

/** Peak amplitude per bucket, 0..1, for the scrub bar. */
export type Waveform = number[];

export interface SessionAudioPlayer {
  status: AudioPlayerStatus;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  error: string | null;
  rate: PlaybackRate;
  waveform: Waveform;
  play: () => Promise<void>;
  pause: () => void;
  seek: (seconds: number) => void;
  seekAndPlay: (seconds: number) => Promise<boolean>;
  setRate: (rate: PlaybackRate) => void;
}

const WAVEFORM_BUCKETS = 220;

/**
 * Peak per bucket rather than RMS: speech is bursty, and averaging flattens a
 * recording into a featureless band that tells the reader nothing about where
 * anyone actually spoke.
 */
function computeWaveform(buffer: AudioBuffer): Waveform {
  const channel = buffer.getChannelData(0);
  const bucketSize = Math.floor(channel.length / WAVEFORM_BUCKETS) || 1;
  const peaks: number[] = [];

  for (let bucket = 0; bucket < WAVEFORM_BUCKETS; bucket += 1) {
    const start = bucket * bucketSize;
    let peak = 0;
    for (let i = start; i < start + bucketSize && i < channel.length; i += 1) {
      const value = Math.abs(channel[i]);
      if (value > peak) peak = value;
    }
    peaks.push(peak);
  }

  const loudest = Math.max(...peaks, 0.0001);
  return peaks.map((peak) => peak / loudest);
}

export function useAudioPlayer(meetingId: string | null): SessionAudioPlayer {
  const [status, setStatus] = useState<AudioPlayerStatus>("loading");
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [rate, setRateState] = useState<PlaybackRate>(1);
  const [waveform, setWaveform] = useState<Waveform>([]);
  const rateRef = useRef<PlaybackRate>(1);
  const contextRef = useRef<AudioContext | null>(null);
  const bufferRef = useRef<AudioBuffer | null>(null);
  const sourceRef = useRef<AudioBufferSourceNode | null>(null);
  const offsetRef = useRef(0);
  const startedAtRef = useRef(0);
  const frameRef = useRef<number>();
  const generationRef = useRef(0);
  const sourceGenerationRef = useRef(0);
  const statusRef = useRef(status);
  statusRef.current = status;

  const stopSource = useCallback(() => {
    sourceGenerationRef.current += 1;
    if (frameRef.current !== undefined) cancelAnimationFrame(frameRef.current);
    frameRef.current = undefined;
    if (sourceRef.current) {
      sourceRef.current.onended = null;
      try { sourceRef.current.stop(); } catch {}
      sourceRef.current.disconnect();
      sourceRef.current = null;
    }
    setIsPlaying(false);
  }, []);

  useEffect(() => {
    const generation = ++generationRef.current;
    stopSource();
    bufferRef.current = null;
    offsetRef.current = 0;
    setCurrentTime(0);
    setDuration(0);
    setError(null);
    setWaveform([]);
    if (!meetingId) {
      setStatus("unavailable");
      return;
    }

    setStatus("loading");
    void invoke<number[]>("api_read_session_audio", { meetingId })
      .then(async (bytes) => {
        const context = contextRef.current ?? new AudioContext();
        contextRef.current = context;
        const decoded = await context.decodeAudioData(new Uint8Array(bytes).buffer);
        if (generation !== generationRef.current) return;
        bufferRef.current = decoded;
        setDuration(decoded.duration);
        setWaveform(computeWaveform(decoded));
        setStatus("ready");
      })
      .catch((reason: unknown) => {
        if (generation !== generationRef.current) return;
        const message = reason instanceof Error ? reason.message : String(reason);
        setError(message);
        setStatus(/unavailable|missing/i.test(message) ? "unavailable" : "error");
      });

    return () => {
      generationRef.current += 1;
      stopSource();
    };
  }, [meetingId, stopSource]);

  useEffect(() => () => {
    void contextRef.current?.close();
  }, []);

  const playFromOffset = useCallback(async () => {
    const context = contextRef.current;
    const buffer = bufferRef.current;
    if (!context || !buffer || statusRef.current !== "ready") {
      setError("Session audio is not ready.");
      return;
    }
    try {
      if (context.state === "suspended") await context.resume();
      stopSource();

      const offset = offsetRef.current >= buffer.duration ? 0 : offsetRef.current;
      const source = context.createBufferSource();
      const sourceGeneration = ++sourceGenerationRef.current;
      source.buffer = buffer;
      source.playbackRate.value = rateRef.current;
      source.connect(context.destination);
      sourceRef.current = source;
      // Wall-clock advances faster than the track at rate > 1, so the offset has
      // to be divided by the rate or the position readout drifts.
      startedAtRef.current = context.currentTime - offset / rateRef.current;
      offsetRef.current = offset;
      setCurrentTime(offset);
      setIsPlaying(true);
      setError(null);

      const update = () => {
        if (sourceGeneration !== sourceGenerationRef.current) return;
        const next = Math.min((context.currentTime - startedAtRef.current) * rateRef.current, buffer.duration);
        offsetRef.current = next;
        setCurrentTime(next);
        frameRef.current = requestAnimationFrame(update);
      };
      source.onended = () => {
        if (sourceGeneration !== sourceGenerationRef.current) return;
        sourceRef.current = null;
        offsetRef.current = 0;
        setCurrentTime(0);
        setIsPlaying(false);
        if (frameRef.current !== undefined) cancelAnimationFrame(frameRef.current);
      };
      source.start(0, offset);
      frameRef.current = requestAnimationFrame(update);
    } catch (reason) {
      stopSource();
      setError(reason instanceof Error ? reason.message : "Failed to play Session audio.");
    }
  }, [stopSource]);

  const pause = useCallback(() => {
    const context = contextRef.current;
    if (context && sourceRef.current) {
      offsetRef.current = Math.min(
        context.currentTime - startedAtRef.current,
        bufferRef.current?.duration ?? 0,
      );
      setCurrentTime(offsetRef.current);
    }
    stopSource();
  }, [stopSource]);

  const seek = useCallback((seconds: number) => {
    const result = validateSessionSeek(seconds, duration, status === "ready");
    if (!result.ok) return;
    const resume = isPlaying;
    stopSource();
    offsetRef.current = result.seconds;
    setCurrentTime(result.seconds);
    if (resume) void playFromOffset();
  }, [duration, isPlaying, playFromOffset, status, stopSource]);

  const seekAndPlay = useCallback(async (seconds: number) => {
    const result = validateSessionSeek(seconds, duration, status === "ready");
    if (!result.ok) {
      setError(result.reason === "past-end"
        ? "This timestamp is beyond the mixed track."
        : result.reason === "not-ready"
          ? "Session audio is not ready."
          : "This passage has an invalid timestamp.");
      return false;
    }
    stopSource();
    offsetRef.current = result.seconds;
    setCurrentTime(result.seconds);
    await playFromOffset();
    return sourceRef.current !== null;
  }, [duration, playFromOffset, status, stopSource]);

  const setRate = useCallback((next: PlaybackRate) => {
    rateRef.current = next;
    setRateState(next);
    // Re-anchor the running source so the position readout stays truthful.
    const context = contextRef.current;
    if (sourceRef.current && context) {
      sourceRef.current.playbackRate.value = next;
      startedAtRef.current = context.currentTime - offsetRef.current / next;
    }
  }, []);

  return {
    status,
    isPlaying,
    currentTime,
    duration,
    error,
    rate,
    waveform,
    play: playFromOffset,
    pause,
    seek,
    seekAndPlay,
    setRate,
  };
}
