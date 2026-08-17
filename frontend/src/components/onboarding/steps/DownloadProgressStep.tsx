import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Check, Loader2, Mic } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { useOnboarding } from '@/contexts/OnboardingContext';
import { OnboardingContainer } from '../OnboardingContainer';

const PARAKEET_MODEL = 'parakeet-tdt-0.6b-v3-int8';

type DownloadStatus = 'waiting' | 'downloading' | 'completed' | 'error';

interface DownloadState {
  status: DownloadStatus;
  progress: number;
  downloadedMb: number;
  totalMb: number;
  speedMbps: number;
  error?: string;
}

export function DownloadProgressStep() {
  const {
    goNext,
    parakeetDownloaded,
    setParakeetDownloaded,
    startBackgroundDownloads,
    completeOnboarding,
  } = useOnboarding();
  const [isMac, setIsMac] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  const startedRef = useRef(false);
  const retryingRef = useRef(false);
  const [state, setState] = useState<DownloadState>({
    status: parakeetDownloaded ? 'completed' : 'waiting',
    progress: parakeetDownloaded ? 100 : 0,
    downloadedMb: 0,
    totalMb: 670,
    speedMbps: 0,
  });

  useEffect(() => {
    import('@tauri-apps/plugin-os')
      .then(({ platform }) => setIsMac(platform() === 'macos'))
      .catch(() => setIsMac(navigator.userAgent.includes('Mac')));
  }, []);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    if (!parakeetDownloaded) setState((current) => ({ ...current, status: 'downloading' }));
    startBackgroundDownloads({ includeParakeet: true }).catch((error) => {
      setState((current) => ({ ...current, status: 'error', error: String(error) }));
    });
  }, [parakeetDownloaded, startBackgroundDownloads]);

  useEffect(() => {
    const progress = listen<{
      modelName: string;
      progress: number;
      downloaded_mb?: number;
      total_mb?: number;
      speed_mbps?: number;
      status?: string;
    }>('parakeet-model-download-progress', ({ payload }) => {
      if (payload.modelName !== PARAKEET_MODEL) return;
      const completed = payload.status === 'completed' || payload.progress >= 100;
      setState((current) => ({
        ...current,
        status: completed ? 'completed' : 'downloading',
        progress: payload.progress,
        downloadedMb: payload.downloaded_mb ?? current.downloadedMb,
        totalMb: payload.total_mb ?? current.totalMb,
        speedMbps: payload.speed_mbps ?? current.speedMbps,
      }));
      if (completed) setParakeetDownloaded(true);
    });
    const complete = listen<{ modelName: string }>('parakeet-model-download-complete', ({ payload }) => {
      if (payload.modelName !== PARAKEET_MODEL) return;
      setState((current) => ({ ...current, status: 'completed', progress: 100 }));
      setParakeetDownloaded(true);
    });
    const failure = listen<{ modelName: string; error: string }>('parakeet-model-download-error', ({ payload }) => {
      if (payload.modelName !== PARAKEET_MODEL) return;
      setState((current) => ({ ...current, status: 'error', error: payload.error }));
    });

    return () => {
      progress.then((unlisten) => unlisten());
      complete.then((unlisten) => unlisten());
      failure.then((unlisten) => unlisten());
    };
  }, [setParakeetDownloaded]);

  const retry = async () => {
    if (retryingRef.current) return;
    retryingRef.current = true;
    setState((current) => ({ ...current, status: 'downloading', progress: 0, error: undefined }));
    try {
      await invoke('parakeet_retry_download', { modelName: PARAKEET_MODEL });
    } catch (error) {
      setState((current) => ({ ...current, status: 'error', error: String(error) }));
      toast.error('Download retry failed', { description: 'Please check your connection and try again.' });
    } finally {
      retryingRef.current = false;
    }
  };

  const continueSetup = async () => {
    try {
      await invoke('parakeet_init');
      if (await invoke<boolean>('parakeet_has_available_models')) {
        setParakeetDownloaded(true);
        setState((current) => ({ ...current, status: 'completed', progress: 100 }));
      } else if (state.status === 'error') {
        toast.error('Transcription engine required', { description: 'Please retry the download before continuing.' });
        return;
      }
    } catch {
      if (!parakeetDownloaded) return;
    }

    if (isMac) {
      goNext();
      return;
    }

    setIsCompleting(true);
    try {
      await completeOnboarding();
      window.location.reload();
    } catch {
      toast.error('Failed to complete setup', { description: 'Please try again.' });
      setIsCompleting(false);
    }
  };

  return (
    <OnboardingContainer
      title="Getting things ready"
      description="Download the local transcription engine. Findings use Apple Intelligence and need no model download."
      step={3}
      totalSteps={isMac ? 4 : 3}
    >
      <div className="flex flex-col items-center space-y-6">
        <div className="w-full max-w-lg rounded-xl border border-gray-200 bg-white p-5" aria-live="polite">
          <div className="mb-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-gray-100">
                <Mic className="h-5 w-5 text-gray-600" aria-hidden="true" />
              </div>
              <div>
                <h3 className="font-medium text-gray-900">Transcription Engine</h3>
                <p className="text-sm text-gray-500">~670 MB</p>
              </div>
            </div>
            {state.status === 'completed' ? (
              <Check className="h-5 w-5 text-green-600" aria-label="Download complete" />
            ) : state.status === 'error' ? (
              <span className="text-sm text-red-600">Failed</span>
            ) : (
              <Loader2 className="h-5 w-5 animate-spin text-gray-700" aria-label="Downloading" />
            )}
          </div>
          {state.status !== 'error' && (
            <>
              <div className="h-2 w-full overflow-hidden rounded-full bg-gray-200">
                <div className="h-full bg-gray-900" style={{ width: `${state.progress}%` }} />
              </div>
              <p className="mt-2 text-sm text-gray-600">
                {state.downloadedMb.toFixed(1)} MB / {state.totalMb.toFixed(1)} MB · {Math.round(state.progress)}%
              </p>
            </>
          )}
          {state.status === 'error' && (
            <div className="mt-3 rounded-md border border-red-200 bg-red-50 p-3">
              <p className="text-sm text-red-700">{state.error || 'Download failed'}</p>
              <Button className="mt-3 w-full" onClick={retry}>Try Again</Button>
            </div>
          )}
        </div>
        <Button
          className="h-11 w-full max-w-xs bg-gray-900 text-white hover:bg-gray-800"
          disabled={!parakeetDownloaded || isCompleting}
          onClick={continueSetup}
        >
          {isCompleting ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : 'Continue'}
        </Button>
      </div>
    </OnboardingContainer>
  );
}
